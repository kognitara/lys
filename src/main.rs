use crate::vcs::{
    branch::list_branches,
    commit::{Commit, awq_log, awq_status},
    hooks::awq_hooks_menu,
    init::init_awq,
    keys::awq_audit,
    ko_commit_sent, locale, ok_commit_sent,
    todo::{awq_add_todo, awq_list_todos, awq_start_todo},
};
use chrono::Local;
use clap::{Arg, Command};
use inquire::{DateSelect, Editor, Text};
use sqlx::Row;
use std::process::ExitCode;

mod health;
mod vcs;
fn cli() -> Command {
    clap::Command::new(env!("CARGO_PKG_NAME"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .subcommand(Command::new("init").about("Initialize the awq repository"))
        .subcommand(Command::new("keygen").about("Initialize the awq sign keys"))
        .subcommand(Command::new("audit").about("Verify integrity of commit signatures"))
        .subcommand(Command::new("commit").about("Run the interactive commit prompt"))
        .subcommand(Command::new("status").about("Show changes in working directory"))
        .subcommand(Command::new("log").about("View the commit history"))
        .subcommand(Command::new("hooks").about("Manage hooks interactively"))
        .subcommand(Command::new("health").about("Run all hooks without commit"))
        .subcommand(Command::new("push").about("Push modifications"))
        .subcommand(
            Command::new("branch")
                .about("Branches management")
                .subcommands([
                    Command::new("list").about("List all branches"),
                    Command::new("create").about("create a new ephemeral branch"),
                ]),
        )
        .subcommand(
            Command::new("todo")
                .about("Manage project internal todos")
                .subcommand(Command::new("list").about("List all active todos"))
                .subcommand(Command::new("add").about("Add a new todo"))
                .subcommand(
                    Command::new("start")
                        .about("Mark a todo as in progress")
                        .arg(Arg::new("id").required(true)),
                ),
        )
}

#[tokio::main]
async fn main() -> ExitCode {
    let app = cli().get_matches();
    match app.subcommand() {
        Some(("init", _)) => {
            if init_awq(&locale()).await {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Some(("branch", sub)) => match sub.subcommand() {
            Some(("list", _)) => {
                if list_branches().await {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                }
            }
            Some(("create", _)) => {
                if crate::vcs::branch::create_branch().await.is_ok() {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                }
            }
            _ => {
                cli().print_help().expect("failed to print help");
                ExitCode::FAILURE
            }
        },
        Some(("status", _)) => {
            if awq_status().await.is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Some(("health", _)) => {
            if health::check().await {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Some(("audit", _)) => {
            if awq_audit().await {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Some(("push", _)) => {
            let pool = crate::vcs::db::conn().await; // Récupère la connexion SQLx locale
            let mut failures = 0;
            // 1. On va chercher tous les commits non envoyés
            let rows = crate::vcs::db::fetch("SELECT id, hash FROM commits WHERE sent = 0").await;

            for row in &rows {
                let commit_id: i64 = row.get(0);
                let commit_hash: String = row.get(1);

                // 2. Tenter l'envoi au Hub Rails
                if crate::vcs::sync::CommitPayload::new(commit_id)
                    .await
                    .push()
                    .await
                {
                    ok_commit_sent(&commit_hash[0..7]);
                    let update_res = sqlx::query("UPDATE commits SET sent = 1 WHERE id = ?")
                        .bind(commit_id)
                        .execute(&pool)
                        .await;

                    match update_res {
                        Ok(_) => {
                            crate::vcs::ok(&format!(
                                "Commit [{}] sent and marked as synchronized locally!",
                                &commit_hash[0..7]
                            ));
                        }
                        Err(e) => {
                            failures += 1;
                            crate::vcs::ko(&format!(
                                "Commit sent but failed to update local DB status: {e}"
                            ));
                        }
                    }
                } else {
                    failures += 1;
                    ko_commit_sent(&commit_hash[0..7]);
                }
            }
            if failures > 0 {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Some(("commit", _)) => {
            if Commit::new().commit().await {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Some(("todo", sub_matches)) => match sub_matches.subcommand() {
            Some(("list", _)) => {
                awq_list_todos().await;
                ExitCode::SUCCESS
            }
            Some(("add", _)) => {
                let title = Text::new("Todo title:").prompt().unwrap_or_default();
                let description = Editor::new("Todo description:")
                    .prompt()
                    .unwrap_or_default();

                let assignee_input = Text::new("Assign to")
                    .with_default("@me")
                    .prompt()
                    .unwrap_or_default();

                let due = DateSelect::new("Due date:")
                    .with_default(Local::now().date_naive())
                    .prompt()
                    .unwrap_or_default();

                // 1. Déterminer les vrais noms
                let reporter_name = crate::vcs::commit::author();
                let assignee_name = if assignee_input == "@me" {
                    reporter_name.clone() // Si "@me", c'est le rapporteur qui s'en charge
                } else {
                    assignee_input
                };

                // 2. Convertir les noms en IDs via notre nouvelle fonction !
                let pool = crate::vcs::db::conn().await;
                let reporter_id = crate::vcs::db::get_or_create_user(&pool, &reporter_name)
                    .await
                    .expect("Failed to get/create reporter");
                let assignee_id = crate::vcs::db::get_or_create_user(&pool, &assignee_name)
                    .await
                    .expect("Failed to get/create assignee");

                // 3. Créer le ticket proprement
                awq_add_todo(
                    title.as_str(),
                    description.as_str(),
                    reporter_id,
                    assignee_id,
                    due.to_string().as_str(),
                )
                .await;
                ExitCode::SUCCESS
            }
            Some(("start", m)) => {
                let id_str = m.get_one::<String>("id").unwrap();
                if let Ok(id) = id_str.parse::<i64>() {
                    let _ = awq_start_todo(id).await;
                }
                ExitCode::SUCCESS
            }
            _ => ExitCode::FAILURE,
        },
        Some(("hooks", _)) => {
            if awq_hooks_menu().await.is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Some(("log", _)) => {
            if awq_log().await.is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        _ => {
            cli().print_help().expect("failed to print help");
            ExitCode::FAILURE
        }
    }
}
