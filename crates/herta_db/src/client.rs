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
                 DEFINE INDEX IF NOT EXISTS idx_refresh_family ON TABLE _auth_refresh_tokens FIELDS family;\
                 DEFINE TABLE IF NOT EXISTS _logs SCHEMALESS;\
                 DEFINE FIELD IF NOT EXISTS log_type ON TABLE _logs TYPE string;\
                 DEFINE FIELD IF NOT EXISTS level ON TABLE _logs TYPE string;\
                 DEFINE FIELD IF NOT EXISTS message ON TABLE _logs TYPE string;\
                 DEFINE FIELD IF NOT EXISTS target ON TABLE _logs TYPE string;\
                 DEFINE FIELD IF NOT EXISTS method ON TABLE _logs TYPE option<string>;\
                 DEFINE FIELD IF NOT EXISTS path ON TABLE _logs TYPE option<string>;\
                 DEFINE FIELD IF NOT EXISTS status_code ON TABLE _logs TYPE option<number>;\
                 DEFINE FIELD IF NOT EXISTS referer ON TABLE _logs TYPE option<string>;\
                 DEFINE FIELD IF NOT EXISTS remote_ip ON TABLE _logs TYPE option<string>;\
                 DEFINE FIELD IF NOT EXISTS user_agent ON TABLE _logs TYPE option<string>;\
                 DEFINE FIELD IF NOT EXISTS auth_type ON TABLE _logs TYPE option<string>;\
                 DEFINE FIELD IF NOT EXISTS user_id ON TABLE _logs TYPE option<string>;\
                 DEFINE FIELD IF NOT EXISTS user_collection ON TABLE _logs TYPE option<string>;\
                 DEFINE FIELD IF NOT EXISTS created_at ON TABLE _logs TYPE datetime DEFAULT time::now();\
                 DEFINE INDEX IF NOT EXISTS idx_logs_type ON TABLE _logs FIELDS log_type;\
                 DEFINE INDEX IF NOT EXISTS idx_logs_level ON TABLE _logs FIELDS level;\
                 DEFINE INDEX IF NOT EXISTS idx_logs_created ON TABLE _logs FIELDS created_at;\
                 DEFINE INDEX IF NOT EXISTS idx_logs_status ON TABLE _logs FIELDS status_code;\
                 DEFINE INDEX IF NOT EXISTS idx_logs_user ON TABLE _logs FIELDS user_id;",
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
