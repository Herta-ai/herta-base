use std::{path::PathBuf, sync::Arc};

use herta_core::HbConfig;
use surrealdb::{
    Surreal,
    engine::local::{Db, Mem, SurrealKv},
    opt::{Config, capabilities::Capabilities},
};

#[derive(Clone)]
pub struct DbClient {
    db: Arc<Surreal<Db>>,
    file_operations: Arc<tokio::sync::Mutex<()>>,
    outbox_claims: Arc<tokio::sync::Mutex<()>>,
}

impl DbClient {
    pub async fn init(config: &HbConfig) -> anyhow::Result<Self> {
        let options = Config::new().capabilities(
            Capabilities::new()
                .with_scripting(false)
                .with_live_query_notifications(true)
                .with_no_net_targets_allowed()
                .with_all_net_targets_denied(),
        );
        let db = if config.database.engine == "memory" {
            Surreal::new::<Mem>(options).await?
        } else {
            let path = PathBuf::from(&config.paths.data_dir).join("database");
            std::fs::create_dir_all(&path)?;
            Surreal::new::<SurrealKv>((path, options)).await?
        };
        db.use_ns("hertabase").use_db("main").await?;
        let client = Self {
            db: Arc::new(db),
            file_operations: Arc::new(tokio::sync::Mutex::new(())),
            outbox_claims: Arc::new(tokio::sync::Mutex::new(())),
        };
        client.init_system_tables().await?;
        crate::transaction::recover_operations(&client).await?;
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
                 DEFINE TABLE IF NOT EXISTS _extension_files SCHEMALESS;\
                 DEFINE TABLE IF NOT EXISTS _extension_outbox SCHEMALESS;\
                 DEFINE TABLE IF NOT EXISTS _extension_outbox_meta SCHEMALESS;\
                 DEFINE INDEX IF NOT EXISTS idx_extension_outbox_due ON _extension_outbox FIELDS state, nextAttemptAt;\
                 DEFINE INDEX IF NOT EXISTS idx_extension_files_key ON _extension_files FIELDS prefix, key UNIQUE;\
                 DEFINE INDEX IF NOT EXISTS idx_extension_files_object ON _extension_files FIELDS object;\
                 DEFINE TABLE IF NOT EXISTS _extension_file_versions SCHEMALESS;\
                 DEFINE INDEX IF NOT EXISTS idx_extension_file_versions_object ON _extension_file_versions FIELDS object;\
                 DEFINE TABLE IF NOT EXISTS _extension_file_resolutions SCHEMALESS;\
                 DEFINE TABLE IF NOT EXISTS _collection_effects SCHEMALESS;\
                 DEFINE INDEX IF NOT EXISTS idx_collection_effects_name ON TABLE _collection_effects FIELDS name;\
                 DEFINE TABLE IF NOT EXISTS _operations SCHEMALESS;\
                 DEFINE INDEX IF NOT EXISTS idx_operations_state ON TABLE _operations FIELDS state;\
                 DEFINE TABLE IF NOT EXISTS _operation_commits SCHEMALESS;\
                 DEFINE TABLE IF NOT EXISTS _record_uploads SCHEMALESS;\
                 DEFINE INDEX IF NOT EXISTS idx_record_uploads_operation ON TABLE _record_uploads FIELDS operationId;\
                 DEFINE TABLE IF NOT EXISTS _record_attachment_states SCHEMALESS;\
                 DEFINE INDEX IF NOT EXISTS idx_collections_name \
                   ON TABLE _collections FIELDS name UNIQUE;\
                 DEFINE TABLE IF NOT EXISTS _users SCHEMALESS;\
                 DEFINE FIELD IF NOT EXISTS created_at ON TABLE _users TYPE datetime DEFAULT time::now();\
                 DEFINE FIELD IF NOT EXISTS updated_at ON TABLE _users TYPE datetime DEFAULT time::now();\
                 DEFINE FIELD IF NOT EXISTS deleted_at ON TABLE _users TYPE option<datetime> DEFAULT NONE;\
                 DEFINE INDEX IF NOT EXISTS idx_users_email ON TABLE _users FIELDS email UNIQUE;\
                 DEFINE TABLE IF NOT EXISTS _admins SCHEMALESS;\
                 DEFINE FIELD IF NOT EXISTS _hb_auth_issuance ON TABLE _admins TYPE option<string>;\
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
                 DEFINE INDEX IF NOT EXISTS idx_logs_user ON TABLE _logs FIELDS user_id;\
                 DEFINE TABLE IF NOT EXISTS _web_projects SCHEMALESS;\
                 DEFINE INDEX IF NOT EXISTS idx_web_projects_name \
                   ON TABLE _web_projects FIELDS name UNIQUE;",
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
        // Backfill the revision of definitions created before runtime support.
        self.db
            .query("UPDATE _collections SET version = rand::uuid::v7() WHERE version IS NONE")
            .await?
            .check()?;
        // Older SCHEMAFULL Auth tables must retain the real-write issuance fence.
        for definition in crate::SchemaManager::new(self).list_collections().await? {
            if definition.collection_type == crate::CollectionType::Auth {
                let table = crate::validation::quote_identifier(&definition.name);
                self.db.query(format!("DEFINE FIELD IF NOT EXISTS _hb_auth_issuance ON TABLE {table} TYPE option<string>"))
                    .await?.check()?;
            }
        }
        Ok(())
    }

    pub fn inner(&self) -> &Surreal<Db> {
        self.db.as_ref()
    }

    /// Shared by all hosts and snapshots using this embedded database.
    pub async fn file_operations_lock(&self) -> tokio::sync::OwnedMutexGuard<()> {
        self.file_operations.clone().lock_owned().await
    }
    pub(crate) async fn outbox_claim_lock(&self) -> tokio::sync::OwnedMutexGuard<()> {
        self.outbox_claims.clone().lock_owned().await
    }
}
