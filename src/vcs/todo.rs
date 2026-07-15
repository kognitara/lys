use crate::vcs::db::{self, fetch};
use crate::vcs::ok;
use sqlx::Row;
use std::fmt::Display;
use std::io::Error;
use tabled::Tabled;
use tabled::builder::Builder;
use tabled::settings::Style;

#[derive(Tabled, Clone, Debug, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct TodoItem {
    #[tabled(rename = "ID")]
    pub id: i64,
    #[tabled(rename = "Title")]
    pub title: String,
    #[tabled(rename = "Description")]
    pub description: String,
    #[tabled(rename = "Status")]
    pub status: String,
    #[tabled(rename = "Assigned to")]
    pub assigned_to: String,
    #[tabled(rename = "Due date")]
    pub due_date: String,
}

impl Display for TodoItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let in_time = if self.due_date == "No limit" {
            "No limit".to_string()
        } else {
            let due_date = chrono::NaiveDate::parse_from_str(&self.due_date, "%Y-%m-%d")
                .unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
            let today = chrono::Local::now().date_naive();
            if due_date < today {
                let days = (today - due_date).num_days();
                if days == 1 {
                    "Overdue by 1 day".to_string()
                } else {
                    format!("Overdue by {} days", days)
                }
            } else {
                let days = (due_date - today).num_days();
                if days == 0 {
                    "Due today".to_string()
                } else if days == 1 {
                    "Due tomorrow".to_string()
                } else {
                    format!("Just in time by {days} days")
                }
            }
        };
        write!(
            f,
            "ID: {}, Title: {}, Status: {}, Assigned to: {}, Due date: {}, In time: {}",
            self.id, self.title, self.status, self.assigned_to, self.due_date, in_time
        )
    }
}

pub async fn awq_start_todo(id: i64) -> Result<(), Error> {
    sqlx::query("UPDATE todos SET status = 'IN_PROGRESS' WHERE id = ?")
        .bind(id)
        .execute(&db::conn().await)
        .await
        .expect("Failed to update task status");
    ok(format!("Task #{id} is now in progress").as_str());
    Ok(())
}

// Dans src/vcs/todo.rs
pub async fn awq_add_todo(
    title: &str,
    description: &str,
    reporter_id: i64,
    assignee_id: i64, // <-- Ajout du développeur assigné
    due_date: &str,
) -> bool {
    sqlx::query(
        "INSERT INTO todos (title, description, reporter_id, assignee_id, due_date) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(title)
    .bind(description)
    .bind(reporter_id)
    .bind(assignee_id) // <-- On le lie dans la DB
    .bind(due_date)
    .execute(&db::conn().await)
    .await
    .is_ok()
}
pub async fn awq_list_todos() -> bool {
    let tasks = todos().await;
    if tasks.is_empty() {
        ok("No pending tasks. You're all caught up!");
        true
    } else {
        let mut table = Builder::default();
        table.push_record(["id", "title", "status", "due"]);
        for task in &tasks {
            table.push_record([
                task.id.to_string(),
                task.title.to_string(),
                task.status.to_string(),
                task.due_date.to_string(),
            ]);
        }
        println!("{}", table.build().with(Style::modern()));
        true
    }
}

pub async fn todos() -> Vec<TodoItem> {
    let mut x: Vec<TodoItem> = Vec::new();

    // Jointure avec la table users pour récupérer le nom du développeur assigné
    let query = "
        SELECT 
            t.id, 
            t.title, 
            t.description, 
            t.status, 
            IFNULL(u.name, 'None') AS assignee_name, 
            IFNULL(t.due_date, 'No limit') 
        FROM todos t
        LEFT JOIN users u ON t.assignee_id = u.id
        WHERE t.status != 'DONE' 
        ORDER BY t.due_date ASC
    ";

    for todo in &fetch(query).await {
        x.push(TodoItem {
            id: todo.get(0),
            title: todo.get(1),
            description: todo.get(2),
            status: todo.get(3),
            assigned_to: todo.get(4), // Ce champ reçoit maintenant 'u.name' proprement
            due_date: todo.get(5),
        });
    }
    x
}

pub async fn awq_complete_todo(id: i64) -> bool {
    sqlx::query("UPDATE todos SET status = 'DONE' WHERE id = ?")
        .bind(id)
        .execute(&db::conn().await)
        .await
        .is_ok()
}
