use crate::vcs::db::conn;
use chrono::{Duration, Local};
use crossterm::{
    execute,
    style::{Print, Stylize},
};
use inquire::{Select, Text};
use sqlx::{Row, SqlitePool};
use std::io::stdout;

pub async fn current_branch(conn: &SqlitePool) -> String {
    if let Ok(x) = sqlx::query("SELECT current_branch FROM config")
        .fetch_one(conn)
        .await
    {
        x.get(0)
    } else {
        String::from("main")
    }
}

pub fn short_ago(timestamp: &str) -> String {
    // On essaie de parser le format RFC3339 (Rust) ou le format classique SQLite
    let parsed = chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|dt| dt.with_timezone(&chrono::Local))
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S")
                // 🛠️ CORRECTION ICI : On précise à Chrono que la date de SQLite est en UTC,
                // puis on la convertit à ton heure locale !
                .map(|ndt| ndt.and_utc().with_timezone(&chrono::Local))
        });

    if let Ok(parsed_time) = parsed {
        let now = chrono::Local::now();
        let duration = now.signed_duration_since(parsed_time);

        // On s'assure de ne pas afficher de valeurs négatives si les horloges
        // de la BDD et du système ont quelques millisecondes de décalage
        if duration.num_seconds() <= 0 {
            "0s".to_string()
        } else if duration.num_seconds() < 60 {
            format!("{}s", duration.num_seconds())
        } else if duration.num_minutes() < 60 {
            format!("{}m", duration.num_minutes())
        } else if duration.num_hours() < 24 {
            format!("{}h", duration.num_hours())
        } else {
            format!("{}d", duration.num_days())
        }
    } else {
        "-".to_string()
    }
}

pub struct BranchMetadata {
    pub name: String,
    pub last_commit_date: String,
    pub ahead: i32,
    pub behind: i32,
    pub insertions: i32,
    pub deletions: i32,
    pub total_commits: i32,
    pub total_contributors: i32,
    pub last_author: String,
    pub last_message: String,
    pub description: String,
    pub expires_at: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

pub async fn get_branch_metadata(
    pool: &SqlitePool,
    branch_name: &str,
) -> Result<BranchMetadata, sqlx::Error> {
    let query = r#"
    WITH RECURSIVE
      -- 1. Historique de main
      main_history AS (
          SELECT c.id, c.hash, c.parent_hash
          FROM branches b
          JOIN commits c ON b.head_commit_id = c.id
          WHERE b.name = 'main'
          UNION ALL
          SELECT c.id, c.hash, c.parent_hash
          FROM commits c
          JOIN main_history mh ON c.hash = mh.parent_hash
      ),
      -- 2. Historique de la branche cible
      branch_history AS (
          SELECT 
              b.name AS branch_name,
              c.id AS commit_id,
              c.hash AS commit_hash,
              c.parent_hash,
              c.author_id,
              u.name AS author_name,
              c.message,
              c.insertions,
              c.deletions,
              c.timestamp,
              1 AS commit_index
          FROM branches b
          JOIN commits c ON b.head_commit_id = c.id
          JOIN users u ON c.author_id = u.id
          WHERE b.name = $1
          
          UNION ALL
          
          SELECT 
              bh.branch_name,
              c.id AS commit_id,
              c.hash AS commit_hash,
              c.parent_hash,
              c.author_id,
              NULL AS author_name,
              NULL AS message,
              c.insertions,
              c.deletions,
              c.timestamp,
              bh.commit_index + 1
          FROM commits c
          JOIN branch_history bh ON c.hash = bh.parent_hash
      )
    SELECT 
        bh.branch_name,
        COALESCE(MAX(CASE WHEN bh.commit_index = 1 THEN bh.timestamp END), 'Unknown') AS last_commit_date,
        COALESCE(MAX(CASE WHEN bh.commit_index = 1 THEN bh.author_name END), 'Unknown') AS last_author,
        COALESCE(MAX(CASE WHEN bh.commit_index = 1 THEN bh.message END), 'No message') AS last_message,
        (SELECT created_at FROM branches WHERE name = bh.branch_name) AS created_at,
        (SELECT updated_at FROM branches WHERE name = bh.branch_name) AS updated_at,
        (SELECT description FROM branches WHERE name = bh.branch_name) AS description,
        (SELECT expires_at FROM branches WHERE name = bh.branch_name) AS expires_at,
        COUNT(*) AS total_commits,
        COUNT(DISTINCT bh.author_id) AS total_contributors,
        COALESCE(SUM(bh.insertions), 0) AS total_insertions,
        COALESCE(SUM(bh.deletions), 0) AS total_deletions,
        COALESCE(SUM(CASE WHEN bh.commit_hash NOT IN (SELECT hash FROM main_history) THEN 1 ELSE 0 END), 0) AS ahead,
        COALESCE((SELECT COUNT(*) FROM main_history) - SUM(CASE WHEN bh.commit_hash IN (SELECT hash FROM main_history) THEN 1 ELSE 0 END), 0) AS behind
    FROM branch_history bh
    GROUP BY bh.branch_name;
    "#;

    let row = sqlx::query(query)
        .bind(branch_name) // On injecte le nom de la branche sécuritairement
        .fetch_one(pool)
        .await?;

    Ok(BranchMetadata {
        name: row.get::<String, _>("branch_name"),
        // Tu pourras formatter la date plus tard si besoin
        last_commit_date: row.get::<String, _>("last_commit_date"),
        ahead: row.get::<i64, _>("ahead") as i32,
        behind: row.get::<i64, _>("behind") as i32,
        insertions: row.get::<i64, _>("total_insertions") as i32,
        deletions: row.get::<i64, _>("total_deletions") as i32,
        total_commits: row.get::<i64, _>("total_commits") as i32,
        total_contributors: row.get::<i64, _>("total_contributors") as i32,
        last_author: row.get::<String, _>("last_author"),
        last_message: row.get::<String, _>("last_message"),
        description: row
            .try_get::<String, _>("description")
            .unwrap_or_else(|_| "No description".to_string()),
        expires_at: row
            .try_get::<Option<String>, _>("expires_at")
            .unwrap_or(None),
        created_at: row
            .try_get::<Option<String>, _>("created_at")
            .unwrap_or(None),
        updated_at: row
            .try_get::<Option<String>, _>("updated_at")
            .unwrap_or(None),
    })
}

// On passe le pool en référence, comme tu l'as fait pour current_branch
async fn branches(pool: &SqlitePool) -> Vec<String> {
    let mut data: Vec<String> = Vec::new();

    // On gère l'erreur doucement avec if let plutôt qu'un expect()
    if let Ok(rows) = sqlx::query("SELECT name FROM branches")
        .fetch_all(pool)
        .await
    {
        for row in rows {
            data.push(row.get(0));
        }
    }
    data
}
pub async fn list_branches() -> bool {
    // Note: Assure-toi que conn() retourne bien un pool utilisable
    let pool = conn().await;

    let current_branch = current_branch(&pool).await;
    let branches = branches(&pool).await;

    for branch_name in &branches {
        // On récupère les métadonnées depuis la BDD
        match get_branch_metadata(&pool, branch_name).await {
            Ok(metadata) => {
                let is_current = branch_name.eq(&current_branch);
                let is_feature = branch_name.starts_with("feature");
                let is_hotfix = branch_name.starts_with("hotfix");

                // On passe la structure complète à ta fonction d'affichage
                print_branch(&metadata, is_current, is_feature, is_hotfix);
            }
            Err(e) => {
                // Gestion d'erreur propre pour éviter que l'outil CLI ne crash
                eprintln!(
                    "Erreur lors de la récupération de la branche {} : {}",
                    branch_name, e
                );
            }
        }
    }
    true
}
pub fn print_branch(meta: &BranchMetadata, current: bool, feature: bool, hotfix: bool) {
    // 1. Gestion du label de type de branche (toujours 7 caractères)
    let label_raw = if current {
        "CURRENT"
    } else if feature {
        "FEATURE"
    } else if hotfix {
        "PATCHES"
    } else {
        "DEFAULT"
    };
    let branch = if current {
        meta.name
            .to_string()
            .replace("feature/", "")
            .replace("hotfix/", "")
    } else if feature {
        meta.name.replace("feature/", "")
    } else if hotfix {
        meta.name.replace("hotfix/", "")
    } else {
        meta.name.to_string()
    };

    // On applique le padding de 7 caractères sur le texte brut
    let label_padded = format!("{:<7}", label_raw);

    // On applique la couleur après coup sur la chaîne déjà formatée
    let label_styled = label_padded.green().bold();

    // 2. Préparation des colonnes avec padding fixe
    let branch_col = format!("{:<20}", branch);

    let c_ago = short_ago(meta.created_at.as_deref().unwrap_or(""));
    let u_ago = short_ago(meta.updated_at.as_deref().unwrap_or(""));
    let comm_ago = short_ago(&meta.last_commit_date);

    // On fabrique une seule colonne de 24 caractères (ex: "crt:2d  upd:1h  cmt:5m  ")
    let dates_col = format!("crt:{:<3} upd:{:<3} cmt:{:<3}", c_ago, u_ago, comm_ago);
    let dates_styled = dates_col.dark_grey(); // En gris pour ne pas distraire

    // Synchro : ex "↑2 ↓0" (toujours aligné sur 8 caractères)
    let sync_text = format!("↑{} ↓{}", meta.ahead, meta.behind);
    let sync_col = format!("{:<8}", sync_text);

    // Lignes modifiées : ex "+120 -15" (aligné sur 12 caractères)
    // On colore individuellement le + en vert et le - en rouge pour le style !
    let diff_col = format!(
        "{:<12}",
        format!("+{:<4} -{:<4}", meta.insertions, meta.deletions)
    );

    // Commits et Contributeurs (Sans icônes, texte propre)
    let commits_text = if meta.total_commits > 1 {
        format!("{} commits", meta.total_commits)
    } else {
        format!("{} commit", meta.total_commits)
    };
    let commits_col = format!("{:<12}", commits_text);

    let devs_text = if meta.total_contributors > 1 {
        format!("{} devs", meta.total_contributors)
    } else {
        format!("{} dev", meta.total_contributors)
    };
    let devs_col = format!("{:<8}", devs_text);

    let author_col = format!("{:<15}", meta.last_author);

    let safe_msg = meta.last_message.lines().next().unwrap_or("").to_string();
    let msg_truncated = if safe_msg.chars().count() > 35 {
        format!("{}...", safe_msg.chars().take(32).collect::<String>())
    } else {
        safe_msg
    };

    execute!(
        stdout(),
        Print("\n"),
        Print("  "),
        Print(label_styled),
        Print(format!(
            "   {} {} {} {} {} {} {} {}", // <-- Un {} en plus ici
            branch_col.white(),
            dates_styled,
            sync_col.yellow(),
            diff_col,
            commits_col.dark_grey(),
            devs_col.dark_grey(),
            author_col.magenta(),
            msg_truncated.white() // <-- Le message en gris italique à la fin !
        ))
    )
    .expect("Erreur d'affichage");
    if meta.name == "main" {
        execute!(stdout(), Print("\n")).expect("Erreur d'affichage sous-ligne");
    }
    if meta.name != "main" {
        let mut expiration_text = String::new();

        // On calcule la différence entre la date d'expiration et maintenant
        if let Some(exp) = &meta.expires_at
            && let Ok(exp_date) = chrono::NaiveDateTime::parse_from_str(exp, "%Y-%m-%d %H:%M:%S")
        {
            let now = chrono::Local::now().naive_local();
            let diff = exp_date - now;

            if diff.num_days() > 0 {
                expiration_text = format!("{} days left", diff.num_days());
            } else if diff.num_hours() > 0 {
                expiration_text = format!("{} hours left", diff.num_hours());
            } else if diff.num_minutes() > 0 {
                expiration_text = format!("{} mins left", diff.num_minutes());
            } else {
                expiration_text = "Expired!".to_string();
            }
        }

        let desc = if meta.description.is_empty() || meta.description == "No description" {
            "No objective defined"
        } else {
            &meta.description
        };

        // On affiche avec un décalage de 12 espaces pour s'aligner sous le nom de la branche
        execute!(
            stdout(),
            Print(format!(
                "    {} ({})",
                desc.dark_grey(),
                expiration_text.dark_grey().italic()
            )), // 12 espaces
        )
        .expect("Erreur d'affichage sous-ligne");
    }
    // On affiche avec un décalage de 12 espaces pour s'aligner sous le nom de la branche
    execute!(stdout(), Print("\n")).expect("Erreur d'affichage sous-ligne");
}

pub async fn create_branch() -> anyhow::Result<()> {
    let pool = crate::vcs::db::conn().await;

    // 1. Déterminer la convention de nommage (Git Flow revisité)
    let branch_types = vec!["feature", "hotfix", "experiment", "refactor"];
    let b_type = Select::new("Branch type:", branch_types).prompt()?;

    let mut b_name = String::new();
    while b_name.is_empty() {
        b_name.clear();
        b_name = Text::new("Branch name (ex: login-screen):").prompt()?;
    }
    let full_branch_name = format!("{b_type}/{b_name}");

    // 2. Demander l'objectif strict de cette branche
    let mut description = String::new();
    while description.is_empty() {
        description.clear();
        description = Text::new("Objective / Description (Why this branch?):").prompt()?;
    }

    // 3. Récupérer le commit actuel (pour le head_commit_id)
    // On part du principe qu'on bifurque depuis la branche courante
    let current_branch_name = current_branch(&pool).await;
    let head_row = sqlx::query("SELECT head_commit_id FROM branches WHERE name = ?")
        .bind(&current_branch_name)
        .fetch_optional(&pool)
        .await?;

    let head_commit_id: i64 = match head_row {
        Some(row) => row.get(0),
        None => {
            crate::vcs::ko("Cannot create a branch from an empty repository. Commit first!");
            return Ok(());
        }
    };

    // 4. Calculer la durée de vie (10 jours)
    let expires_at = (Local::now() + Duration::try_days(10).unwrap())
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    // 5. Insertion en base de données
    let result = sqlx::query(
        "INSERT INTO branches (name, head_commit_id, description, expires_at) VALUES (?, ?, ?, ?)",
    )
    .bind(&full_branch_name)
    .bind(head_commit_id)
    .bind(description)
    .bind(&expires_at)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            // On bascule automatiquement sur la nouvelle branche (checkout)
            sqlx::query("UPDATE config SET value = ? WHERE key = 'current_branch'")
                .bind(&full_branch_name)
                .execute(&pool)
                .await?;

            crate::vcs::ok(&format!(
                "Branch `{}` created ! Ephemeral timer started: expires on {}",
                full_branch_name, expires_at
            ));
        }
        Err(e) => {
            crate::vcs::ko(&format!("Failed to create branch: {}", e));
        }
    }
    Ok(())
}
