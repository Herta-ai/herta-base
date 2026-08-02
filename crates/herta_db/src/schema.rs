use herta_core::{HbError, HbResult};
use surrealdb::types::RecordId;

use crate::{
    DbClient,
    models::{CollectionDef, FieldDef, IndexDef, SchemaMode, UpdateCollectionRequest},
    validation::{quote_identifier, validate_collection, validate_patch},
};

pub struct SchemaManager<'a> {
    db: &'a DbClient,
}

impl<'a> SchemaManager<'a> {
    pub fn new(db: &'a DbClient) -> Self {
        Self { db }
    }

    pub async fn create_collection(&self, def: &CollectionDef) -> HbResult<CollectionDef> {
        validate_collection(def)?;
        if self.get_collection_optional(&def.name).await?.is_some() {
            return Err(HbError::Conflict(format!(
                "collection '{}' already exists",
                def.name
            )));
        }

        for field in &def.fields {
            if let Some(target) = relation_target(field)
                && self.get_collection_optional(target).await?.is_none()
            {
                return Err(HbError::validation(format!(
                    "relation field '{}' targets missing collection '{target}'",
                    field.name
                )));
            }
        }

        let mut sql = String::from("BEGIN TRANSACTION;\n");
        let table = quote_identifier(&def.name);
        let schema = match def.schema_mode {
            SchemaMode::Strict => "SCHEMAFULL",
            SchemaMode::Schemaless | SchemaMode::Mixed => "SCHEMALESS",
        };
        sql.push_str(&format!("DEFINE TABLE {table} {schema};\n"));
        for field in &def.fields {
            append_field_ddl(&mut sql, &table, field);
        }
        if def.collection_type == crate::models::CollectionType::Auth {
            append_auth_fields(&mut sql, &table);
        }
        append_system_fields(&mut sql, &table);
        for index in &def.indexes {
            append_index_ddl(&mut sql, &table, index);
        }
        sql.push_str("CREATE ONLY type::record('_collections', $name) CONTENT $definition;\n");
        sql.push_str("COMMIT TRANSACTION;");

        let response = self
            .db
            .inner()
            .query(sql)
            .bind(("name", def.name.clone()))
            .bind((
                "definition",
                serde_json::to_value(def).map_err(|error| HbError::Database(error.to_string()))?,
            ))
            .await
            .map_err(database_error)?;
        response.check().map_err(database_error)?;
        Ok(def.clone())
    }

    pub async fn list_collections(&self) -> HbResult<Vec<CollectionDef>> {
        let mut response = self
            .db
            .inner()
            .query("SELECT * OMIT id FROM _collections ORDER BY name ASC")
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let records: Vec<serde_json::Value> = response.take(0).map_err(database_error)?;
        records
            .into_iter()
            .map(|value| serde_json::from_value(value).map_err(database_error))
            .collect()
    }

    pub async fn get_collection(&self, name: &str) -> HbResult<CollectionDef> {
        self.get_collection_optional(name)
            .await?
            .ok_or_else(|| HbError::CollectionNotFound(name.into()))
    }

    async fn get_collection_optional(&self, name: &str) -> HbResult<Option<CollectionDef>> {
        let mut response = self
            .db
            .inner()
            .query("SELECT * OMIT id FROM type::record('_collections', $name)")
            .bind(("name", name.to_owned()))
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let records: Vec<serde_json::Value> = response.take(0).map_err(database_error)?;
        records
            .into_iter()
            .next()
            .map(|value| serde_json::from_value(value).map_err(database_error))
            .transpose()
    }

    pub async fn update_collection(
        &self,
        name: &str,
        patch: &UpdateCollectionRequest,
    ) -> HbResult<CollectionDef> {
        let existing = self.get_collection(name).await?;
        validate_patch(&existing, patch)?;
        for field in &patch.fields {
            if let Some(target) = relation_target(field)
                && self.get_collection_optional(target).await?.is_none()
            {
                return Err(HbError::validation(format!(
                    "relation field '{}' targets missing collection '{target}'",
                    field.name
                )));
            }
        }

        let mut updated = existing.clone();
        updated.fields.extend(patch.fields.clone());
        updated.indexes.extend(patch.indexes.clone());
        if let Some(rules) = &patch.rules {
            updated.rules = rules.clone();
        }

        let table = quote_identifier(name);
        let mut sql = String::from("BEGIN TRANSACTION;\n");
        for field in &patch.fields {
            append_field_ddl(&mut sql, &table, field);
        }
        for index in &patch.indexes {
            append_index_ddl(&mut sql, &table, index);
        }
        sql.push_str(
            "UPDATE ONLY type::record('_collections', $name) CONTENT $definition;\n\
             COMMIT TRANSACTION;",
        );

        let response = self
            .db
            .inner()
            .query(sql)
            .bind(("name", name.to_owned()))
            .bind((
                "definition",
                serde_json::to_value(&updated)
                    .map_err(|error| HbError::Database(error.to_string()))?,
            ))
            .await
            .map_err(database_error)?;
        response.check().map_err(database_error)?;
        Ok(updated)
    }

    pub async fn delete_collection(&self, name: &str) -> HbResult<()> {
        self.get_collection(name).await?;
        let table = quote_identifier(name);
        let sql = format!(
            "BEGIN TRANSACTION;\n\
             REMOVE TABLE {table};\n\
             DELETE ONLY type::record('_collections', $name);\n\
             COMMIT TRANSACTION;"
        );
        let response = self
            .db
            .inner()
            .query(sql)
            .bind(("name", name.to_owned()))
            .await
            .map_err(database_error)?;
        response.check().map_err(database_error)?;
        Ok(())
    }
}

fn append_field_ddl(sql: &mut String, table: &str, field: &FieldDef) {
    let field_name = quote_identifier(&field.name);
    let kind = field.field_type.surreal_kind(field.options.as_ref());
    let kind = if field.required {
        kind
    } else {
        format!("option<{kind}>")
    };
    sql.push_str(&format!(
        "DEFINE FIELD {field_name} ON TABLE {table} TYPE {kind};\n"
    ));
}

fn append_system_fields(sql: &mut String, table: &str) {
    sql.push_str(&format!(
        "DEFINE FIELD created_at ON TABLE {table} TYPE datetime DEFAULT time::now();\n\
         DEFINE FIELD updated_at ON TABLE {table} TYPE datetime DEFAULT time::now();\n\
         DEFINE FIELD deleted_at ON TABLE {table} TYPE option<datetime> DEFAULT NONE;\n"
    ));
}

fn append_auth_fields(sql: &mut String, table: &str) {
    sql.push_str(&format!(
        "DEFINE FIELD email ON TABLE {table} TYPE string;\n\
         DEFINE FIELD password_hash ON TABLE {table} TYPE string;\n\
         DEFINE FIELD token_key ON TABLE {table} TYPE string;\n\
         DEFINE FIELD verified ON TABLE {table} TYPE bool DEFAULT false;\n\
         DEFINE FIELD role ON TABLE {table} TYPE string DEFAULT 'user';\n\
         DEFINE FIELD failed_attempts ON TABLE {table} TYPE number DEFAULT 0;\n\
         DEFINE FIELD locked_until ON TABLE {table} TYPE option<number> DEFAULT NONE;\n\
         DEFINE INDEX idx_auth_email ON TABLE {table} FIELDS email UNIQUE;\n"
    ));
}

fn append_index_ddl(sql: &mut String, table: &str, index: &IndexDef) {
    let index_name = quote_identifier(&index.name);
    let fields = index
        .fields
        .iter()
        .map(|field| quote_identifier(field))
        .collect::<Vec<_>>()
        .join(", ");
    let unique = if index.unique { " UNIQUE" } else { "" };
    sql.push_str(&format!(
        "DEFINE INDEX {index_name} ON TABLE {table} FIELDS {fields}{unique};\n"
    ));
}

fn relation_target(field: &FieldDef) -> Option<&str> {
    (field.field_type == crate::models::FieldType::Relation)
        .then(|| field.options.as_ref()?.get("collection")?.as_str())
        .flatten()
}

pub(crate) fn database_error(error: impl std::fmt::Display) -> HbError {
    let message = error.to_string();
    if message.to_ascii_lowercase().contains("unique")
        || message.to_ascii_lowercase().contains("already exists")
        || message.to_ascii_lowercase().contains("already contains")
    {
        HbError::Conflict(message)
    } else {
        HbError::Database(message)
    }
}

pub(crate) fn record_id(table: &str, id: &str) -> RecordId {
    RecordId::new(table, id)
}
