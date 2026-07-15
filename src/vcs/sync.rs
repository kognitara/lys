use chrono::NaiveDateTime;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use std::{env::current_dir, io::Read};

#[derive(Serialize, Deserialize, Default)]
pub struct CommitPayload {
    project: String,
    hash: String,
    tree_hash: String,
    parent_hash: Option<String>,
    message: String,
    author: String,
    timestamp: NaiveDateTime,
    signature: String,
    ticket: String,
    public_key: String,
}

impl CommitPayload {
    pub async fn new(id: i64) -> Self {
        let pool = crate::vcs::db::conn().await;
        let row = sqlx::query(
            "SELECT hash, parent_hash, tree_hash, author, message, ticket, timestamp, signature FROM commits WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .expect("");
        let config_row = sqlx::query("SELECT value FROM config WHERE key = 'project_name'")
            .fetch_optional(&pool)
            .await
            .expect("DB error");

        let project_name = match config_row {
            Some(r) => r.get(0),
            None => current_dir().expect("").to_str().expect("").to_string(),
        };
        // Lire ta clé publique locale (comme dans ton module keys.rs)
        let pub_key_path = std::path::Path::new(".awq/keys/public.key");
        let mut pub_file = std::fs::File::open(pub_key_path).expect("Clé publique introuvable");
        let mut pub_bytes = [0u8; 32];
        pub_file
            .read_exact(&mut pub_bytes)
            .expect("Erreur de lecture");
        let public_key_hex = hex::encode(pub_bytes);
        Self {
            ticket: row.get(5),
            hash: row.get(0),
            tree_hash: row.get(2),
            parent_hash: row.get(1),
            message: row.get(4),
            author: row.get(3),
            timestamp: row.get(6),
            signature: row.get(7),
            public_key: public_key_hex,
            project: project_name,
        }
    }
    pub async fn push(&self) -> bool {
        let client = Client::new();
        let url = "http://localhost:3000/api/v1/commits";

        let response = client
            .post(url)
            .json(&json!({ "commit":  self}))
            .send()
            .await
            .expect("server not runing");

        response.status().is_success()
    }
}
