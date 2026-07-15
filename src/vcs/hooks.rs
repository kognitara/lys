use crate::vcs::db::conn;
use crate::vcs::{ko, ok};
use ignore::WalkBuilder;
use inquire::{Confirm, Select, Text};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::fmt::Display;
use std::fs;
use std::path::Path;
use tabled::settings::Style;
use tabled::{Table, Tabled};

pub const AWQ_HOOKS_EXPORT_FILENAME: &str = "awq_hooks.json";

pub const AWQ_HOOKS_RUST_CONTENT: &str = include_str!("../../hooks/rust/awq.json");
pub const AWQ_HOOKS_CMAKE_CONTENT: &str = include_str!("../../hooks/cmake/awq.json");
pub const AWQ_HOOKS_C_CONTENT: &str = include_str!("../../hooks/c/awq.json");
pub const AWQ_HOOKS_CXX_CONTENT: &str = include_str!("../../hooks/cxx/awq.json");
pub const AWQ_HOOKS_PYTHON_CONTENT: &str = include_str!("../../hooks/python/awq.json");
pub const AWQ_HOOKS_D_CONTENT: &str = include_str!("../../hooks/d/awq.json");
pub const AWQ_HOOKS_JS_CONTENT: &str = include_str!("../../hooks/js/awq.json");
pub const AWQ_HOOKS_TS_CONTENT: &str = include_str!("../../hooks/ts/awq.json");
pub const AWQ_HOOKS_PHP_CONTENT: &str = include_str!("../../hooks/php/awq.json");
pub const AWQ_HOOKS_GO_CONTENT: &str = include_str!("../../hooks/go/awq.json");
pub const AWQ_HOOKS_RUBY_CONTENT: &str = include_str!("../../hooks/ruby/awq.json");

pub const AWQ_HOOKS_PURPOSED: [(&str, &str, &str); 14] = [
    ("Gemfile", "Ruby", AWQ_HOOKS_RUBY_CONTENT),
    ("Cargo.toml", "Rust", AWQ_HOOKS_RUST_CONTENT),
    ("CMakeLists.txt", "CMake", AWQ_HOOKS_CMAKE_CONTENT),
    // Pour C et C++, on utilise souvent des Makefiles classiques ou des fichiers de formatage
    //
    ("Makefile", "C/C++", AWQ_HOOKS_C_CONTENT),
    (".clang-format", "C/C++", AWQ_HOOKS_CXX_CONTENT),
    // Python a deux standards majeurs aujourd'hui
    ("pyproject.toml", "Python", AWQ_HOOKS_PYTHON_CONTENT),
    ("requirements.txt", "Python", AWQ_HOOKS_PYTHON_CONTENT),
    // D
    ("dub.json", "D", AWQ_HOOKS_D_CONTENT),
    ("dub.sdl", "D", AWQ_HOOKS_D_CONTENT),
    // L'écosystème web
    ("package.json", "JavaScript", AWQ_HOOKS_JS_CONTENT),
    ("tsconfig.json", "TypeScript", AWQ_HOOKS_TS_CONTENT),
    // PHP
    ("composer.json", "PHP", AWQ_HOOKS_PHP_CONTENT),
    // Go (Le fichier s'appelle toujours go.mod)
    ("go.mod", "Go", AWQ_HOOKS_GO_CONTENT),
    ("go.work", "Go", AWQ_HOOKS_GO_CONTENT),
];

#[derive(Tabled, Clone, Debug, Serialize, Deserialize)]
pub struct HookItem {
    #[serde(skip)] // On ne veut pas exporter/importer l'ID local
    #[tabled(rename = "ID")]
    pub id: i64,

    #[tabled(rename = "Name")]
    pub name: String,

    #[tabled(rename = "Trigger")]
    pub trigger: String,

    #[tabled(rename = "Command")]
    pub command: String,

    #[tabled(rename = "Active")]
    pub is_active: bool,
}

// Implémentation de Display pour que inquire puisse l'afficher dans ses listes de sélection
impl Display for HookItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = if self.is_active { "A" } else { "Z" };
        write!(
            f,
            "{} {} [{}] -> `{}`",
            status, self.name, self.trigger, self.command
        )
    }
}
pub async fn export_hooks() -> anyhow::Result<()> {
    let hooks = get_all_hooks().await;
    if hooks.is_empty() {
        ko("No hooks available to export.");
        return Ok(());
    }
    let json_data = serde_json::to_string_pretty(&hooks).expect("");

    fs::write(AWQ_HOOKS_EXPORT_FILENAME, json_data)?;
    ok(&format!(
        "Hooks exported successfully to `{AWQ_HOOKS_EXPORT_FILENAME}`",
    ));
    Ok(())
}
pub async fn import_hooks() -> anyhow::Result<()> {
    let mut file_path = String::new();
    while Path::new(file_path.as_str()).is_file().eq(&false) {
        file_path.clear();
        file_path = Text::new("Path to hooks JSON file:")
            .with_default(AWQ_HOOKS_EXPORT_FILENAME)
            .prompt()?;
        if Path::new(file_path.as_str()).extension().is_none()
            || Path::new(file_path.as_str())
                .extension()
                .unwrap()
                .to_str()
                .ne(&Some("json"))
        {
            file_path.clear();
            ko("must be a json file");
        } else {
            break;
        }
    }
    let content = match fs::read_to_string(&file_path) {
        Ok(c) => c,
        Err(_) => {
            ko(&format!("Could not read file `{file_path}`"));
            return Ok(());
        }
    };

    let imported_hooks: Vec<HookItem> = match serde_json::from_str(&content) {
        Ok(h) => h,
        Err(e) => {
            ko(&format!("Invalid JSON format: {e}"));
            return Ok(());
        }
    };

    let pool = conn().await;
    let mut added_count = 0;
    let mut failed_count = 0;

    for hook in &imported_hooks {
        let result = sqlx::query(
            "INSERT INTO hooks (name, trigger, command, is_active) VALUES (?, ?, ?, ?)",
        )
        .bind(&hook.name)
        .bind(&hook.trigger)
        .bind(&hook.command)
        .bind(hook.is_active)
        .execute(&pool)
        .await;

        if result.is_ok() {
            added_count += 1;
        } else {
            failed_count += 1;
        }
    }
    if failed_count > 0 {
        ko(format!(
            "Imported {added_count}/{} hooks, Failed to import {failed_count} hooks from `{file_path}`",imported_hooks.len()
        )
        .as_str());
    } else {
        ok(format!(
            "Successfully imported {added_count}/{} hooks from `{file_path}`",
            imported_hooks.len()
        )
        .as_str());
    }
    Ok(())
}
/// Récupère tous les hooks depuis la base de données
pub async fn get_all_hooks() -> Vec<HookItem> {
    let pool = conn().await;
    let rows = sqlx::query("SELECT id, name, trigger, command, is_active FROM hooks")
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

    let mut items = Vec::new();
    for row in rows {
        items.push(HookItem {
            id: row.get(0),
            name: row.get(1),
            trigger: row.get(2),
            command: row.get(3),
            is_active: row.get(4),
        });
    }
    items
}

/// Scanne la racine du projet et propose d'installer les hooks adaptés
pub async fn awq_auto_setup_hooks() -> anyhow::Result<()> {
    let pool = conn().await;

    // 1. Configurer le scanner pour ne lire QUE la racine
    let walker = WalkBuilder::new(".")
        .max_depth(Some(1)) // La magie est ici : profondeur maximale de 1 (racine uniquement)
        .hidden(false) // Important pour détecter les fichiers comme .clang-format
        .build();

    // 2. Récolter les noms des fichiers présents à la racine
    let mut root_files = Vec::new();
    for result in walker.flatten() {
        if let Some(file_name) = result.file_name().to_str() {
            root_files.push(file_name.to_string());
        }
    }

    let mut total_added = 0;

    // 3. Parcourir notre tableau de langages
    for (file_trigger, lang, json_content) in AWQ_HOOKS_PURPOSED {
        if root_files.contains(&file_trigger.to_string()) {
            println!();

            // On a trouvé un marqueur ! On demande confirmation au développeur
            let msg = format!(
                "Projet {lang} detected (file `{file_trigger}`). Install recommended hooks ?",
            );
            let ans = Confirm::new(&msg).with_default(true).prompt()?;

            if ans {
                let hooks: Vec<HookItem> = match serde_json::from_str(json_content) {
                    Ok(h) => h,
                    Err(e) => {
                        ko(&format!("Error JSON for {lang}: {e}"));
                        continue;
                    }
                };

                let mut count = 0;
                for hook in hooks {
                    let res = sqlx::query(
                        "INSERT INTO hooks (name, trigger, command, is_active) VALUES (?, ?, ?, ?)",
                    )
                    .bind(&hook.name)
                    .bind(&hook.trigger)
                    .bind(&hook.command)
                    .bind(hook.is_active)
                    .execute(&pool)
                    .await;

                    if res.is_ok() {
                        count += 1;
                    }
                }
                ok(format!("{count} hooks installed for {lang}.").as_str());
                total_added += count;
            }
        }
    }

    if total_added == 0 {
        ok("No hooks installed for your project.");
    } else {
        println!();
        ok(&format!(
            "End of analyze. {total_added} hooks has been added.",
        ));
    }

    Ok(())
}

/// Affiche la liste sous forme de tableau
pub async fn list_hooks() {
    let hooks = get_all_hooks().await;
    if hooks.is_empty() {
        ko("No hooks configured yet.");
    } else {
        let mut table = Table::new(&hooks);
        println!("{}", table.with(Style::modern()));
    }
}

/// Ajoute un nouveau hook
pub async fn add_hook() -> anyhow::Result<()> {
    let name = Text::new("Hook name (ex: Format & Lint):").prompt()?;

    let triggers = vec!["pre-commit", "post-commit", "pre-push", "post-merge"];
    let trigger = Select::new("Trigger event:", triggers).prompt()?;

    let command = Text::new("Command to execute (ex: cargo fmt --check):").prompt()?;

    let pool = conn().await;
    sqlx::query("INSERT INTO hooks (name, trigger, command, is_active) VALUES (?, ?, ?, 1)")
        .bind(name)
        .bind(trigger)
        .bind(command)
        .execute(&pool)
        .await?;

    ok("Hook added successfully.");
    Ok(())
}

/// Active ou désactive un hook existant
pub async fn toggle_hook() -> anyhow::Result<()> {
    let hooks = get_all_hooks().await;
    if hooks.is_empty() {
        ko("No hooks available to toggle.");
        return Ok(());
    }

    let selected = Select::new("Select hook to toggle:", hooks).prompt()?;
    let new_status = !selected.is_active;

    let pool = conn().await;
    sqlx::query("UPDATE hooks SET is_active = ? WHERE id = ?")
        .bind(new_status)
        .bind(selected.id)
        .execute(&pool)
        .await?;

    let status_str = if new_status { "activated" } else { "disabled" };
    ok(&format!("Hook '{}' is now {}.", selected.name, status_str));
    Ok(())
}

/// Supprime un hook
pub async fn delete_hook() -> anyhow::Result<()> {
    let hooks = get_all_hooks().await;
    if hooks.is_empty() {
        ko("No hooks available to delete.");
        return Ok(());
    }

    let selected = Select::new("Select hook to delete:", hooks).prompt()?;

    let ans = Confirm::new(&format!(
        "Are you sure you want to delete '{}'?",
        selected.name
    ))
    .with_default(false)
    .prompt()?;

    if ans {
        let pool = conn().await;
        sqlx::query("DELETE FROM hooks WHERE id = ?")
            .bind(selected.id)
            .execute(&pool)
            .await?;
        ok("Hook deleted successfully.");
    } else {
        ko("Deletion aborted.");
    }

    Ok(())
}

pub enum Trigger {
    PreCommit,
    PostCommit,
    PrePush,
    PostPush,
}

pub async fn group_hooks(trigger: Trigger) -> Vec<HookItem> {
    let pool = conn().await;
    let t = match trigger {
        Trigger::PreCommit => "pre-commit",
        Trigger::PostCommit => "post-commit",
        Trigger::PrePush => "pre-push",
        Trigger::PostPush => "post-push",
    };
    let rows = sqlx::query("SELECT id, name, command, is_active FROM hooks WHERE trigger = ?")
        .bind(t)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

    let mut items = Vec::new();
    for row in rows {
        items.push(HookItem {
            id: row.get(0),
            name: row.get(1),
            command: row.get(2),
            is_active: row.get(3),
            trigger: t.to_string(),
        });
    }
    items
}

/// Le menu interactif principal
pub async fn awq_hooks_menu() -> anyhow::Result<()> {
    let options = vec![
        "Auto-detect & Install Hooks",
        "List Hooks",
        "Add Hook",
        "Toggle Hook Status (Enable/Disable)",
        "Delete Hook",
        "Export Hooks to JSON",
        "Import Hooks from JSON",
        "Exit",
    ];

    loop {
        println!();
        let choice = Select::new("Select an option:", options.clone()).prompt()?;

        match choice {
            "Auto-detect & Install Hooks" => {
                if let Err(e) = awq_auto_setup_hooks().await {
                    ko(&format!("Error: {e}"));
                }
            }
            "List Hooks" => list_hooks().await,
            "Add Hook" => {
                if let Err(e) = add_hook().await {
                    ko(&format!("Error: {e}"));
                }
            }
            "Toggle Hook Status (Enable/Disable)" => {
                if let Err(e) = toggle_hook().await {
                    ko(&format!("Error: {e}"));
                }
            }
            "Delete Hook" => {
                if let Err(e) = delete_hook().await {
                    ko(&format!("Error: {e}"));
                }
            }
            "Export Hooks to JSON" => {
                if let Err(e) = export_hooks().await {
                    ko(&format!("Error: {e}"));
                }
            }
            "Import Hooks from JSON" => {
                if let Err(e) = import_hooks().await {
                    ko(&format!("Error: {e}"));
                }
            }
            "Exit" => break,
            _ => {}
        }
    }
    Ok(())
}
