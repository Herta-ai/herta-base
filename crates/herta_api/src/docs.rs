use std::sync::Arc;

use herta_core::HbResult;
use herta_db::{
    CollectionDef, CollectionType, FieldDef, FieldType, SchemaManager, relation_is_many,
};
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
        if collection.collection_type == CollectionType::Auth {
            add_auth_collection_paths(&mut paths, collection);
        }
    }
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "HertaBase API",
            "version": env!("CARGO_PKG_VERSION")
        },
        "paths": paths,
        "components": {
            "schemas": schemas,
            "securitySchemes": {
                "bearerAuth": {"type": "http", "scheme": "bearer", "bearerFormat": "JWT"}
            }
        }
    })
}

fn base_paths() -> Map<String, Value> {
    let mut paths = Map::new();
    paths.insert(
        "/api/realtime/{collection}".into(),
        json!({
            "get": {
                "summary": "Subscribe to collection changes",
                "security": [{}, {"bearerAuth": []}],
                "parameters": [
                    path_parameter("collection"),
                    {"name": "filter", "in": "query", "required": false,
                     "schema": {"type": "string"}},
                    {"name": "token", "in": "query", "required": false,
                     "schema": {"type": "string"}}
                ],
                "responses": {
                    "200": {
                        "description": "Server-sent collection events",
                        "content": {"text/event-stream": {"schema": {"type": "string"}}}
                    },
                    "401": {"description": "Authentication required"},
                    "403": {"description": "Access forbidden"},
                    "429": {"description": "Connection limit exceeded"}
                }
            }
        }),
    );
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
    paths.insert(
        "/api/auth/register".into(),
        json!({"post": auth_operation("Register a user", "_usersRegister", false)}),
    );
    paths.insert(
        "/api/auth/login".into(),
        json!({"post": auth_operation("Log in a user", "Credentials", false)}),
    );
    paths.insert(
        "/api/auth/refresh".into(),
        json!({"post": auth_operation("Rotate a user token pair", "RefreshRequest", false)}),
    );
    paths.insert(
        "/api/auth/me".into(),
        json!({"get": secured_operation("Get the current user", "AuthUserEnvelope")}),
    );
    paths.insert(
        "/api/admin/auth/login".into(),
        json!({"post": auth_operation("Log in an administrator", "Credentials", false)}),
    );
    paths.insert(
        "/api/admin/auth/refresh".into(),
        json!({"post": auth_operation("Rotate an administrator token pair", "RefreshRequest", false)}),
    );
    paths.insert(
        "/api/admin/auth/me".into(),
        json!({"get": secured_operation("Get the current administrator", "AuthUserEnvelope")}),
    );
    paths.insert(
        "/api/admin/logs".into(),
        json!({
            "get": {
                "summary": "List persisted server and request logs",
                "security": [{"bearerAuth": []}],
                "parameters": log_list_parameters(),
                "responses": {
                    "200": {
                        "description": "Log list",
                        "content": {"application/json": {
                            "schema": {"$ref": "#/components/schemas/LogListEnvelope"}
                        }}
                    },
                    "400": {"description": "Invalid query parameters"},
                    "401": {"description": "Authentication required"},
                    "403": {"description": "Administrator access required"}
                }
            }
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
    schemas.insert("ApiRules".into(), rules_schema());
    schemas.insert(
        "Credentials".into(),
        json!({
            "type": "object",
            "required": ["email", "password"],
            "properties": {
                "email": {"type": "string", "format": "email"},
                "password": {"type": "string", "format": "password", "minLength": 12}
            }
        }),
    );
    schemas.insert(
        "RefreshRequest".into(),
        json!({
            "type": "object",
            "required": ["refreshToken"],
            "properties": {"refreshToken": {"type": "string"}}
        }),
    );
    schemas.insert(
        "AuthUser".into(),
        json!({
            "type": "object",
            "required": ["id", "collection", "email", "role", "verified", "admin"],
            "properties": {
                "id": {"type": "string"}, "collection": {"type": "string"},
                "email": {"type": "string", "format": "email"}, "role": {"type": "string"},
                "verified": {"type": "boolean"}, "admin": {"type": "boolean"}
            }
        }),
    );
    schemas.insert(
        "AuthUserEnvelope".into(),
        envelope(json!({"$ref": "#/components/schemas/AuthUser"})),
    );
    schemas.insert(
        "AuthEnvelope".into(),
        envelope(json!({
            "type": "object",
            "required": ["accessToken", "refreshToken", "tokenType", "expiresIn", "user"],
            "properties": {
                "accessToken": {"type": "string"}, "refreshToken": {"type": "string"},
                "tokenType": {"type": "string", "const": "Bearer"}, "expiresIn": {"type": "integer"},
                "user": {"$ref": "#/components/schemas/AuthUser"}
            }
        })),
    );
    schemas.insert(
        "CollectionPatch".into(),
        json!({
            "type": "object",
            "properties": {
                "fields": {"type": "array", "items": {"type": "object"}},
                "indexes": {"type": "array", "items": {"type": "object"}}
                ,"rules": {"$ref": "#/components/schemas/ApiRules"}
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
    schemas.insert(
        "LogRecord".into(),
        json!({
            "type": "object",
            "required": ["id", "created_at", "log_type", "level", "message", "target"],
            "properties": {
                "id": {"type": "string", "readOnly": true},
                "created_at": {"type": "string", "format": "date-time", "readOnly": true},
                "log_type": {"type": "string", "enum": ["server", "request"]},
                "level": {"type": "string", "enum": ["trace", "debug", "info", "warn", "error"]},
                "message": {"type": "string"},
                "target": {"type": "string"},
                "method": {"type": ["string", "null"]},
                "path": {"type": ["string", "null"]},
                "status_code": {"type": ["integer", "null"]},
                "referer": {"type": ["string", "null"]},
                "remote_ip": {"type": ["string", "null"]},
                "user_agent": {"type": ["string", "null"]},
                "auth_type": {"type": ["string", "null"]},
                "user_id": {"type": ["string", "null"]},
                "user_collection": {"type": ["string", "null"]}
            }
        }),
    );
    schemas.insert(
        "LogListEnvelope".into(),
        envelope(json!({
            "type": "array",
            "items": {"$ref": "#/components/schemas/LogRecord"}
        })),
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
            "type": {"type": "string", "enum": ["base", "auth"]},
            "schema_mode": {"type": "string", "enum": ["schema-less", "strict", "mixed"]},
            "fields": {"type": "array", "items": {"type": "object"}},
            "indexes": {"type": "array", "items": {"type": "object"}},
            "rules": {"$ref": "#/components/schemas/ApiRules"}
        }
    })
}

fn rules_schema() -> Value {
    let rule = json!({"oneOf": [{"type": "null"}, {"type": "boolean"}, {"type": "string"}]});
    json!({
        "type": "object",
        "properties": {
            "list": rule.clone(), "view": rule.clone(), "create": rule.clone(),
            "update": rule.clone(), "delete": rule
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
                "security": [{}, {"bearerAuth": []}],
                "parameters": list_parameters(),
                "responses": success_response(&list_name)
            },
            "post": {
                "summary": format!("Create a {} record", collection.name),
                "security": [{}, {"bearerAuth": []}],
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
                "security": [{}, {"bearerAuth": []}],
                "parameters": [{"name": "expand", "in": "query", "schema": {"type": "string"}}],
                "responses": success_response(&envelope_name)
            },
            "patch": {
                "summary": format!("Update a {} record", collection.name),
                "security": [{}, {"bearerAuth": []}],
                "requestBody": json_body(&update),
                "responses": success_response(&envelope_name)
            },
            "delete": {
                "summary": format!("Soft-delete a {} record", collection.name),
                "security": [{}, {"bearerAuth": []}],
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
        json!({"type": "object", "required": required.clone(), "properties": request_properties.clone()}),
    );
    schemas.insert(
        update_name,
        json!({"type": "object", "properties": request_properties.clone()}),
    );
    schemas.insert(
        format!("{}Envelope", collection.name),
        envelope(json!({"$ref": format!("#/components/schemas/{record_name}")})),
    );
    schemas.insert(format!("{}ListEnvelope", collection.name), envelope(json!({"type": "array", "items": {"$ref": format!("#/components/schemas/{record_name}")}})));
    if collection.collection_type == CollectionType::Auth {
        let mut register_properties = request_properties;
        register_properties.insert("email".into(), json!({"type": "string", "format": "email"}));
        register_properties.insert(
            "password".into(),
            json!({"type": "string", "format": "password", "minLength": 12}),
        );
        let mut register_required = required;
        register_required.push(json!("email"));
        register_required.push(json!("password"));
        schemas.insert(
            format!("{}Register", collection.name),
            json!({"type": "object", "required": register_required, "properties": register_properties}),
        );
    }
}

fn add_auth_collection_paths(paths: &mut Map<String, Value>, collection: &CollectionDef) {
    let root = format!("/api/auth/{}", collection.name);
    paths.insert(format!("{root}/register"), json!({
        "post": auth_operation("Register an auth collection user", &format!("{}Register", collection.name), false)
    }));
    paths.insert(
        format!("{root}/login"),
        json!({
            "post": auth_operation("Log in an auth collection user", "Credentials", false)
        }),
    );
    paths.insert(
        format!("{root}/refresh"),
        json!({
            "post": auth_operation("Rotate an auth collection token pair", "RefreshRequest", false)
        }),
    );
    paths.insert(
        format!("{root}/me"),
        json!({
            "get": secured_operation("Get the current auth collection user", "AuthUserEnvelope")
        }),
    );
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
    let mut value = json!({"summary": summary, "security": [{"bearerAuth": []}], "responses": success_response(response)});
    if path_name {
        value["parameters"] = json!([path_parameter("name")]);
    }
    value
}

fn operation_with_body(summary: &str, body: &str, response: &str) -> Value {
    json!({"summary": summary, "security": [{"bearerAuth": []}], "requestBody": json_body(body), "responses": success_response(response)})
}

fn secured_operation(summary: &str, response: &str) -> Value {
    json!({"summary": summary, "security": [{"bearerAuth": []}], "responses": success_response(response)})
}

fn auth_operation(summary: &str, body: &str, secured: bool) -> Value {
    let mut operation = json!({
        "summary": summary,
        "requestBody": json_body(body),
        "responses": success_response("AuthEnvelope")
    });
    if secured {
        operation["security"] = json!([{"bearerAuth": []}]);
    }
    operation
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

fn log_list_parameters() -> Value {
    json!([
        {"name": "page", "in": "query", "schema": {"type": "integer", "minimum": 1, "default": 1}},
        {"name": "perPage", "in": "query", "schema": {"type": "integer", "minimum": 1, "maximum": 500, "default": 30}},
        {"name": "level", "in": "query", "schema": {"type": "string", "enum": ["trace", "debug", "info", "warn", "error"]}},
        {"name": "logType", "in": "query", "schema": {"type": "string", "enum": ["server", "request"]}},
        {"name": "q", "in": "query", "schema": {"type": "string", "maxLength": 256}},
        {"name": "target", "in": "query", "schema": {"type": "string"}},
        {"name": "path", "in": "query", "schema": {"type": "string"}},
        {"name": "statusCode", "in": "query", "schema": {"type": "integer", "minimum": 100, "maximum": 599}},
        {"name": "from", "in": "query", "schema": {"type": "string", "format": "date-time"}},
        {"name": "to", "in": "query", "schema": {"type": "string", "format": "date-time"}}
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
            rules: Default::default(),
        };
        let document = generate_document(&[collection]);
        assert!(document["paths"]["/api/collections/posts/records"].is_object());
        assert!(document["paths"]["/api/admin/logs"].is_object());
        assert_eq!(
            document["components"]["schemas"]["postsCreate"]["required"][0],
            "title"
        );
        assert!(document["components"]["schemas"]["LogRecord"].is_object());
    }
}
