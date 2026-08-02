use std::sync::Arc;

use herta_core::HbResult;
use herta_db::{CollectionDef, FieldDef, FieldType, SchemaManager, relation_is_many};
use serde_json::{Map, Value, json};
use tokio::sync::RwLock;

use crate::router::ApiState;

#[derive(Clone)]
pub struct OpenApiCache(Arc<RwLock<Value>>);

impl OpenApiCache {
    pub fn empty() -> Self {
        Self(Arc::new(RwLock::new(generate_document(&[]))))
    }

    pub async fn read(&self) -> Value {
        self.0.read().await.clone()
    }

    pub async fn refresh(&self, state: &ApiState) -> HbResult<()> {
        let collections = SchemaManager::new(&state.db).list_collections().await?;
        *self.0.write().await = generate_document(&collections);
        Ok(())
    }
}

pub fn generate_document(collections: &[CollectionDef]) -> Value {
    let mut paths = base_paths();
    let mut schemas = base_schemas();
    for collection in collections {
        add_collection_paths(&mut paths, collection);
        add_collection_schemas(&mut schemas, collection);
    }
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "HertaBase API",
            "version": env!("CARGO_PKG_VERSION")
        },
        "paths": paths,
        "components": { "schemas": schemas }
    })
}

fn base_paths() -> Map<String, Value> {
    let mut paths = Map::new();
    paths.insert(
        "/_/collections".into(),
        json!({
            "get": operation("List collections", "CollectionListEnvelope", false),
            "post": operation_with_body("Create collection", "Collection", "CollectionEnvelope")
        }),
    );
    paths.insert(
        "/_/collections/{name}".into(),
        json!({
            "get": operation("Get collection", "CollectionEnvelope", true),
            "patch": operation_with_body("Add fields or indexes", "CollectionPatch", "CollectionEnvelope"),
            "delete": operation("Delete collection", "GenericEnvelope", true)
        }),
    );
    paths
}

fn base_schemas() -> Map<String, Value> {
    let mut schemas = Map::new();
    schemas.insert(
        "ApiError".into(),
        json!({
            "type": "object",
            "required": ["code", "message", "error"],
            "properties": {
                "code": {"type": "integer"},
                "message": {"type": "string"},
                "error": {"type": "string", "pattern": "^HB_"},
                "details": {}
            }
        }),
    );
    schemas.insert("Collection".into(), collection_schema());
    schemas.insert(
        "CollectionPatch".into(),
        json!({
            "type": "object",
            "properties": {
                "fields": {"type": "array", "items": {"type": "object"}},
                "indexes": {"type": "array", "items": {"type": "object"}}
            }
        }),
    );
    schemas.insert(
        "CollectionEnvelope".into(),
        envelope(json!({"$ref": "#/components/schemas/Collection"})),
    );
    schemas.insert(
        "CollectionListEnvelope".into(),
        envelope(json!({"type": "array", "items": {"$ref": "#/components/schemas/Collection"}})),
    );
    schemas.insert("GenericEnvelope".into(), envelope(json!({})));
    schemas
}

fn collection_schema() -> Value {
    json!({
        "type": "object",
        "required": ["name", "type", "schema_mode", "fields"],
        "properties": {
            "name": {"type": "string", "pattern": "^[A-Za-z][A-Za-z0-9_]*$"},
            "type": {"type": "string", "enum": ["base"]},
            "schema_mode": {"type": "string", "enum": ["schema-less", "strict", "mixed"]},
            "fields": {"type": "array", "items": {"type": "object"}},
            "indexes": {"type": "array", "items": {"type": "object"}}
        }
    })
}

fn add_collection_paths(paths: &mut Map<String, Value>, collection: &CollectionDef) {
    let record = format!("{}Record", collection.name);
    let create = format!("{}Create", collection.name);
    let update = format!("{}Update", collection.name);
    let envelope_name = format!("{}Envelope", collection.name);
    let list_name = format!("{}ListEnvelope", collection.name);
    let root = format!("/api/collections/{}/records", collection.name);
    paths.insert(
        root.clone(),
        json!({
            "get": {
                "summary": format!("List {} records", collection.name),
                "parameters": list_parameters(),
                "responses": success_response(&list_name)
            },
            "post": {
                "summary": format!("Create a {} record", collection.name),
                "requestBody": json_body(&create),
                "responses": success_response(&envelope_name)
            }
        }),
    );
    paths.insert(
        format!("{root}/{{id}}"),
        json!({
            "parameters": [path_parameter("id")],
            "get": {
                "summary": format!("Get a {} record", collection.name),
                "parameters": [{"name": "expand", "in": "query", "schema": {"type": "string"}}],
                "responses": success_response(&envelope_name)
            },
            "patch": {
                "summary": format!("Update a {} record", collection.name),
                "requestBody": json_body(&update),
                "responses": success_response(&envelope_name)
            },
            "delete": {
                "summary": format!("Soft-delete a {} record", collection.name),
                "responses": success_response(&envelope_name)
            }
        }),
    );
    let _ = record;
}

fn add_collection_schemas(schemas: &mut Map<String, Value>, collection: &CollectionDef) {
    let mut record_properties = Map::new();
    record_properties.insert("id".into(), json!({"type": "string", "readOnly": true}));
    record_properties.insert(
        "created_at".into(),
        json!({"type": "string", "format": "date-time", "readOnly": true}),
    );
    record_properties.insert(
        "updated_at".into(),
        json!({"type": "string", "format": "date-time", "readOnly": true}),
    );
    record_properties.insert(
        "deleted_at".into(),
        json!({"type": ["string", "null"], "format": "date-time", "readOnly": true}),
    );
    record_properties.insert(
        "expand".into(),
        json!({"type": "object", "readOnly": true, "additionalProperties": true}),
    );
    let mut request_properties = Map::new();
    let mut required = Vec::new();
    for field in &collection.fields {
        let schema = field_schema(field);
        record_properties.insert(field.name.clone(), schema.clone());
        request_properties.insert(field.name.clone(), schema);
        if field.required {
            required.push(Value::String(field.name.clone()));
        }
    }
    let record_name = format!("{}Record", collection.name);
    let create_name = format!("{}Create", collection.name);
    let update_name = format!("{}Update", collection.name);
    schemas.insert(
        record_name.clone(),
        json!({"type": "object", "properties": record_properties}),
    );
    schemas.insert(
        create_name,
        json!({"type": "object", "required": required, "properties": request_properties}),
    );
    schemas.insert(
        update_name,
        json!({"type": "object", "properties": request_properties}),
    );
    schemas.insert(
        format!("{}Envelope", collection.name),
        envelope(json!({"$ref": format!("#/components/schemas/{record_name}")})),
    );
    schemas.insert(format!("{}ListEnvelope", collection.name), envelope(json!({"type": "array", "items": {"$ref": format!("#/components/schemas/{record_name}")}})));
}

fn field_schema(field: &FieldDef) -> Value {
    match field.field_type {
        FieldType::Text | FieldType::File => json!({"type": "string"}),
        FieldType::Number => json!({"type": "number"}),
        FieldType::Bool => json!({"type": "boolean"}),
        FieldType::Datetime => json!({"type": "string", "format": "date-time"}),
        FieldType::Json => json!({}),
        FieldType::Email => json!({"type": "string", "format": "email"}),
        FieldType::Url => json!({"type": "string", "format": "uri"}),
        FieldType::Select => json!({
            "type": "string",
            "enum": field.options.as_ref().and_then(|v| v.get("values")).cloned().unwrap_or(json!([]))
        }),
        FieldType::Relation if relation_is_many(field.options.as_ref()) => {
            json!({"type": "array", "items": {"type": "string"}})
        }
        FieldType::Relation => json!({"type": "string"}),
    }
}

fn envelope(data: Value) -> Value {
    json!({
        "type": "object",
        "required": ["data", "meta", "error"],
        "properties": {
            "data": data,
            "meta": {},
            "error": {"oneOf": [{"$ref": "#/components/schemas/ApiError"}, {"type": "null"}]}
        }
    })
}

fn operation(summary: &str, response: &str, path_name: bool) -> Value {
    let mut value = json!({"summary": summary, "responses": success_response(response)});
    if path_name {
        value["parameters"] = json!([path_parameter("name")]);
    }
    value
}

fn operation_with_body(summary: &str, body: &str, response: &str) -> Value {
    json!({"summary": summary, "requestBody": json_body(body), "responses": success_response(response)})
}

fn json_body(schema: &str) -> Value {
    json!({"required": true, "content": {"application/json": {"schema": {"$ref": format!("#/components/schemas/{schema}")}}}})
}

fn success_response(schema: &str) -> Value {
    json!({
        "200": {"description": "Success", "content": {"application/json": {"schema": {"$ref": format!("#/components/schemas/{schema}")}}}},
        "400": {"description": "Invalid request", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/GenericEnvelope"}}}},
        "404": {"description": "Not found", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/GenericEnvelope"}}}}
    })
}

fn path_parameter(name: &str) -> Value {
    json!({"name": name, "in": "path", "required": true, "schema": {"type": "string"}})
}

fn list_parameters() -> Value {
    json!([
        {"name": "page", "in": "query", "schema": {"type": "integer", "minimum": 1, "default": 1}},
        {"name": "perPage", "in": "query", "schema": {"type": "integer", "minimum": 1, "maximum": 500, "default": 30}},
        {"name": "sort", "in": "query", "schema": {"type": "string"}},
        {"name": "filter", "in": "query", "schema": {"type": "string"}},
        {"name": "expand", "in": "query", "schema": {"type": "string"}}
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use herta_db::{CollectionType, SchemaMode};

    #[test]
    fn dynamic_document_contains_collection_paths_and_schema() {
        let collection = CollectionDef {
            name: "posts".into(),
            collection_type: CollectionType::Base,
            schema_mode: SchemaMode::Strict,
            fields: vec![FieldDef {
                name: "title".into(),
                field_type: FieldType::Text,
                required: true,
                options: None,
            }],
            indexes: vec![],
        };
        let document = generate_document(&[collection]);
        assert!(document["paths"]["/api/collections/posts/records"].is_object());
        assert_eq!(
            document["components"]["schemas"]["postsCreate"]["required"][0],
            "title"
        );
    }
}
