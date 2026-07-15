use sqlx::Row;
use sqlx::SqlitePool;
use sqlx::query;
use sqlx::sqlite::SqliteRow;
pub const AWQ_DB_PATH: &str = ".awq/awq.db";

pub async fn conn() -> SqlitePool {
    SqlitePool::connect("sqlite://.awq/awq.db")
        .await
        .expect("no database file")
}
pub async fn fetch(sql: &'static str) -> Vec<SqliteRow> {
    query(sql).fetch_all(&conn().await).await.expect("")
}

pub async fn get_or_create_user(pool: &SqlitePool, name: &str) -> Result<i64, sqlx::Error> {
    let row = sqlx::query("SELECT id FROM users WHERE name = ?")
        .bind(name)
        .fetch_optional(pool)
        .await?;

    if let Some(r) = row {
        Ok(r.get(0))
    } else {
        // L'utilisateur n'existe pas encore, on le crée à la volée !
        let email = format!("{}@awq.local", name);
        // Remplace 'developer' par 'fullstack'
        let new_user = sqlx::query(
            "INSERT INTO users (name, email, default_role) VALUES (?, ?, 'fullstack') RETURNING id",
        )
        .bind(name)
        .bind(email)
        .fetch_one(pool)
        .await?;
        Ok(new_user.get(0))
    }
}
