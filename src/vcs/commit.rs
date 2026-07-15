use crate::vcs::commit_created;
use crate::vcs::db;
use crate::vcs::hooks::group_hooks;
use crate::vcs::keys::awq_sign_message;
use crate::vcs::ko;
use crate::vcs::todo::TodoItem;
use crate::vcs::todo::awq_complete_todo;
use crate::vcs::todo::todos;
use crate::vcs::{locale, ok, ok_status, tt};
use chrono::Local;
use crossterm::style::Stylize;
use inquire::error::InquireResult;
use inquire::{Editor, InquireError, Select, Text};
use justify::{Settings, justify};
#[cfg(unix)]
use nix::sys::utsname::uname;
#[cfg(unix)]
use nix::unistd::User;
use similar::{ChangeTag, TextDiff};
use sqlx::Connection;
use sqlx::Row;
use sqlx::SqliteConnection;
use sqlx::SqlitePool;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env::consts::ARCH;
use std::fmt::{Display, Formatter};
use std::io::Error;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;

#[cfg(unix)]
pub fn get_file_mode(path: &Path) -> Option<u32> {
    let metadata = std::fs::metadata(path).ok()?;
    Some(metadata.permissions().mode())
}

#[cfg(not(unix))]
pub fn get_file_mode(_path: &Path) -> Option<u32> {
    Some(0o100644)
}

pub enum Node {
    File { hash: String, mode: u32, size: u64 },
    Directory { children: BTreeMap<String, Node> },
}

pub fn format_justified_with_newlines(raw_text: &str) -> String {
    // On configure la largeur de ta justification
    let settings = Settings::default();

    raw_text
        .split('\n') // On coupe le texte à chaque retour à la ligne explicite
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                // Si la ligne est vide (ex: l'espace entre deux paragraphes),
                // on la laisse vide pour ne pas que justify plante ou mette des espaces bizarres.
                String::new()
            } else {
                // On justifie uniquement ce bloc/cette ligne
                justify(trimmed, &settings)
            }
        })
        .collect::<Vec<String>>() // On rassemble toutes les lignes traitées
        .join("\n") // On remet nos retours à la ligne !
}

pub async fn awq_log() -> Result<(), anyhow::Error> {
    let mut pool = SqliteConnection::connect("sqlite://.awq/awq.db")
        .await
        .expect("no db file");

    let query = "
        SELECT 
            c.hash, 
            u.name AS author, 
            c.message, 
            c.timestamp, 
            COALESCE(c.signature, '') AS signature, 
            COALESCE(t.title, 'No ticket') AS ticket, 
            COALESCE(c.todo_id, 0) AS ticket_id, 
            COALESCE(t.description, 'No description') AS ticket_description, 
            c.tree_hash, 
            c.parent_hash 
        FROM commits c
        JOIN users u ON c.author_id = u.id
        LEFT JOIN todos t ON c.todo_id = t.id
        ORDER BY c.id DESC
    ";

    let rows = sqlx::query(query).fetch_all(&mut pool).await?;

    if rows.is_empty() {
        crate::vcs::ok("No commits yet. Time to create the Big Bang!");
        return Ok(());
    }

    for row in rows {
        let _hash: String = row.get(0);
        let author: String = row.get(1);
        let message: String = row.get(2);
        let timestamp: String = row.get(3);
        let signature: String = row.get(4);
        let ticket: String = row.get(5);
        let ticket_id: i64 = row.get(6);
        let ticket_description: String = row.get(7);
        let tree_hash: String = row.get(8);

        // parent_hash peut être NULL pour le premier commit (Option<String>)
        let parent_hash: Option<String> = row.try_get(9).unwrap_or(None);

        let log_entry = Log {
            author,
            message,
            at: timestamp,
            signature,
            changes: compute_commit_changes(&mut pool, &tree_hash, parent_hash)
                .await
                .expect("failed to get changes"),
            ticket,
            ticket_id: ticket_id.to_string(),
            ticket_description,
        };
        println!("{}", log_entry);
    }

    Ok(())
}
async fn compute_commit_changes(
    pool: &mut SqliteConnection,
    current_tree_hash: &str,
    parent_hash: Option<String>,
) -> Result<Vec<(String, FileChange)>, anyhow::Error> {
    let current_files = get_tree_files(&mut *pool, current_tree_hash)
        .await
        .unwrap_or_default();
    let mut parent_files = HashMap::new();

    if let Some(parent) = parent_hash {
        let p_row = sqlx::query("SELECT tree_hash FROM commits WHERE hash = ?")
            .bind(parent)
            .fetch_optional(&mut *pool) // <-- CHANGEMENT ICI
            .await?;

        if let Some(r) = p_row {
            let p_tree: String = r.get(0);
            parent_files = get_tree_files(&mut *pool, &p_tree)
                .await
                .unwrap_or_default();
        }
    }

    let mut changes = Vec::new();

    // 1. Détecter les ajouts et modifications
    for (path, current_file_hash) in &current_files {
        let path_str = path.display().to_string();
        match parent_files.get(path) {
            Some(parent_file_hash) => {
                if current_file_hash != parent_file_hash {
                    // LE VRAI CALCUL DU DIFF EST ICI !
                    let old_text = get_blob_text(pool, parent_file_hash)
                        .await
                        .unwrap_or_default();
                    let new_text = get_blob_text(pool, current_file_hash)
                        .await
                        .unwrap_or_default();
                    let (added, deleted) = count_diff_lines(&old_text, &new_text);

                    changes.push((
                        path_str,
                        FileChange::Modified {
                            added,
                            deleted,
                            mode: None,
                        },
                    ));
                }
            }
            None => {
                // Fichier ajouté : on compte toutes ses lignes comme "ajoutées"
                let new_text = get_blob_text(pool, current_file_hash)
                    .await
                    .unwrap_or_default();
                let added = new_text.lines().count();

                changes.push((path_str, FileChange::Added { added, mode: None }));
            }
        }
    }

    // 2. Détecter les suppressions
    for (path, parent_file_hash) in &parent_files {
        if !current_files.contains_key(path) {
            // Fichier supprimé : on compte toutes ses lignes comme "supprimées"
            let old_text = get_blob_text(pool, parent_file_hash)
                .await
                .unwrap_or_default();
            let deleted = old_text.lines().count();

            changes.push((
                path.display().to_string(),
                FileChange::Deleted {
                    deleted,
                    mode: None,
                },
            ));
        }
    }

    Ok(changes)
}

pub async fn get_tree_files(
    pool: &mut SqliteConnection,
    root_tree_hash: &str,
) -> Result<HashMap<PathBuf, String>, anyhow::Error> {
    let mut files = HashMap::new();

    // Notre pile pour parcourir l'arbre sans récursion.
    // Elle stocke des tuples : (Chemin_en_cours, Hash_de_l_arbre_ou_dossier)
    let mut stack = vec![(PathBuf::new(), root_tree_hash.to_string())];

    while let Some((current_path, current_tree_hash)) = stack.pop() {
        // On récupère tous les enfants de ce "nœud"
        let rows = sqlx::query("SELECT name, hash, mode FROM nodes WHERE parent_tree_hash = ?")
            .bind(&current_tree_hash)
            .fetch_all(&mut *pool)
            .await?;

        for row in rows {
            let name: String = row.get(0);
            let hash: String = row.get(1);
            let mode: u32 = row.get(2);

            let mut node_path = current_path.clone();
            node_path.push(name);

            // MAGIE SYSTÈME :
            // En Unix, le masque de type de fichier dans st_mode est 0o170000.
            // Le type pour un répertoire (S_IFDIR) est 0o040000.
            let is_dir = (mode & 0o170000) == 0o040000;

            if is_dir {
                // C'est un sous-dossier : on l'empile pour l'explorer plus tard
                stack.push((node_path, hash));
            } else {
                // C'est un fichier : on l'ajoute au résultat final aplati
                files.insert(node_path, hash);
            }
        }
    }

    Ok(files)
}

#[allow(dead_code)]
pub struct HeadState {
    pub branch_name: String,
    pub commit_id: i64,
    pub commit_hash: String,
    pub tree_hash: String,
}

pub async fn get_head_state(pool: &SqlitePool) -> Result<Option<HeadState>, anyhow::Error> {
    // Une seule requête SQL optimisée avec des JOIN pour lier config -> branches -> commits
    let query = "
        SELECT b.name, c.id, c.hash, c.tree_hash
        FROM config cfg
        JOIN branches b ON cfg.value = b.name
        JOIN commits c ON b.head_commit_id = c.id
        WHERE cfg.key = 'current_branch'
    ";

    let row = sqlx::query(query).fetch_optional(pool).await?;

    // On retourne Option<HeadState> car un dépôt qui vient d'être initialisé
    // n'a pas encore de commits, donc le HEAD est virtuellement "vide".
    match row {
        Some(r) => Ok(Some(HeadState {
            branch_name: r.get(0),
            commit_id: r.get(1),
            commit_hash: r.get(2),
            tree_hash: r.get(3),
        })),
        None => Ok(None),
    }
}

#[allow(dead_code)]
pub enum FileStatus {
    New(PathBuf),           // N'existe pas en base -> Nouvel Asset
    Modified(PathBuf, i64), // Existe mais hash différent -> Même Asset
    Deleted(PathBuf, i64),  // Existe en base mais plus sur disque
    Unchanged,
}

pub async fn awq_status() -> Result<Vec<FileStatus>, anyhow::Error> {
    let mut pool = SqliteConnection::connect(".awq/awq.db")
        .await
        .expect("no db file");
    // 1. On récupère la photo du dernier commit (Les métadonnées HEAD)
    let head_state = get_head_state(&db::conn().await).await.expect("no db file");

    // 2. CORRECTION : On convertit ce HEAD en HashMap de fichiers (db_state)
    // Si c'est le tout premier commit (None), on renvoie une HashMap vide.
    let db_state = match head_state {
        Some(head) => get_tree_files(&mut pool, &head.tree_hash)
            .await
            .unwrap_or_default(),
        None => HashMap::new(),
    };

    let mut changes = Vec::new();
    let mut files_on_disk: HashSet<PathBuf> = HashSet::new();

    // 3. On scanne le dossier de travail actuel
    let walk = ignore::WalkBuilder::new(".")
        .add_custom_ignore_filename(".awqignore")
        .threads(4)
        .parents(true)
        .git_ignore(false)
        .git_exclude(false)
        .hidden(false)
        .build()
        .flatten(); // Flatten simplifie l'itération

    for result in walk {
        let path = result.path();

        // On ignore les dossiers profonds du VCS (.awq et .git) et les dossiers eux-mêmes
        if path.components().any(|c| {
            c.as_os_str() == ".awq"
                || c.as_os_str() == ".git"
                || c.as_os_str() == ".hg"
                || c.as_os_str() == ".svn"
        }) || path.is_dir()
        {
            continue;
        }

        let relative_path = path.strip_prefix("./").unwrap_or(path).to_path_buf();
        files_on_disk.insert(relative_path.clone());

        // Lecture et hash du fichier actuel sur le disque
        let content = match std::fs::read(path) {
            Ok(c) => c,
            Err(_) => continue, // On ignore silencieusement les fichiers illisibles
        };
        let current_hash = blake3::hash(&content).to_hex().to_string();

        // 3. Comparaison avec la BDD
        match db_state.get(&relative_path) {
            Some(db_hash) => {
                if *db_hash != current_hash {
                    // Modifié (On met 0 pour l'id, car ok_status s'en fiche pour l'affichage)
                    changes.push(FileStatus::Modified(relative_path, 0));
                }
            }
            None => {
                // Fichier totalement nouveau
                changes.push(FileStatus::New(relative_path));
            }
        }
    }

    // 4. Détection des suppressions (présent en BDD, mais plus sur le disque)
    for (path, _) in db_state {
        if !files_on_disk.contains(&path) {
            changes.push(FileStatus::Deleted(path, 0));
        }
    }

    // 5. Affichage propre
    if changes.is_empty() {
        ok("No changes detected. Working tree is clean.");
    } else {
        for change in &changes {
            ok_status(change);
        }
    }
    Ok(changes)
}
#[async_recursion::async_recursion]
async fn store_tree_recursive_async(
    tx: &mut SqliteConnection,
    _name: &str,
    node: &Node,
) -> Result<String, anyhow::Error> {
    match node {
        // Si c'est un fichier, on retourne juste son hash
        Node::File { hash, .. } => Ok(hash.clone()),

        // Si c'est un dossier, on traite ses enfants
        Node::Directory { children } => {
            let mut hasher = blake3::Hasher::new();
            let mut children_data = Vec::new();

            for (name, child_node) in children {
                // Appel récursif asynchrone
                let child_hash = store_tree_recursive_async(&mut *tx, name, child_node).await?;

                let (mode, size) = match child_node {
                    Node::File { mode, size, .. } => (*mode, Some(*size as i64)),
                    Node::Directory { .. } => (0o040755, None),
                };

                hasher.update(name.as_bytes());
                hasher.update(child_hash.as_bytes());

                children_data.push((name, child_hash, mode, size));
            }

            let dir_hash = hasher.finalize().to_hex().to_string();

            // On enregistre dans la table `nodes` (anciennement `tree_nodes`)
            for (name, hash, mode, size) in children_data {
                sqlx::query("INSERT OR IGNORE INTO nodes (parent_tree_hash, name, hash, mode, size) VALUES (?, ?, ?, ?, ?)")
                    .bind(&dir_hash)
                    .bind(name)
                    .bind(hash)
                    .bind(mode as i64)
                    .bind(size)
                    .execute(&mut *tx)
                    .await?;
            }
            Ok(dir_hash)
        }
    }
}

fn insert_into_tree(root: &mut Node, path: &Path, hash: String, mode: u32, size: u64) {
    let mut current = root;

    // On parcourt chaque composant du chemin (ex: ["src", "ui", "main.rs"])
    for component in path.components() {
        let name = component.as_os_str().to_string_lossy().to_string();

        // On descend dans l'arbre. Si le dossier n'existe pas, on le crée.
        if let Node::Directory { children } = current {
            current = children.entry(name).or_insert_with(|| Node::Directory {
                children: BTreeMap::new(),
            });
        }
    }

    // Une fois arrivé au bout du chemin, on remplace le nœud par le fichier réel
    *current = Node::File { hash, mode, size };
}
pub async fn save_commit(
    message: &str,
    author: &str,
    ticket: &TodoItem,
) -> Result<i64, anyhow::Error> {
    // 1. On démarre une transaction globale
    let pool = db::conn().await;
    let mut tx = pool.begin().await?;

    let mut root_tree = Node::Directory {
        children: BTreeMap::new(),
    };

    let walk = ignore::WalkBuilder::new(".")
        .threads(4)
        .add_custom_ignore_filename(".awqignore")
        .standard_filters(true)
        .hidden(false)
        .build();

    for result in walk.flatten() {
        let path = result.path();
        if path.is_dir()
            || path.components().any(|c| {
                c.as_os_str() == ".awq"
                    || c.as_os_str() == ".git"
                    || c.as_os_str() == ".hg"
                    || c.as_os_str() == ".svn"
            })
        {
            continue;
        }

        let relative = path.strip_prefix("./").unwrap_or(path);
        let content = std::fs::read(path)?;
        let content_hash = blake3::hash(&content).to_hex().to_string();
        let metadata = std::fs::metadata(path)?;
        let size = metadata.len() as i64;

        // Compression avec ta fonction (zstd)
        let compressed = compress(&content);

        // Insertion du blob directement dans la transaction
        sqlx::query(
            "INSERT OR IGNORE INTO blobs (hash, content, size, mime_type) VALUES (?, ?, ?, ?)",
        )
        .bind(&content_hash)
        .bind(&compressed)
        .bind(size)
        .bind("application/octet-stream")
        .execute(&mut *tx)
        .await?;

        let mode = get_file_mode(path).unwrap_or(0);
        insert_into_tree(&mut root_tree, relative, content_hash, mode, metadata.len());
    }

    // 2. Calcul du root hash et enregistrement des nodes
    let root_hash = store_tree_recursive_async(&mut tx, "ROOT", &root_tree).await?;

    // 3. Récupération du parent
    let parent_row = sqlx::query("SELECT hash FROM commits ORDER BY id DESC LIMIT 1")
        .fetch_optional(&mut *tx)
        .await?;
    let parent_hash: String = parent_row.map(|r| r.get(0)).unwrap_or_default();

    // NOUVEAU : Récupération ou création de l'auteur (author_id)
    let author_row = sqlx::query("SELECT id FROM users WHERE name = ?")
        .bind(author)
        .fetch_optional(&mut *tx)
        .await?;

    let author_id: i64 = match author_row {
        Some(row) => row.get(0),
        None => {
            // Création à la volée : on remplace 'developer' par 'fullstack'
            let default_email = format!("{}@awq.local", author);
            let new_user = sqlx::query("INSERT INTO users (name, email, default_role) VALUES (?, ?, 'fullstack') RETURNING id")
                .bind(author)
                .bind(default_email)
                .fetch_one(&mut *tx)
                .await?;
            new_user.get(0)
        }
    };

    let timestamp = chrono::Utc::now().to_rfc3339();
    let commit_hash = blake3::hash(format!("{root_hash}{author}{message}").as_bytes())
        .to_hex()
        .to_string();
    let signature = awq_sign_message(&commit_hash);

    // 🛠️ NOUVEAU : On calcule les insertions et suppressions
    // (Attention: il faudra adapter compute_commit_changes pour qu'elle accepte &mut *tx ou utiliser une logique similaire)
    let mut total_insertions = 0;
    let mut total_deletions = 0;

    let p_hash_opt = if parent_hash.is_empty() {
        None
    } else {
        Some(parent_hash.clone())
    };

    // On appelle notre fonction de diff avec la transaction en cours (&mut *tx)
    if let Ok(changes) = compute_commit_changes(&mut tx, &root_hash, p_hash_opt).await {
        for (_, change) in changes {
            match change {
                FileChange::Added { added, .. } => total_insertions += added,
                FileChange::Deleted { deleted, .. } => total_deletions += deleted,
                FileChange::Modified { added, deleted, .. } => {
                    total_insertions += added;
                    total_deletions += deleted;
                }
            }
        }
    }

    // 🛠️ CORRECTION DE LA REQUÊTE : On ajoute insertions et deletions
    let commit_row = sqlx::query(
        "INSERT INTO commits (hash, parent_hash, tree_hash, author_id, todo_id, message, timestamp, signature, insertions, deletions) 
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id"
    )
        .bind(&commit_hash)
        .bind(&parent_hash)
        .bind(&root_hash)
        .bind(author_id)
        .bind(ticket.id)
        .bind(message)
        .bind(&timestamp)
        .bind(&signature)
        .bind(total_insertions as i64) // Ajout des ajouts
        .bind(total_deletions as i64)  // Ajout des suppressions
        .fetch_one(&mut *tx)
        .await?;

    let commit_id: i64 = commit_row.get(0);

    // Ta fonction pour signer
    sqlx::query("INSERT INTO oplog (operation_type, view_state) VALUES ('commit', ?)")
        .bind(format!("{{\"head\": \"{commit_hash}\"}}"))
        .execute(&mut *tx)
        .await?;
    let config_row = sqlx::query("SELECT value FROM config WHERE key = 'current_branch'")
        .fetch_optional(&mut *tx)
        .await?;

    let branch_name = config_row
        .map(|r| r.get::<String, _>(0))
        .unwrap_or_else(|| "main".to_string());

    // On insère la branche ou on met à jour son pointeur si elle existe déjà
    sqlx::query(
        "INSERT INTO branches (name, head_commit_id) VALUES (?, ?) 
         ON CONFLICT(name) DO UPDATE SET head_commit_id = excluded.head_commit_id, updated_at = CURRENT_TIMESTAMP",
    )
    .bind(&branch_name)
    .bind(commit_id)
    .execute(&mut *tx)
    .await?;
    // Tu peux rajouter ta logique "Manifest" ici (en utilisant `&mut *tx` au lieu de `conn`)

    // 6. Validation définitive de la transaction
    tx.commit().await?;
    commit_created(&commit_hash[0..7]);
    Ok(commit_id)
}
pub fn decompress(data: &[u8]) -> Vec<u8> {
    match zstd::decode_all(data) {
        Ok(decoded) => decoded,
        Err(_) => data.to_vec(),
    }
}
pub fn compress(data: &[u8]) -> Vec<u8> {
    zstd::encode_all(data, 0).expect("Failed to compress blob with zstd")
}
fn run(active: bool, cmd: &str) -> bool {
    if active
        && std::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(".")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("missing program")
            .wait()
            .expect("failed to wait")
            .success()
            .eq(&false)
    {
        return false;
    }
    true
}

pub async fn hooks(commit: fn(&mut Commit) -> String, c: &mut Commit) -> bool {
    for x in &group_hooks(super::hooks::Trigger::PreCommit).await {
        ok(&format!("Running pre-commit hook: {}", x.name));
        if x.is_active && run(x.is_active, &x.command).eq(&false) {
            ko(&format!("Hook failed: {}", x.name));
            return false;
        }
    }
    let ticket = c.ask_ticket().await;
    let message = commit(c);
    save_commit(message.as_str(), author().as_str(), &ticket)
        .await
        .expect("failed to write commit");
    if awq_complete_todo(ticket.id).await {
        ok(tt(&locale(), "ticket-closed-successfully").as_str());
    } else {
        ko(tt(&locale(), "failed-to-close-ticket").as_str());
    }
    for x in &group_hooks(super::hooks::Trigger::PostCommit).await {
        ok(&format!("Running post-commit hook: {}", x.name));
        if x.is_active && run(x.is_active, &x.command).eq(&false) {
            ko(&format!("Hook failed: {}", x.name));
            return false;
        }
    }
    for x in &group_hooks(super::hooks::Trigger::PrePush).await {
        ok(&format!("Running pre-push hook: {}", x.name));
        if x.is_active && run(x.is_active, &x.command).eq(&false) {
            ko(&format!("Hook failed: {}", x.name));
            return false;
        }
    }
    for x in &group_hooks(super::hooks::Trigger::PostPush).await {
        ok(&format!("Running post-push hook: {}", x.name));
        if x.is_active && run(x.is_active, &x.command).eq(&false) {
            ko(&format!("Hook failed: {}", x.name));
            return false;
        }
    }
    true
}
#[doc = "First prompt to ask the user what is the objective of the changes"]
pub const WHAT: &str = "What is the objective of the changes?";

pub const HOW: &str = "How the changes were made and what was changed?";
pub const WHY: &str =
    "Why is the objective of the changes important and what is the expected outcome?";

pub const OUTCOME_PROMPT: &str = "Outcome of changes";

#[derive(Debug, Clone, PartialEq, PartialOrd, Hash, Eq, Ord)]
pub struct CommitType {
    pub name: &'static str,
    pub mnemonic: &'static str,
    pub description: &'static str,
    pub example: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CommitCategory {
    #[default]
    CoreChanges,
    MaintenanceInfrastructure,
    ProjectEvents,
    CommunicationCollaboration,
    CelestialEvents,
    CelestialObjects,
    AstronomicalConcepts,
    SpaceExploration,
}

impl Default for CommitType {
    fn default() -> Self {
        Self {
            name: "Star",
            mnemonic: "Shiny Technology Added or Refined",
            description: "New feature or enhancement",
            example: "Star(Auth): Implement two-factor authentication",
        }
    }
}

impl Display for CommitCategory {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.as_str())?;
        Ok(())
    }
}

impl Display for CommitType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}: {}", self.description, self.name)?;
        Ok(())
    }
}

impl CommitCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CoreChanges => "Core Changes",
            Self::MaintenanceInfrastructure => "Maintenance & Infrastructure",
            Self::ProjectEvents => "Project Events",
            Self::CommunicationCollaboration => "Communication & Collaboration",
            Self::CelestialEvents => "Celestial Events",
            Self::CelestialObjects => "Celestial Objects",
            Self::AstronomicalConcepts => "Astronomical Concepts",
            Self::SpaceExploration => "Space Exploration",
        }
    }

    pub fn all() -> Vec<Self> {
        Vec::from([
            Self::CoreChanges,
            Self::MaintenanceInfrastructure,
            Self::ProjectEvents,
            Self::CommunicationCollaboration,
            Self::CelestialEvents,
            Self::CelestialObjects,
            Self::AstronomicalConcepts,
        ])
    }
}

/// Va chercher un fichier dans la DB, le décompresse et le convertit en texte
async fn get_blob_text(pool: &mut SqliteConnection, hash: &str) -> Result<String, anyhow::Error> {
    let row = sqlx::query("SELECT content FROM blobs WHERE hash = ?")
        .bind(hash)
        .fetch_optional(&mut *pool)
        .await?;

    if let Some(r) = row {
        let compressed: Vec<u8> = r.get(0);
        let decompressed = decompress(&compressed); // Ta fonction existante avec zstd
        // On convertit en String, en ignorant les caractères non-UTF8 (ex: les binaires)
        return Ok(String::from_utf8_lossy(&decompressed).to_string());
    }

    Ok(String::new())
}

/// Compare deux textes et retourne le nombre de lignes (Ajoutées, Supprimées)
fn count_diff_lines(old_text: &str, new_text: &str) -> (usize, usize) {
    let diff = TextDiff::from_lines(old_text, new_text);
    let mut added = 0;
    let mut deleted = 0;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => added += 1,
            ChangeTag::Delete => deleted += 1,
            ChangeTag::Equal => {} // Ligne inchangée
        }
    }

    (added, deleted)
}

// Macro utilitaire pour instancier rapidement les CommitType
macro_rules! commit_type {
    ($name:expr, $mnemonic:expr, $desc:expr, $ex:expr) => {
        CommitType {
            name: $name,
            mnemonic: $mnemonic,
            description: $desc,
            example: $ex,
        }
    };
}

/// Synchronise automatiquement les changements vers Git.
// pour générer la map des thèmes spatiaux
pub fn get_space_themes() -> HashMap<CommitCategory, Vec<CommitType>> {
    let mut themes = HashMap::new();

    themes.insert(
        CommitCategory::CoreChanges,
        vec![
            commit_type!(
                "Star",
                "Shiny Technology Added or Refined",
                "New feature or enhancement",
                "Star(Auth): Implement two-factor authentication"
            ),
            commit_type!(
                "Comet",
                "Code or Module Error Terminated",
                "Bug fix or error resolution",
                "Comet(UI): Fix responsive layout issue on mobile devices"
            ),
            commit_type!(
                "Nebula",
                "New Efficient Better Understandable Logic Achieved",
                "Code refactoring",
                "Nebula(Backend): Refactor user management module for improved maintainability"
            ),
            commit_type!(
                "Pulsar",
                "Powerful Upgrade, Less Sluggish, Agile Response",
                "Performance improvement",
                "Pulsar(Database): Optimize queries for faster response times"
            ),
            commit_type!(
                "Quasar",
                "Quick Adjustments for Superior Accuracy and Readability",
                "Documentation or clarity improvement",
                "Quasar(API): Update documentation with new endpoint parameters"
            ),
        ],
    );

    themes.insert(
        CommitCategory::MaintenanceInfrastructure,
        vec![
            commit_type!("Asteroid Belt", "Adjustments, Sweeps, Tidy-ups, Elimination, Reordering of Items, Decrease Bloat", "Code cleanup and maintenance", "Asteroid Belt: Remove unused CSS and optimize images"),
            commit_type!("Solar Flare", "Securing Our Logic Against Regressions, Failures, and Latencies Actively, Rigorously Ensured", "Adding or updating tests (unit, integration, end-to-end).", "Solar Flare(Payments): Add unit tests for payment processing module"),
            commit_type!("Dwarf Planet", "Details Warranted Attention, Refined Further, Polished Little Aspects Neatly Enhanced Tiny", "Minor but essential updates or fixes.", "Dwarf Planet: Update project dependencies to latest versions"),
            commit_type!("Terraform", "Technology Engineering Resources Readily Automated, Foundation of Reliable Management", "Infrastructure changes", "Terraform(AWS): Provision new EC2 instance for staging environment"),
        ],
    );

    themes.insert(
        CommitCategory::ProjectEvents,
        vec![
            commit_type!(
                "Black Hole",
                "Big Legacy Aspects Consumed, Killing Heavy, Old Loads Entirely",
                "Removing large chunks of code or features",
                "Black Hole: Remove deprecated user profile module"
            ),
            commit_type!(
                "Wormhole",
                "Weaving or Reconnecting Modules, Hitching onto Linked Elements",
                "Merging branches or connecting code parts",
                "Wormhole: Merge feature/new-dashboard into develop branch"
            ),
            commit_type!(
                "Big Bang",
                "Birth of Initial Greatness, Beginning All New Growth",
                "Initial commit of a project or major feature",
                "Big Bang: Initial project setup and scaffolding"
            ),
            commit_type!(
                "Launch",
                "Lifting Application Upward, New Code Entering Production",
                "Deploying to production or releasing a version",
                "Launch(v1.2): Release new version with user profile customization"
            ),
        ],
    );

    themes.insert(
        CommitCategory::CommunicationCollaboration,
        vec![
            commit_type!("Lightspeed", "Lightening Speed Enhancements", "Significant performance improvements", "Lightspeed(Frontend): Implement lazy loading for images"),
            commit_type!("Mission Control", "Managing Changes, Issues, Scope, Teamwork, and Release On Time", "Project management changes", "Mission Control: Update project roadmap and assign tasks for Q3"),
            commit_type!("Spacewalk", "Swift Work Above Limits, Keeping All Systems Extra Safe", "Urgent hotfixes or critical production updates.", "Spacewalk(Security): Patch critical vulnerability in authentication module"),
            commit_type!("Moon Landing", "Major Leaps Over Night, New Doors and Incredible Achievements", "Completing major milestones or goals", "Moon Landing: Successfully launch beta version to select users"),
            commit_type!("First Contact", "Forge Initial Connections, Open New Territories", "Establishing initial connections or integrations", "First Contact(API): Integrate with new payment provider's API"),
            commit_type!("Interstellar Communication", "Informing, Sharing, Teaching, Educating, & Learning Lucidly & Clearly", "Improving documentation or communication", "Interstellar Communication: Update wiki with troubleshooting guide for common errors"),
        ],
    );

    themes.insert(
        CommitCategory::CelestialEvents,
        vec![
            commit_type!(
                "Solar Eclipse",
                "Sun Escapes, Legacy Code Lurks",
                "Temporarily masking functionality.",
                "Solar Eclipse(Feature): Temporarily disable new onboarding flow for testing"
            ),
            commit_type!(
                "Supernova",
                "Sudden Unbelievable Performance Revolution, New Version Arrives",
                "Major, transformative change or improvement.",
                "Supernova(Architecture): Migrate to microservices architecture"
            ),
            commit_type!(
                "Meteor Shower",
                "Many Edits, Tiny Overall Result, Overhaul Routines",
                "Series of small changes or fixes.",
                "Meteor Shower: Small alignment fixes"
            ),
            commit_type!(
                "Cosmic Dawn",
                "Creating Original, Simple, Minimal Initial Draft",
                "Initial implementation of a feature.",
                "Cosmic Dawn(Search): Initial implementation of basic search functionality"
            ),
            commit_type!(
                "Solar Storm",
                "Sudden Transformations Occur Rapidly, Modifications",
                "Rapid, impactful changes.",
                "Solar Storm(Refactor): Overhaul data processing pipeline for improved performance"
            ),
            commit_type!(
                "Lunar Transit",
                "Little Update, Now Adjustments Require Testing",
                "Minor, temporary change.",
                "Lunar Transit(Config): Temporarily adjust logging level for debugging"
            ),
            commit_type!(
                "Perihelion",
                "Perfect Ending, Refined, Improved, High Efficiency, Low Obstacles, Near Goal",
                "Significant milestone or feature completion.",
                "Perihelion: Successfully complete user acceptance testing for new dashboard"
            ),
            commit_type!(
                "Aphelion",
                "Away From Perfection, High Effort, Long Overhaul, Intense Overhaul, Obstacles",
                "Refactor, dependency update, or architecture change.",
                "Aphelion: Upgrade to React 18 and refactor components"
            ),
        ],
    );

    themes.insert(
        CommitCategory::CelestialObjects,
        vec![
            commit_type!(
                "White Dwarf",
                "Writing, Improving, Detailed Documentation For All",
                "Improving code comments or documentation",
                "White Dwarf(API): Add detailed documentation for new endpoints"
            ),
            commit_type!(
                "Red Giant",
                "Refactoring, Enhancing, Growing, Increasing, Adding New Things",
                "Expanding a feature or functionality",
                "Red Giant(Payments): Add support for Apple Pay and Google Pay"
            ),
            commit_type!(
                "Neutron Star",
                "New Efficient Utility, Tweaks, Robust Optimization, Nimble Solution",
                "Optimizing code for performance",
                "Neutron Star(Search): Optimize search algorithm for faster results"
            ),
            commit_type!(
                "Binary Star",
                "Bringing In New And Revised, Yielding Integrated Results",
                "Merging features or components",
                "Binary Star: Merge user authentication and authorization modules"
            ),
            commit_type!(
                "Brown Dwarf",
                "Barely Developed, Requires Work, Ongoing Development For Future",
                "Undeveloped feature with potential",
                "Brown Dwarf(Social): Initial prototype for social sharing feature"
            ),
            commit_type!(
                "Quark Star",
                "Questionable, Unstable, Anticipated Results, Risky, Keen Experiment",
                "Experimental or speculative change",
                "Quark Star(AI): Experiment with integrating GPT-3 for content generation"
            ),
            commit_type!(
                "Rogue Planet",
                "Refactoring Or Generating Operations, Unique Path, Leaping Ahead",
                "Independent change unrelated to the main codebase",
                "Rogue Planet: Create standalone script for data migration"
            ),
            commit_type!(
                "Stellar Nursery",
                "Starting To Enhance, Laying Layers, Launching New Requirements",
                "Creating new components",
                "Stellar Nursery(UI): Add new component library for design system"
            ),
            commit_type!(
                "Planetary Nebula",
                "Pruning, Leaving, Abandoning, Nostalgic Era, Totally Removed",
                "Removal or deprecation of a component",
                "Planetary Nebula: Remove legacy image carousel component"
            ),
            commit_type!(
                "Globular Cluster",
                "Gathering, Linking, Operations, Bringing Unity, Lots of Adjustments, All Related",
                "Collection of related changes",
                "Globular Cluster(Refactor): Refactor multiple API endpoints for consistency"
            ),
            commit_type!(
                "Void",
                "Vanished, Obliterated, Irrelevant, Deleted",
                "Removal of a module, component, or feature",
                "Void: Remove unused user settings module"
            ),
        ],
    );

    themes.insert(
        CommitCategory::AstronomicalConcepts,
        vec![
            commit_type!("Gravity", "Glitch Resolution, Adjusting Versions, Integrating, Troubleshooting Yielding", "Resolving merge conflicts or dependencies", "Gravity: Resolve merge conflicts in feature/new-navigation branch"),
            commit_type!("Dark Matter", "Debugging And Resolving Mysterious Attributes, Tricky issues Removed", "Fixing unknown or mysterious bugs", "Dark Matter: Fix intermittent crash on user login"),
            commit_type!("Time Dilation", "Time Is Dilated, Improvements Leverage Agility, Time-Saving", "Improving code performance or reducing execution time.", "Time Dilation(Backend): Optimize image processing algorithm for faster response"),
            commit_type!("Spacetime", "Scheduling, Planning, Adjusting Calendar Events, Coordinating Time", "Changes to date, time, or scheduling", "Spacetime(API): Fix timezone handling for event timestamps"),
            commit_type!("Gravitational Lensing", "Gravity Redirects Light, Altering Information Pathways", "Altering data or information flow", "Gravitational Lensing(Data): Refactor data pipeline for improved throughput"),
            commit_type!("Cosmic String", "Connecting Our Sections, Merging Together, Interlinking New Groups", "Connecting code parts", "Cosmic String(API): Connect user service with authentication middleware"),
            commit_type!("Quantum Fluctuation", "Quick Unpredictable Adjustments, Noticed Tiny Unexpected Modification", "Small, random change", "Quantum Fluctuation: Fix typo in error message"),
            commit_type!("Hawking Radiation", "Hastily And Willingly Killing Redundancies, Ageing Dead-ends, Tidying In Order, Obliterating Noise", "Removing technical debt", "Hawking Radiation: Remove unused CSS classes and refactor styles"),
            commit_type!("Quantum Entanglement", "Quantum Effects Never Tangled, Greater Efficiency, Linked Adjustments", "Establishing close relationships between code parts", "Quantum Entanglement(API): Tightly couple user profile and order history endpoints"),
            commit_type!("Gravitational Redshift", "Gravity Reduces Efficiency, Degraded Speed, Shift Happens", "Slowing down or reducing code performance", "Gravitational Redshift(UI): Disable unnecessary animations for low-end devices"),
        ],
    );

    themes.insert(
        CommitCategory::SpaceExploration,
        vec![
            commit_type!("Space Probe", "Surveying, Planning, Analysing, Checking Every Nook", "Testing new features or technologies", "Space Probe(AI): Experiment with ChatGPT integration for customer support"),
            commit_type!("Space Station", "Setting Up The Area, Testing In Orbit, Optimising New", "Creating or improving environments", "Space Station(DevOps): Set up new development environment with Docker"),
            commit_type!("Rocket Launch", "Releasing Our Code, Keenly Entering The Production", "Deploying to production", "Rocket Launch(v1.5): Deploy new version to production with enhanced security features"),
            commit_type!("Spacewalk", "Swift Patches And Lookout Work, Keeping Systems Extra safe", "Urgent production hotfixes", "Spacewalk(Database): Fix critical database connection issue causing downtime"),
            commit_type!("Space Elevator", "Streamlined Access, Providing Easy Vertical On boarding, Lifting Entries", "Making code base more accessible", "Space Elevator: Refactor README for onboarding"),
        ],
    );

    themes
}

fn ago(timestamp: &str) -> String {
    if let Ok(parsed_time) = chrono::DateTime::parse_from_rfc3339(timestamp) {
        let now = Local::now();
        let duration = now.signed_duration_since(parsed_time.with_timezone(&Local));
        if duration.num_seconds() < 60 {
            format!("{} seconds ago", duration.num_seconds())
        } else if duration.num_minutes() < 60 {
            format!("{} minutes ago", duration.num_minutes())
        } else if duration.num_hours() < 24 {
            format!("{} hours ago", duration.num_hours())
        } else if duration.num_days() < 30 {
            format!("{} days ago", duration.num_days())
        } else if duration.num_days() < 365 {
            format!("{} months ago", duration.num_days() / 30)
        } else {
            format!("{} years ago", duration.num_days() / 365)
        }
    } else {
        String::new()
    }
}

fn file_stats(
    f: &mut Formatter,
    prefix: &str,
    connector: &str,
    file: &str,
    added: usize,
    deleted: usize,
) -> std::fmt::Result {
    let total = added + deleted;
    const MAX_BAR_WIDTH: usize = 40; // La largeur maximale de ta barre de stats

    // Calcul du nombre de '+' et de '-' à afficher
    let (display_added, display_deleted) = if total > MAX_BAR_WIDTH {
        let factor = MAX_BAR_WIDTH as f64 / total as f64;
        let mut a = (added as f64 * factor).round() as usize;
        let mut d = (deleted as f64 * factor).round() as usize;

        // Sécurité pour corriger les micro-erreurs d'arrondi des f64
        while a + d > MAX_BAR_WIDTH {
            if a > d {
                a -= 1;
            } else {
                d -= 1;
            }
        }

        // Sécurité visuelle : si on a des ajouts/suppressions, on affiche au moins un caractère
        if a == 0 && added > 0 {
            a = 1;
        }
        if d == 0 && deleted > 0 {
            d = 1;
        }

        (a, d)
    } else {
        // Si ça rentre largement, on garde les vraies valeurs
        (added, deleted)
    };

    writeln!(
        f,
        "{prefix}{connector}{} {} {} {}{}",
        file,
        added.to_string().white(),
        deleted.to_string().white(),
        "+".repeat(display_added).green().bold(),
        "-".repeat(display_deleted).red().bold()
    )?;

    Ok(())
}

pub struct Log {
    pub author: String,
    pub message: String,
    pub at: String,
    pub signature: String,
    pub ticket_id: String,
    pub ticket: String,
    pub ticket_description: String,
    pub changes: Vec<(String, FileChange)>,
}

impl Display for Log {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let when = ago(self.at.as_str());
        let when_display = if when.is_empty() {
            self.at.as_str()
        } else {
            when.as_str()
        };
        writeln!(
            f,
            "Commit {}\nAuthor {}\nDate   {}\nTicket {}",
            self.signature, self.author, when_display, self.ticket
        )?;
        writeln!(f)?;
        writeln!(f, "{}\n", self.message.to_string().white().bold())?;
        writeln!(f, "Fixes : #{}", self.ticket_id)?;
        writeln!(f, "Title : {}", self.ticket)?;
        writeln!(f, "Descr : {}", self.ticket_description)?;
        writeln!(f, ".")?;
        let mut root = Tree::default();
        for (path, change) in &self.changes {
            let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            into_tree(&mut root, &parts, change.clone());
        }
        print_tree(f, &root, "", true)?;
        writeln!(f)?;
        Ok(())
    }
}

#[derive(Default)]
struct Tree {
    children: BTreeMap<String, Tree>,
    is_file: bool,
    change: Option<FileChange>,
}

fn into_tree(node: &mut Tree, parts: &[&str], change: FileChange) {
    if parts.is_empty() {
        return;
    }
    let first = parts[0];
    let child = node.children.entry(first.to_string()).or_default();
    if parts.len() == 1 {
        child.is_file = true;
        child.change = Some(change);
    } else {
        into_tree(child, &parts[1..], change);
    }
}
fn format_mode(mode: i64) -> String {
    let m = mode as u32;
    if (m & 0o170000) == 0o040000 {
        "d".to_string()
    } else {
        "f".to_string()
    }
}
fn print_tree(f: &mut Formatter<'_>, node: &Tree, prefix: &str, is_root: bool) -> std::fmt::Result {
    // On extrait les enfants dans un vecteur pour pouvoir les trier par type
    let mut children_vec: Vec<(&String, &Tree)> = node.children.iter().collect();

    // Tri : Dossiers d'abord, puis ordre alphabétique
    children_vec.sort_by(|a, b| {
        let a_is_dir = !a.1.is_file;
        let b_is_dir = !b.1.is_file;

        if a_is_dir == b_is_dir {
            a.0.cmp(b.0) // Si même type, tri alphabétique
        } else {
            b_is_dir.cmp(&a_is_dir) // true (dossier) arrive avant false (fichier)
        }
    });

    let len = children_vec.len();

    // On itère sur le vecteur trié (attention, enumerate commence à 0)
    for (i, (name, child)) in children_vec.into_iter().enumerate() {
        let is_last = i == len - 1;
        let connector = if is_last { "└──" } else { "├──" };

        if child.is_file {
            // Affiche le marqueur et les compteurs
            let marker: (String, usize, usize) = match &child.change {
                Some(FileChange::Added { added, mode }) => {
                    let m = mode.map(format_mode).unwrap_or_default();
                    (
                        format!("{} {}", m.white(), name.clone().white().bold()),
                        *added,
                        0,
                    )
                }
                Some(FileChange::Deleted { deleted, mode }) => {
                    let m = mode.map(format_mode).unwrap_or_default();
                    (
                        format!("{} {}", m.white(), name.clone().white().bold()),
                        0,
                        *deleted,
                    )
                }
                Some(FileChange::Modified {
                    added,
                    deleted,
                    mode,
                }) => {
                    let m = mode.map(format_mode).unwrap_or_default();
                    (
                        format!("{} {}", m.white(), name.clone().white().bold()),
                        *added,
                        *deleted,
                    )
                }
                _ => (String::new(), 0, 0),
            };
            file_stats(f, prefix, connector, marker.0.as_str(), marker.1, marker.2)?;
        } else {
            writeln!(f, "{prefix}{connector} {}", name.to_string().blue().bold())?;
        }

        let new_prefix = if is_last {
            format!("{}    ", prefix)
        } else {
            format!("{}│   ", prefix)
        };

        print_tree(f, child, &new_prefix, false)?;
    }

    if is_root && len == 0 {
        // nothing to print
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub enum FileChange {
    Added {
        added: usize,
        mode: Option<i64>,
    },
    Deleted {
        deleted: usize,
        mode: Option<i64>,
    },
    Modified {
        added: usize,
        deleted: usize,
        mode: Option<i64>,
    },
}

pub fn author() -> String {
    let u = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string());

    #[cfg(unix)]
    {
        // Sur Unix, on tente de récupérer le "Real Name" (GECOS)
        if let Ok(Some(user)) = User::from_name(u.as_str()) {
            let gecos = user.gecos.to_string_lossy().to_string();
            if !gecos.is_empty() {
                return gecos;
            }
        }
    }
    u
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct Commit {
    pub category: CommitCategory,
    pub types: CommitType,
    pub os: String,
    pub ticket: TodoItem,
    pub os_release: String,
    pub os_version: String,
    pub os_domain: String,
    pub machine: String,
    pub arch: String,
    pub summary: String,
    pub why: String,
    pub who: String,
    pub src: String,
    pub how: String,
    pub when: String,
    pub what: String,
    pub where_path: Vec<String>,
    pub outcome: String,
    pub impact: String,
    pub breaking_changes: String,
}

impl Display for Commit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}\n", format_args!("{}", self.summary.trim_end()))?;
        writeln!(
            f,
            "Os    : {} {} {}",
            self.os.as_str(),
            self.os_release,
            self.arch
        )?;
        writeln!(f)?;
        writeln!(f, "{}", "What?".bold())?;
        writeln!(f)?;
        writeln!(
            f,
            "{}",
            format_justified_with_newlines(self.what.as_str()).white()
        )?;

        writeln!(f)?;
        writeln!(f, "{}", "Why?".bold())?;
        writeln!(f)?;
        writeln!(
            f,
            "{}",
            format_justified_with_newlines(self.why.as_str()).white()
        )?;
        writeln!(f)?;
        writeln!(f, "{}", "How?".bold())?;
        writeln!(f)?;
        writeln!(
            f,
            "{}",
            format_justified_with_newlines(self.how.as_str()).white()
        )?;
        writeln!(f)?;
        writeln!(f, "{}", "Breaking Changes?".bold())?;
        writeln!(f)?;
        writeln!(
            f,
            "{}",
            format_justified_with_newlines(self.breaking_changes.as_str()).white()
        )?;
        Ok(())
    }
}
impl Commit {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn ask_ticket(&mut self) -> TodoItem {
        let x = todos().await;
        if x.is_empty() {
            panic!("create a ticket first");
        }
        Select::new("Resolves ticket:", x.clone())
            .prompt()
            .expect("")
    }

    ///
    /// Commit the changes to the repository
    ///
    /// # Errors
    ///
    /// On bad user inputs
    ///
    pub async fn commit(&mut self) -> bool {
        hooks(
            |c| {
                c.ask_category()
                    .expect("")
                    .ask_types()
                    .expect("")
                    .ask_summary()
                    .expect("")
                    .ask_what()
                    .expect("")
                    .ask_how()
                    .expect("")
                    .ask_benefits()
                    .expect("")
                    .ask_breaking()
                    .expect("")
                    .ask_why()
                    .expect("")
                    .human_and_system()
                    .expect("")
                    .to_string()
            },
            self,
        )
        .await
    }

    ///
    /// # Errors
    ///
    /// On bad user inputs
    ///
    pub fn ask_types(&mut self) -> InquireResult<&mut Self> {
        let x = get_space_themes();
        let y = x.get(&self.category).expect("a");
        self.types = Select::new("commit types", y.clone()).prompt()?;
        Ok(self)
    }

    ///
    /// # Errors
    ///
    /// On bad user inputs
    ///
    pub fn ask_summary(&mut self) -> InquireResult<&mut Self> {
        self.summary.clear();
        while self.summary.is_empty() {
            self.summary.clear();
            self.summary
                .push_str(Text::new("Commit summary:").prompt()?.as_str());
        }
        if self.summary.is_empty() {
            return Err(InquireError::from(Error::other("bad summary")));
        }
        Ok(self)
    }
    ///
    /// # Errors
    ///
    /// On bad user inputs
    ///
    pub fn ask_category(&mut self) -> InquireResult<&mut Self> {
        self.category = Select::new("", CommitCategory::all()).prompt()?;
        Ok(self)
    }

    ///
    /// # Errors
    ///
    /// On bad user inputs
    ///
    pub fn ask_breaking(&mut self) -> InquireResult<&mut Self> {
        self.breaking_changes.clear();
        while self.breaking_changes.is_empty() {
            self.breaking_changes.clear();
            self.breaking_changes
                .push_str(Editor::new("Breaking Changes?").prompt()?.as_str());
        }
        if self.breaking_changes.is_empty() {
            return Err(InquireError::from(Error::other("bad changes")));
        }
        Ok(self)
    }

    ///
    /// Why are you making these changes?
    ///
    /// # Errors
    ///
    /// On bad user inputs
    ///
    pub fn ask_what(&mut self) -> InquireResult<&mut Self> {
        self.what.clear();
        while self.what.is_empty() {
            self.what.clear();
            self.what.push_str(Editor::new(WHAT).prompt()?.as_str());
        }
        if self.what.is_empty() {
            return Err(InquireError::from(Error::other("bad what")));
        }
        Ok(self)
    }

    ///
    /// Why are you making these changes?
    ///
    /// # Errors
    ///
    /// On bad user inputs
    ///
    pub fn ask_why(&mut self) -> InquireResult<&mut Self> {
        self.why.clear();
        while self.why.is_empty() {
            self.why.clear();
            self.why.push_str(Editor::new(WHY).prompt()?.as_str());
        }
        if self.why.is_empty() {
            return Err(InquireError::from(Error::other("bad why")));
        }
        Ok(self)
    }

    ///
    /// Why are you making these changes?
    ///
    /// # Errors
    ///
    /// On bad user inputs
    ///
    pub fn ask_how(&mut self) -> InquireResult<&mut Self> {
        self.how.clear();
        while self.how.is_empty() {
            self.how.clear();
            self.how.push_str(Editor::new(HOW).prompt()?.as_str());
        }
        if self.how.is_empty() {
            return Err(InquireError::from(Error::other("bad how")));
        }
        Ok(self)
    }

    pub fn human_and_system(&mut self) -> InquireResult<&mut Self> {
        self.os.clear();
        self.os_version.clear();
        self.os_release.clear();
        self.os_domain.clear();
        self.machine.clear();
        self.arch.clear();
        self.who.clear();
        self.when.clear();
        self.arch.push_str(ARCH);
        self.when.push_str(
            Local::now()
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
                .as_str(),
        );
        #[cfg(unix)]
        {
            let o = uname().expect("failed");
            self.os
                .push_str(o.sysname().to_str().expect("").to_string().as_str());
            self.machine
                .push_str(o.machine().to_str().expect("").to_string().as_str());
            self.os_release
                .push_str(o.release().to_str().expect("").to_string().as_str());
            self.os_version
                .push_str(o.version().to_str().expect("").to_string().as_str());
            self.os_domain
                .push_str(o.nodename().to_str().expect("").to_string().as_str());
        }
        #[cfg(windows)]
        {
            let os_name = std::env::consts::OS;
            let os_release = std::env::var("OS").unwrap_or_else(|_| "Windows".to_string());
            let machine = std::env::var("COMPUTERNAME").unwrap_or_default();
            let domain = std::env::var("USERDOMAIN").unwrap_or_default();
            self.os.push_str(os_name);
            self.machine.push_str(machine.as_str());
            self.os_release.push_str(os_release.as_str());
            self.os_version.push_str(os_release.as_str());
            self.os_domain.push_str(domain.as_str());
        }
        self.who.push_str(author().as_str());
        Ok(self)
    }

    ///
    /// What code resolve
    ///
    /// # Errors
    ///
    /// On bad user inputs
    ///
    pub fn ask_benefits(&mut self) -> InquireResult<&mut Self> {
        self.outcome.clear();
        while self.outcome.is_empty() {
            self.outcome.clear();
            self.outcome
                .push_str(Editor::new(OUTCOME_PROMPT).prompt()?.as_str());
        }
        if self.outcome.is_empty() {
            return Err(InquireError::from(Error::other("bad outcome")));
        }
        Ok(self)
    }
}
