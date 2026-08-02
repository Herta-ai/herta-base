use std::collections::{HashMap, HashSet};

use herta_core::{HbError, HbResult};
use serde_json::{Map, Value};
use surrealdb::types::RecordId;
use uuid::Uuid;

use crate::{
    DbClient, SchemaManager,
    filter::compile_filter,
    models::{CollectionDef, FieldDef, FieldType, ListParams, relation_is_many},
    schema::{database_error, record_id},
    validation::{quote_identifier, validate_identifier, validate_record},
};

const SYSTEM_FIELDS: [&str; 4] = ["id", "created_at", "updated_at", "deleted_at"];
const MAX_EXPAND_DEPTH: usize = 3;
const MAX_EXPAND_PATHS: usize = 10;

pub struct RecordManager<'a> {
    db: &'a DbClient,
}

impl<'a> RecordManager<'a> {
    pub fn new(db: &'a DbClient) -> Self {
        Self { db }
    }

    pub async fn list(&self, collection: &str, params: &ListParams) -> HbResult<(Vec<Value>, u64)> {
        params.validate()?;
        let schema = SchemaManager::new(self.db)
            .get_collection(collection)
            .await?;
        let allowed_fields = allowed_fields(&schema);
        let mut where_sql = String::from("deleted_at IS NONE");
        let compiled_filter = params
            .filter
            .as_deref()
            .map(|filter| compile_filter(filter, &allowed_fields))
            .transpose()?;
        if let Some(filter) = &compiled_filter {
            where_sql.push_str(" AND (");
            where_sql.push_str(&filter.sql);
            where_sql.push(')');
        }
        let order = compile_sort(params.sort.as_deref(), &allowed_fields)?;
        let table = quote_identifier(collection);
        let sql = format!(
            "SELECT count() AS total FROM {table} WHERE {where_sql} GROUP ALL;\n\
             SELECT * FROM {table} WHERE {where_sql} {order} LIMIT $limit START $offset;"
        );
        let mut query = self
            .db
            .inner()
            .query(sql)
            .bind(("limit", params.per_page()))
            .bind(("offset", (params.page() - 1) * params.per_page()));
        if let Some(filter) = compiled_filter {
            for (name, value) in filter.bindings {
                query = query.bind((name, value));
            }
        }
        let mut response = query
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let counts: Vec<Value> = response.take(0).map_err(database_error)?;
        let total = counts
            .first()
            .and_then(|value| value.get("total"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let mut records: Vec<Value> = response.take(1).map_err(database_error)?;
        normalize_records(&mut records);

        if let Some(expand) = params.expand.as_deref() {
            let paths = validate_expand_paths(self.db, &schema, expand).await?;
            self.expand_records(&mut records, &paths).await?;
        }
        Ok((records, total))
    }

    pub async fn get(&self, collection: &str, id: &str, expand: Option<&str>) -> HbResult<Value> {
        let schema = SchemaManager::new(self.db)
            .get_collection(collection)
            .await?;
        let rid = record_id(collection, id);
        let record: Option<Value> = self.db.inner().select(rid).await.map_err(database_error)?;
        let mut record = record.ok_or(HbError::NotFound)?;
        normalize_value(&mut record);
        if !record.get("deleted_at").is_none_or(Value::is_null) {
            return Err(HbError::NotFound);
        }
        if let Some(expand) = expand {
            let paths = validate_expand_paths(self.db, &schema, expand).await?;
            self.expand_records(std::slice::from_mut(&mut record), &paths)
                .await?;
        }
        Ok(record)
    }

    pub async fn create(&self, collection: &str, mut data: Value) -> HbResult<Value> {
        let schema = SchemaManager::new(self.db)
            .get_collection(collection)
            .await?;
        validate_record(&schema, &mut data, true)?;
        let id = Uuid::now_v7().to_string();
        self.write_record(&schema, &id, data, true).await
    }

    pub async fn update(&self, collection: &str, id: &str, mut data: Value) -> HbResult<Value> {
        let schema = SchemaManager::new(self.db)
            .get_collection(collection)
            .await?;
        validate_record(&schema, &mut data, false)?;
        self.get(collection, id, None).await?;
        self.write_record(&schema, id, data, false).await
    }

    async fn write_record(
        &self,
        schema: &CollectionDef,
        id: &str,
        data: Value,
        create: bool,
    ) -> HbResult<Value> {
        let object = data
            .as_object()
            .ok_or_else(|| HbError::validation("record body must be a JSON object"))?;
        if !create && object.is_empty() {
            return Err(HbError::validation("update body cannot be empty"));
        }

        let mut assignments = Vec::new();
        enum Binding {
            Json(String, Value),
            Record(String, RecordId),
            Records(String, Vec<RecordId>),
        }
        let mut bindings = Vec::new();
        for (index, (field_name, value)) in object.iter().enumerate() {
            validate_identifier("field name", field_name)?;
            let binding_name = format!("value_{index}");
            assignments.push(format!(
                "{} = ${binding_name}",
                quote_identifier(field_name)
            ));
            let field = schema.fields.iter().find(|field| field.name == *field_name);
            if let Some(field) = field.filter(|field| field.field_type == FieldType::Relation) {
                if relation_is_many(field.options.as_ref()) {
                    let values = value
                        .as_array()
                        .expect("relation arrays were validated")
                        .iter()
                        .map(|value| parse_record_id(value.as_str().expect("validated relation")))
                        .collect::<HbResult<Vec<_>>>()?;
                    bindings.push(Binding::Records(binding_name, values));
                } else {
                    let value = parse_record_id(value.as_str().expect("validated relation"))?;
                    bindings.push(Binding::Record(binding_name, value));
                }
            } else {
                bindings.push(Binding::Json(binding_name, value.clone()));
            }
        }
        if !create {
            assignments.push("updated_at = time::now()".into());
        }
        let operation = if create { "CREATE ONLY" } else { "UPDATE ONLY" };
        let sql = format!(
            "{operation} $record SET {} RETURN AFTER",
            assignments.join(", ")
        );
        let mut query = self
            .db
            .inner()
            .query(sql)
            .bind(("record", record_id(&schema.name, id)));
        for binding in bindings {
            query = match binding {
                Binding::Json(name, value) => query.bind((name, value)),
                Binding::Record(name, value) => query.bind((name, value)),
                Binding::Records(name, value) => query.bind((name, value)),
            };
        }
        let mut response = query
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let mut records: Vec<Value> = response.take(0).map_err(database_error)?;
        normalize_records(&mut records);
        records.pop().ok_or(HbError::Internal)
    }

    pub async fn delete(&self, collection: &str, id: &str) -> HbResult<Value> {
        SchemaManager::new(self.db)
            .get_collection(collection)
            .await?;
        let mut response = self
            .db
            .inner()
            .query(
                "UPDATE ONLY $record SET deleted_at = time::now(), updated_at = time::now() \
                 WHERE deleted_at IS NONE RETURN AFTER",
            )
            .bind(("record", record_id(collection, id)))
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let mut records: Vec<Value> = response.take(0).map_err(database_error)?;
        normalize_records(&mut records);
        records.pop().ok_or(HbError::NotFound)
    }

    async fn expand_records(&self, records: &mut [Value], paths: &[ExpandPath]) -> HbResult<()> {
        if records.is_empty() || paths.is_empty() {
            return Ok(());
        }
        let ids = records
            .iter()
            .filter_map(|record| record.get("id").and_then(Value::as_str))
            .map(parse_record_id)
            .collect::<HbResult<Vec<_>>>()?;
        let fetch = paths
            .iter()
            .map(|path| {
                path.segments
                    .iter()
                    .map(|segment| quote_identifier(segment))
                    .collect::<Vec<_>>()
                    .join(".")
            })
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!("SELECT * FROM $records FETCH {fetch}");
        let mut response = self
            .db
            .inner()
            .query(sql)
            .bind(("records", ids))
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let mut expanded: Vec<Value> = response.take(0).map_err(database_error)?;
        normalize_records(&mut expanded);
        let expanded_by_id: HashMap<String, Value> = expanded
            .into_iter()
            .filter_map(|record| {
                let id = record.get("id")?.as_str()?.to_owned();
                Some((id, record))
            })
            .collect();

        let root_fields: HashSet<&str> = paths
            .iter()
            .filter_map(|path| path.segments.first().map(String::as_str))
            .collect();
        for record in records {
            let Some(id) = record.get("id").and_then(Value::as_str) else {
                continue;
            };
            let Some(expanded) = expanded_by_id.get(id) else {
                continue;
            };
            let mut expand = Map::new();
            for field in &root_fields {
                if let Some(value) = expanded.get(*field) {
                    expand.insert((*field).to_owned(), value.clone());
                }
            }
            if let Some(object) = record.as_object_mut() {
                object.insert("expand".into(), Value::Object(expand));
            }
        }
        Ok(())
    }
}

fn allowed_fields(schema: &CollectionDef) -> HashSet<String> {
    schema
        .fields
        .iter()
        .map(|field| field.name.clone())
        .chain(SYSTEM_FIELDS.map(str::to_owned))
        .collect()
}

fn compile_sort(sort: Option<&str>, allowed: &HashSet<String>) -> HbResult<String> {
    let sort = sort.unwrap_or("-created_at");
    let mut fields = Vec::new();
    for item in sort.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let (field, direction) = item
            .strip_prefix('-')
            .map_or((item, "ASC"), |field| (field, "DESC"));
        if !allowed.contains(field) {
            return Err(HbError::InvalidSort(format!("unknown field '{field}'")));
        }
        fields.push(format!("{} {direction}", quote_identifier(field)));
    }
    if fields.is_empty() {
        return Err(HbError::InvalidSort(
            "at least one sort field is required".into(),
        ));
    }
    Ok(format!("ORDER BY {}", fields.join(", ")))
}

fn parse_record_id(value: &str) -> HbResult<RecordId> {
    RecordId::parse_simple(value).map_err(|_| HbError::validation("invalid relation record id"))
}

#[derive(Debug, Clone)]
struct ExpandPath {
    segments: Vec<String>,
}

async fn validate_expand_paths(
    db: &DbClient,
    root: &CollectionDef,
    input: &str,
) -> HbResult<Vec<ExpandPath>> {
    let raw_paths: Vec<&str> = input
        .split(',')
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .collect();
    if raw_paths.is_empty() || raw_paths.len() > MAX_EXPAND_PATHS {
        return Err(HbError::validation(format!(
            "expand must contain between 1 and {MAX_EXPAND_PATHS} paths"
        )));
    }
    let manager = SchemaManager::new(db);
    let mut paths = Vec::new();
    for raw in raw_paths {
        let segments: Vec<String> = raw.split('.').map(str::to_owned).collect();
        if segments.len() > MAX_EXPAND_DEPTH || segments.iter().any(|segment| segment.is_empty()) {
            return Err(HbError::validation(format!(
                "expand path '{raw}' exceeds {MAX_EXPAND_DEPTH} levels or is malformed"
            )));
        }
        let mut current = root.clone();
        for segment in &segments {
            let field = current
                .fields
                .iter()
                .find(|field| field.name == *segment && field.field_type == FieldType::Relation)
                .ok_or_else(|| {
                    HbError::validation(format!("expand path '{raw}' is not a relation path"))
                })?;
            let target = relation_target(field).ok_or_else(|| {
                HbError::validation(format!("relation field '{segment}' has no target"))
            })?;
            current = manager.get_collection(target).await?;
        }
        paths.push(ExpandPath { segments });
    }
    Ok(paths)
}

fn relation_target(field: &FieldDef) -> Option<&str> {
    field.options.as_ref()?.get("collection")?.as_str()
}

fn normalize_records(records: &mut [Value]) {
    for record in records {
        normalize_value(record);
    }
}

fn normalize_value(value: &mut Value) {
    match value {
        Value::String(text) => {
            if let Some((table, key)) = text.split_once(':')
                && key.starts_with('`')
                && key.ends_with('`')
                && key.len() >= 2
            {
                *text = format!("{table}:{}", &key[1..key.len() - 1]);
            }
        }
        Value::Array(values) => values.iter_mut().for_each(normalize_value),
        Value::Object(object) => {
            for value in object.values_mut() {
                normalize_value(value);
            }
            if object.len() == 2 {
                let table = object.get("table").and_then(Value::as_str);
                let key = object.get("key").and_then(record_key_string);
                if let (Some(table), Some(key)) = (table, key) {
                    *value = Value::String(format!("{table}:{key}"));
                }
            }
        }
        _ => {}
    }
}

fn record_key_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_i64().map(|value| value.to_string()))
        .or_else(|| value.as_u64().map(|value| value.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_rejects_unknown_fields() {
        let allowed = ["title".to_owned()].into_iter().collect();
        assert!(compile_sort(Some("title;REMOVE TABLE posts"), &allowed).is_err());
    }

    #[test]
    fn normalizes_record_id_objects() {
        let mut value = serde_json::json!({"id": {"table": "posts", "key": "one"}});
        normalize_value(&mut value);
        assert_eq!(value["id"], "posts:one");
    }
}
