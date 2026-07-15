use crate::vcs::db::AWQ_DB_PATH;
use crate::vcs::db::conn;
use crate::vcs::hooks::awq_auto_setup_hooks;
use crate::vcs::keys::awq_generate_keypair;
use crate::vcs::ok;
use crate::vcs::tt;
use std::path::Path;
use tokio::fs::{File, create_dir_all};
use unic_langid::LanguageIdentifier;

pub async fn init_awq(lang: &LanguageIdentifier) -> bool {
    create_dir_all(".awq").await.expect("failed to create dir");

    if Path::new(AWQ_DB_PATH).exists().eq(&false) {
        File::create_new(AWQ_DB_PATH)
            .await
            .expect("failed to create file");
    }

    sqlx::migrate!("./migrations")
        .run(&conn().await)
        .await
        .expect("failed to create tables");

    ok(tt(lang, "db-init-success").as_str());
    awq_generate_keypair(lang);
    awq_auto_setup_hooks().await.is_ok()
}
