use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env};

pub type RowData = HashMap<String, String>;
#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
pub enum Driver {
    Sqlite,
    Postgres,
    Mariadb,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Config {
    pub driver: Driver,
    pub database_url: String,
    pub user: Option<String>,
    pub password: Option<String>,
}

pub struct Conn {
    pub driver: Driver,
    pub config: Config,
}
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum SqlValue {
    Text(String),
    Int(i64),
}

// 1. L'enum avec les vrais pools de connexions SQLx
pub enum RawConnection {
    Sqlite(sqlx::SqlitePool),
    Postgres(sqlx::PgPool),
    Mariadb(sqlx::MySqlPool),
}

// 2. La structure concrète qui contient la connexion active
pub struct DatabaseBackend {
    pub driver: Driver,
    pub inner_conn: RawConnection,
}

// 3. Définition des Traits (Asynchrones pour SQLx)
pub trait Connection {
    // On passe en async car l'ouverture d'un pool réseau prend du temps
    fn conn(&self) -> impl std::future::Future<Output = Result<DatabaseBackend, String>> + Send;
}
pub trait Db {
    fn driver(&self) -> Driver;
    fn fetch(
        &self,
        query: &str,
        params: &[&str],
    ) -> impl std::future::Future<Output = Result<Vec<SqlValue>, sqlx::Error>> + Send;
    fn execute(
        &self,
        query: &str,
        params: &[&str],
    ) -> impl std::future::Future<Output = Result<u64, sqlx::Error>> + Send;
}

impl Connection for Conn {
    async fn conn(&self) -> Result<DatabaseBackend, String> {
        match self.driver {
            Driver::Sqlite => {
                let pool = sqlx::SqlitePool::connect(&self.config.database_url)
                    .await
                    .map_err(|e| format!("Erreur SQLite : {}", e))?;

                // EXÉCUTION AUTOMATIQUE DE LA MIGRATION
                #[cfg(feature = "sqlite")]
                sqlx::migrate!("./migrations/sqlite")
                    .run(&pool)
                    .await
                    .map_err(|e| format!("Échec de la migration SQLite : {e}"))?;

                Ok(DatabaseBackend {
                    driver: Driver::Sqlite,
                    inner_conn: RawConnection::Sqlite(pool),
                })
            }
            Driver::Postgres => {
                let pool = sqlx::PgPool::connect(&self.config.database_url)
                    .await
                    .map_err(|e| format!("Erreur Postgres : {e}"))?;

                #[cfg(feature = "postgres")]
                sqlx::migrate!("./migrations/postgres")
                    .run(&pool)
                    .await
                    .map_err(|e| format!("Échec de la migration Postgres : {e}"))?;

                Ok(DatabaseBackend {
                    driver: Driver::Postgres,
                    inner_conn: RawConnection::Postgres(pool),
                })
            }
            Driver::Mariadb => {
                let pool = sqlx::MySqlPool::connect(&self.config.database_url)
                    .await
                    .map_err(|e| format!("Erreur MariaDB : {e}"))?;

                #[cfg(feature = "mariadb")]
                sqlx::migrate!("./migrations/mariadb")
                    .run(&pool)
                    .await
                    .map_err(|e| format!("Échec de la migration MariaDB : {e}"))?;

                Ok(DatabaseBackend {
                    driver: Driver::Mariadb,
                    inner_conn: RawConnection::Mariadb(pool),
                })
            }
        }
    }
}

// Dans ton implémentation :
impl Db for DatabaseBackend {
    fn driver(&self) -> Driver {
        self.driver
    }

    async fn execute(&self, query: &str, params: &[&str]) -> Result<u64, sqlx::Error> {
        match &self.inner_conn {
            RawConnection::Sqlite(pool) => {
                // 1. On initialise le constructeur avec la requête
                let mut builder: sqlx::QueryBuilder<sqlx::Sqlite> = sqlx::QueryBuilder::new(query);

                // 2. On insère les paramètres de façon sécurisée
                for param in params {
                    builder.push_bind(*param);
                }

                // 3. On compile la requête finale et on l'exécute
                Ok(builder.build().execute(pool).await?.rows_affected())
            }
            RawConnection::Postgres(pool) => {
                let mut builder: sqlx::QueryBuilder<sqlx::Postgres> =
                    sqlx::QueryBuilder::new(query);
                for param in params {
                    builder.push_bind(*param);
                }
                Ok(builder.build().execute(pool).await?.rows_affected())
            }
            RawConnection::Mariadb(pool) => {
                let mut builder: sqlx::QueryBuilder<sqlx::MySql> = sqlx::QueryBuilder::new(query);
                for param in params {
                    builder.push_bind(*param);
                }
                Ok(builder.build().execute(pool).await?.rows_affected())
            }
        }
    }

    async fn fetch(&self, query: &str, params: &[&str]) -> Result<Vec<SqlValue>, sqlx::Error> {
        match &self.inner_conn {
            RawConnection::Sqlite(pool) => {
                // 1. On initialise le constructeur avec la requête
                let mut builder: sqlx::QueryBuilder<sqlx::Sqlite> = sqlx::QueryBuilder::new(query);

                // 2. On insère les paramètres de façon sécurisée
                for param in params {
                    builder.push_bind(*param);
                }

                Ok(Vec::new())
            }
            RawConnection::Postgres(pool) => {
                let mut builder: sqlx::QueryBuilder<sqlx::Postgres> =
                    sqlx::QueryBuilder::new(query);
                for param in params {
                    builder.push_bind(*param);
                }
                Ok(Vec::new())
            }
            RawConnection::Mariadb(pool) => {
                let mut builder: sqlx::QueryBuilder<sqlx::MySql> = sqlx::QueryBuilder::new(query);
                for param in params {
                    builder.push_bind(*param);
                }
                Ok(Vec::new())
            }
        }
    }
}
impl Conn {
    #[must_use]
    pub fn new() -> Self {
        dotenv().ok();

        let host = env::var("DB_HOST").expect("DB_HOST must be define");
        let db_name = env::var("DB_NAME").expect("DB_NAME must be define");
        let user = env::var("DB_USER").expect("DB_USER must be define");
        let password = env::var("DB_PASSWORD").expect("DB_PASSWORD must be define");
        let driver_type = env::var("DB_DRIVER").expect("DB_DRIVER must be define (ex: postgres)");

        let database_url = format!("{driver_type}://{user}:{password}@{host}/{db_name}");

        let driver = match driver_type.as_str() {
            "sqlite" => Driver::Sqlite,
            "postgres" | "postgresql" => Driver::Postgres,
            "mysql" => Driver::Mariadb,
            _ => panic!("Driver not supported"),
        };

        Self {
            driver,
            config: Config {
                driver,
                database_url: database_url.clone(),
                user: Some(user),
                password: Some(password),
            },
        }
    }
}
