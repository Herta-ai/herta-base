use std::path::PathBuf;

use herta_core::HbConfig;
use surrealdb::{
    Surreal,
    engine::local::{Db, Mem, SurrealKv},
};

#[derive(Clone)]
pub struct DbClient {
    db: Surreal<Db>,
}

impl DbClient {
    pub async fn init(config: &HbConfig) -> anyhow::Result<Self> {
        let db = if config.database.engine == "memory" {
            Surreal::new::<Mem>(()).await?
        } else {
            let path = PathBuf::from(&config.paths.data_dir).join("database");
            std::fs::create_dir_all(&path)?;
            Surreal::new::<SurrealKv>(path).await?
        };
        db.use_ns("hertabase").use_db("main").await?;
        let client = Self { db };
        client.init_system_tables().await?;
        Ok(client)
    }

    pub async fn memory() -> anyhow::Result<Self> {
        let mut config = HbConfig::default();
        config.database.engine = "memory".into();
        Self::init(&config).await
    }

    async fn init_system_tables(&self) -> anyhow::Result<()> {
        let response = self
            .db
            .query(
                "DEFINE TABLE IF NOT EXISTS _collections SCHEMALESS;\
                 DEFINE INDEX IF NOT EXISTS idx_collections_name \
                   ON TABLE _collections FIELDS name UNIQUE;\
                 DEFINE TABLE IF NOT EXISTS _users SCHEMALESS;\
                 DEFINE FIELD IF NOT EXISTS created_at ON TABLE _users TYPE datetime DEFAULT time::now();\
                 DEFINE FIELD IF NOT EXISTS updated_at ON TABLE _users TYPE datetime DEFAULT time::now();\
                 DEFINE FIELD IF NOT EXISTS deleted_at ON TABLE _users TYPE option<datetime> DEFAULT NONE;\
                 DEFINE INDEX IF NOT EXISTS idx_users_email ON TABLE _users FIELDS email UNIQUE;\
                 DEFINE TABLE IF NOT EXISTS _admins SCHEMALESS;\
                 DEFINE FIELD IF NOT EXISTS created_at ON TABLE _admins TYPE datetime DEFAULT time::now();\
                 DEFINE FIELD IF NOT EXISTS updated_at ON TABLE _admins TYPE datetime DEFAULT time::now();\
                 DEFINE FIELD IF NOT EXISTS deleted_at ON TABLE _admins TYPE option<datetime> DEFAULT NONE;\
                 DEFINE INDEX IF NOT EXISTS idx_admins_email ON TABLE _admins FIELDS email UNIQUE;\
                 DEFINE TABLE IF NOT EXISTS _auth_refresh_tokens SCHEMALESS;\
                 DEFINE INDEX IF NOT EXISTS idx_refresh_jti ON TABLE _auth_refresh_tokens FIELDS jti UNIQUE;\
                 DEFINE INDEX IF NOT EXISTS idx_refresh_family ON TABLE _auth_refresh_tokens FIELDS family;",
            )
            .await?;
        response.check()?;
        let users: Option<serde_json::Value> = self.db.select(("_collections", "_users")).await?;
        if users.is_none() {
            let response = self
                .db
                .query(
                    "CREATE ONLY type::record('_collections', '_users') CONTENT {\
                       name: '_users', type: 'auth', schema_mode: 'schema-less', fields: [], indexes: [],\
                       rules: { list: NONE, view: NONE, create: NONE, update: NONE, delete: NONE }\
                     };",
                )
                .await?;
            response.check()?;
        }
        Ok(())
    }

    pub fn inner(&self) -> &Surreal<Db> {
        &self.db
    }
}
