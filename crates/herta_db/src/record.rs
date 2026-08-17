use std::{
    collections::{HashMap, HashSet},
    future::Future,
    pin::Pin,
};

use herta_core::{HbError, HbResult};
use serde_json::{Map, Value};
use surrealdb::types::{Object, RecordId, Value as SurrealValue};
use uuid::Uuid;

use crate::{
    DbClient, SchemaManager,
    filter::compile_filter,
    models::{
        ApiRule, CollectionDef, CollectionType, FieldDef, FieldType, ListParams, RuleContext,
        relation_is_many,
    },
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
        self.list_authorized(collection, params, &admin_context())
            .await
    }

    pub async fn list_authorized(
        &self,
        collection: &str,
        params: &ListParams,
        context: &RuleContext,
    ) -> HbResult<(Vec<Value>, u64)> {
        params.validate()?;
        let schema = SchemaManager::new(self.db)
            .get_collection(collection)
            .await?;
        let allowed_fields = allowed_fields(&schema);
        let mut where_sql = String::from("deleted_at IS NONE");
        let rule = compile_rule(&schema.rules.list, context, true)?;
        where_sql.push_str(" AND (");
        where_sql.push_str(&rule);
        where_sql.push(')');
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
            .bind(("offset", (params.page() - 1) * params.per_page()))
            .bind(("hb_auth", context.auth.clone()))
            .bind(("hb_auth_record", context.auth_record.clone()))
            .bind(("hb_request", json_request(&context.request_body)));
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
        sanitize_records(&schema, &mut records);

        if let Some(expand) = params.expand.as_deref() {
            let paths = validate_expand_paths(self.db, &schema, expand).await?;
            self.expand_records(&mut records, &paths, context).await?;
        }
        Ok((records, total))
    }

    pub async fn get(&self, collection: &str, id: &str, expand: Option<&str>) -> HbResult<Value> {
        self.get_authorized(collection, id, expand, &admin_context())
            .await
    }

    pub async fn get_authorized(
        &self,
        collection: &str,
        id: &str,
        expand: Option<&str>,
        context: &RuleContext,
    ) -> HbResult<Value> {
        let schema = SchemaManager::new(self.db)
            .get_collection(collection)
            .await?;
        let rule = compile_rule(&schema.rules.view, context, true)?;
        let mut response = self
            .db
            .inner()
            .query(format!(
                "SELECT * FROM ONLY $record WHERE deleted_at IS NONE AND ({rule})"
            ))
            .bind(("record", route_record_id(collection, id)?))
            .bind(("hb_auth", context.auth.clone()))
            .bind(("hb_auth_record", context.auth_record.clone()))
            .bind(("hb_request", json_request(&context.request_body)))
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let records: Vec<Value> = response.take(0).map_err(database_error)?;
        let mut record = records
            .into_iter()
            .find(|record| !record.is_null())
            .ok_or(HbError::NotFound)?;
        normalize_value(&mut record);
        if !record.get("deleted_at").is_none_or(Value::is_null) {
            return Err(HbError::NotFound);
        }
        sanitize_record(&schema, &mut record);
        if let Some(expand) = expand {
            let paths = validate_expand_paths(self.db, &schema, expand).await?;
            self.expand_records(std::slice::from_mut(&mut record), &paths, context)
                .await?;
        }
        Ok(record)
    }

    pub async fn create(&self, collection: &str, data: Value) -> HbResult<Value> {
        self.create_authorized(collection, data, &admin_context())
            .await
    }

    pub async fn create_authorized(
        &self,
        collection: &str,
        data: Value,
        context: &RuleContext,
    ) -> HbResult<Value> {
        self.create_authorized_with_id(collection, &Uuid::now_v7().to_string(), data, context)
            .await
    }

    pub async fn create_authorized_with_id(
        &self,
        collection: &str,
        id: &str,
        mut data: Value,
        context: &RuleContext,
    ) -> HbResult<Value> {
        let schema = SchemaManager::new(self.db)
            .get_collection(collection)
            .await?;
        validate_record(&schema, &mut data, true)?;
        check_create_rule(self.db, &schema, context, &data).await?;
        let mut value = self
            .write_record(&schema, id, data, true, None, context)
            .await?;
        sanitize_record(&schema, &mut value);
        Ok(value)
    }

    pub async fn preflight_create_authorized(
        &self,
        collection: &str,
        mut data: Value,
        context: &RuleContext,
    ) -> HbResult<CollectionDef> {
        let schema = SchemaManager::new(self.db)
            .get_collection(collection)
            .await?;
        validate_record(&schema, &mut data, true)?;
        check_create_rule(self.db, &schema, context, &data).await?;
        Ok(schema)
    }

    pub async fn update(&self, collection: &str, id: &str, data: Value) -> HbResult<Value> {
        self.update_authorized(collection, id, data, &admin_context())
            .await
    }

    pub async fn update_authorized(
        &self,
        collection: &str,
        id: &str,
        mut data: Value,
        context: &RuleContext,
    ) -> HbResult<Value> {
        let schema = SchemaManager::new(self.db)
            .get_collection(collection)
            .await?;
        validate_record(&schema, &mut data, false)?;
        let rule = compile_rule(&schema.rules.update, context, true)?;
        let mut value = self
            .write_record(&schema, id, data, false, Some(rule), context)
            .await?;
        sanitize_record(&schema, &mut value);
        Ok(value)
    }

    pub async fn preflight_update_authorized(
        &self,
        collection: &str,
        id: &str,
        mut data: Value,
        context: &RuleContext,
    ) -> HbResult<CollectionDef> {
        let schema = SchemaManager::new(self.db)
            .get_collection(collection)
            .await?;
        validate_record(&schema, &mut data, false)?;
        if data.as_object().is_none_or(Map::is_empty) {
            return Err(HbError::validation("update body cannot be empty"));
        }
        let rule = compile_rule(&schema.rules.update, context, true)?;
        let mut response = self
            .db
            .inner()
            .query(format!(
                "SELECT id FROM ONLY $record WHERE deleted_at IS NONE AND ({rule})"
            ))
            .bind(("record", route_record_id(collection, id)?))
            .bind(("hb_auth", context.auth.clone()))
            .bind(("hb_auth_record", context.auth_record.clone()))
            .bind(("hb_request", json_request(&context.request_body)))
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let records: Vec<Value> = response.take(0).map_err(database_error)?;
        if records.iter().any(|record| !record.is_null()) {
            Ok(schema)
        } else {
            Err(HbError::Forbidden)
        }
    }

    async fn write_record(
        &self,
        schema: &CollectionDef,
        id: &str,
        data: Value,
        create: bool,
        rule: Option<String>,
        context: &RuleContext,
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
            if value.is_null() {
                assignments.push(format!("{} = NONE", quote_identifier(field_name)));
                continue;
            }
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
        let where_clause = rule.map_or(String::new(), |rule| {
            format!(" WHERE deleted_at IS NONE AND ({rule})")
        });
        let sql = format!(
            "{operation} $record SET {}{where_clause} RETURN AFTER",
            assignments.join(", ")
        );
        let mut query = self
            .db
            .inner()
            .query(sql)
            .bind(("record", route_record_id(&schema.name, id)?))
            .bind(("hb_auth", context.auth.clone()))
            .bind(("hb_auth_record", context.auth_record.clone()))
            .bind(("hb_request", json_request(&context.request_body)));
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
        records
            .into_iter()
            .find(|record| !record.is_null())
            .ok_or(if create {
                HbError::Internal
            } else {
                HbError::Forbidden
            })
    }

    pub async fn delete(&self, collection: &str, id: &str) -> HbResult<Value> {
        self.delete_authorized(collection, id, &admin_context())
            .await
    }

    pub async fn delete_authorized(
        &self,
        collection: &str,
        id: &str,
        context: &RuleContext,
    ) -> HbResult<Value> {
        let schema = SchemaManager::new(self.db)
            .get_collection(collection)
            .await?;
        let rule = compile_rule(&schema.rules.delete, context, true)?;
        let mut response = self
            .db
            .inner()
            .query(format!(
                "UPDATE ONLY $record SET deleted_at = time::now(), updated_at = time::now() \
                 WHERE deleted_at IS NONE AND ({rule}) RETURN AFTER"
            ))
            .bind(("record", route_record_id(collection, id)?))
            .bind(("hb_auth", context.auth.clone()))
            .bind(("hb_auth_record", context.auth_record.clone()))
            .bind(("hb_request", json_request(&context.request_body)))
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let mut records: Vec<Value> = response.take(0).map_err(database_error)?;
        normalize_records(&mut records);
        sanitize_records(&schema, &mut records);
        records
            .into_iter()
            .find(|record| !record.is_null())
            .ok_or(HbError::Forbidden)
    }

    async fn expand_records(
        &self,
        records: &mut [Value],
        paths: &[ExpandPath],
        context: &RuleContext,
    ) -> HbResult<()> {
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
                    let field_paths = paths
                        .iter()
                        .filter(|path| {
                            path.segments
                                .first()
                                .is_some_and(|segment| segment == *field)
                        })
                        .collect::<Vec<_>>();
                    if let Some(target) = field_paths.first().and_then(|path| path.targets.first())
                        && let Some(mut value) =
                            self.authorize_expanded(target, value, context).await?
                    {
                        for path in field_paths
                            .into_iter()
                            .filter(|path| path.segments.len() > 1)
                        {
                            self.prune_nested_expansion(
                                &mut value,
                                &path.segments[1..],
                                &path.targets[1..],
                                context,
                            )
                            .await?;
                        }
                        expand.insert((*field).to_owned(), value);
                    }
                }
            }
            if let Some(object) = record.as_object_mut() {
                object.insert("expand".into(), Value::Object(expand));
            }
        }
        Ok(())
    }

    async fn authorize_expanded(
        &self,
        target: &str,
        value: &Value,
        context: &RuleContext,
    ) -> HbResult<Option<Value>> {
        match value {
            Value::Array(values) => {
                let mut allowed = Vec::new();
                for value in values {
                    if let Some(value) = self
                        .authorize_expanded_record(target, value, context)
                        .await?
                    {
                        allowed.push(value);
                    }
                }
                Ok(Some(Value::Array(allowed)))
            }
            Value::Object(_) => self.authorize_expanded_record(target, value, context).await,
            Value::Null => Ok(None),
            _ => Ok(None),
        }
    }

    async fn authorize_expanded_record(
        &self,
        target: &str,
        value: &Value,
        context: &RuleContext,
    ) -> HbResult<Option<Value>> {
        let Some(id) = value.get("id").and_then(Value::as_str) else {
            return Ok(None);
        };
        let schema = SchemaManager::new(self.db).get_collection(target).await?;
        let id = parse_record_id(id)?;
        if id.table.as_str() != target {
            return Ok(None);
        }
        let rule = match compile_rule(&schema.rules.view, context, true) {
            Ok(rule) => rule,
            Err(HbError::Forbidden) => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut response = self
            .db
            .inner()
            .query(format!(
                "SELECT id FROM ONLY $record WHERE deleted_at IS NONE AND ({rule})"
            ))
            .bind(("record", id))
            .bind(("hb_auth", context.auth.clone()))
            .bind(("hb_auth_record", context.auth_record.clone()))
            .bind(("hb_request", json_request(&context.request_body)))
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let allowed: Vec<Value> = response.take(0).map_err(database_error)?;
        if allowed.iter().all(Value::is_null) {
            return Ok(None);
        }
        let mut value = value.clone();
        sanitize_record(&schema, &mut value);
        sanitize_sensitive_expansion(&mut value);
        Ok(Some(value))
    }

    fn prune_nested_expansion<'b>(
        &'b self,
        value: &'b mut Value,
        segments: &'b [String],
        targets: &'b [String],
        context: &'b RuleContext,
    ) -> Pin<Box<dyn Future<Output = HbResult<()>> + Send + 'b>> {
        Box::pin(async move {
            if segments.is_empty() || targets.is_empty() {
                return Ok(());
            }
            if let Value::Array(values) = value {
                for value in values {
                    self.prune_nested_expansion(value, segments, targets, context)
                        .await?;
                }
                return Ok(());
            }
            let Value::Object(object) = value else {
                return Ok(());
            };
            let field = &segments[0];
            let Some(child) = object.get(field).cloned() else {
                return Ok(());
            };
            let Some(mut child) = self
                .authorize_expanded(&targets[0], &child, context)
                .await?
            else {
                object.remove(field);
                return Ok(());
            };
            if segments.len() > 1 {
                self.prune_nested_expansion(&mut child, &segments[1..], &targets[1..], context)
                    .await?;
            }
            object.insert(field.clone(), child);
            Ok(())
        })
    }
}

fn admin_context() -> RuleContext {
    RuleContext {
        admin: true,
        auth: serde_json::json!({"admin": true, "role": "admin"}),
        auth_record: None,
        request_body: Value::Null,
    }
}

fn json_request(body: &Value) -> Value {
    serde_json::json!({"body": body})
}

pub(crate) fn compile_rule(
    rule: &ApiRule,
    context: &RuleContext,
    current_record: bool,
) -> HbResult<String> {
    if context.admin {
        return Ok("true".into());
    }
    match rule {
        ApiRule::AdminOnly | ApiRule::Boolean(false) => Err(HbError::Forbidden),
        ApiRule::Boolean(true) => Ok("true".into()),
        ApiRule::Expression(expression) if expression.trim().is_empty() => Err(HbError::Forbidden),
        ApiRule::Expression(expression) => {
            let mut compiled = expression
                .replace("$auth.record", "$hb_auth_record")
                .replace("$auth", "$hb_auth")
                .replace("$request", "$hb_request");
            if current_record {
                compiled = compiled.replace("$record.", "");
                if compiled.contains("$record") {
                    return Err(HbError::validation(
                        "$record must be followed by a field path in row rules",
                    ));
                }
            } else {
                compiled = compiled.replace("$record", "$hb_record");
            }
            Ok(compiled)
        }
    }
}

async fn check_create_rule(
    db: &DbClient,
    schema: &CollectionDef,
    context: &RuleContext,
    record: &Value,
) -> HbResult<()> {
    let expression = compile_rule(&schema.rules.create, context, false)?;
    if expression == "true" {
        return Ok(());
    }
    let mut response = db
        .inner()
        .query(format!("RETURN ({expression})"))
        .bind(("hb_auth", context.auth.clone()))
        .bind(("hb_auth_record", context.auth_record.clone()))
        .bind(("hb_record", native_record_value(schema, record)?))
        .bind(("hb_request", json_request(&context.request_body)))
        .await
        .map_err(database_error)?
        .check()
        .map_err(database_error)?;
    let allowed: Option<bool> = response.take(0).map_err(database_error)?;
    if allowed == Some(true) {
        Ok(())
    } else {
        Err(HbError::Forbidden)
    }
}

fn native_record_value(schema: &CollectionDef, record: &Value) -> HbResult<SurrealValue> {
    let object = record
        .as_object()
        .ok_or_else(|| HbError::validation("record body must be a JSON object"))?;
    let mut native = Object::new();
    for (name, value) in object {
        let Some(field) = schema.fields.iter().find(|field| field.name == *name) else {
            native.insert(name.clone(), value.clone());
            continue;
        };
        if field.field_type != FieldType::Relation || value.is_null() {
            native.insert(name.clone(), value.clone());
            continue;
        }
        if relation_is_many(field.options.as_ref()) {
            let values = value
                .as_array()
                .ok_or_else(|| HbError::validation("relation value must be an array"))?
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .ok_or_else(|| HbError::validation("relation value must be a record id"))
                        .and_then(parse_record_id)
                })
                .collect::<HbResult<Vec<_>>>()?;
            native.insert(name.clone(), values);
        } else {
            let id = value
                .as_str()
                .ok_or_else(|| HbError::validation("relation value must be a record id"))
                .and_then(parse_record_id)?;
            native.insert(name.clone(), id);
        }
    }
    Ok(SurrealValue::Object(native))
}

fn sanitize_records(schema: &CollectionDef, records: &mut [Value]) {
    for record in records {
        sanitize_record(schema, record);
    }
}

pub(crate) fn sanitize_record(schema: &CollectionDef, record: &mut Value) {
    if schema.collection_type != CollectionType::Auth {
        return;
    }
    if let Some(object) = record.as_object_mut() {
        for field in [
            "password_hash",
            "token_key",
            "failed_attempts",
            "locked_until",
        ] {
            object.remove(field);
        }
    }
}

fn sanitize_sensitive_expansion(value: &mut Value) {
    match value {
        Value::Array(values) => values.iter_mut().for_each(sanitize_sensitive_expansion),
        Value::Object(object) => {
            for field in [
                "password_hash",
                "token_key",
                "failed_attempts",
                "locked_until",
            ] {
                object.remove(field);
            }
            object.values_mut().for_each(sanitize_sensitive_expansion);
        }
        _ => {}
    }
}

pub(crate) fn allowed_fields(schema: &CollectionDef) -> HashSet<String> {
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

pub fn parse_record_id(value: &str) -> HbResult<RecordId> {
    let (table, key) = value
        .split_once(':')
        .ok_or_else(|| HbError::validation("invalid relation record id"))?;
    let key = key
        .strip_prefix('`')
        .and_then(|key| key.strip_suffix('`'))
        .unwrap_or(key);
    if table.is_empty()
        || !table
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        || key.is_empty()
        || key.len() > 255
        || !key
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(HbError::validation("invalid relation record id"));
    }
    Ok(RecordId::new(table, key))
}

fn route_record_id(collection: &str, value: &str) -> HbResult<RecordId> {
    let (table, key) = value
        .split_once(':')
        .map_or((collection, value), |parts| parts);
    if table != collection {
        return Err(HbError::NotFound);
    }
    let key = key
        .strip_prefix('`')
        .and_then(|key| key.strip_suffix('`'))
        .unwrap_or(key);
    if key.is_empty()
        || key.len() > 255
        || !key
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(HbError::NotFound);
    }
    Ok(record_id(collection, key))
}

#[derive(Debug, Clone)]
struct ExpandPath {
    segments: Vec<String>,
    targets: Vec<String>,
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
        let mut targets = Vec::new();
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
            targets.push(target.to_owned());
            current = manager.get_collection(target).await?;
        }
        paths.push(ExpandPath { segments, targets });
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

pub(crate) fn normalize_value(value: &mut Value) {
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

    #[test]
    fn normalizes_record_id_strings_and_backtick_forms() {
        let mut value = serde_json::json!({
            "string": "posts:one",
            "quoted": "posts:`two-three`",
            "object": {"table": "posts", "key": "four"},
            "numeric": {"table": "posts", "key": 5}
        });
        normalize_value(&mut value);
        assert_eq!(value["string"], "posts:one");
        assert_eq!(value["quoted"], "posts:two-three");
        assert_eq!(value["object"], "posts:four");
        assert_eq!(value["numeric"], "posts:5");
    }
}
