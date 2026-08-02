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
                   ON TABLE _collections FIELDS name UNIQUE;",
            )
            .await?;
        response.check()?;
        Ok(())
    }

    pub fn inner(&self) -> &Surreal<Db> {
        &self.db
    }
}
