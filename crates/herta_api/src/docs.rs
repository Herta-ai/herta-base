use std::sync::Arc;

use herta_core::HbResult;
use herta_db::{
    CollectionDef, CollectionType, FieldDef, FieldType, SchemaManager, file_is_many,
    relation_is_many,
};
use serde_json::{Map, Value, json};
use tokio::sync::RwLock;

use crate::router::ApiState;

#[derive(Clone)]
pub struct OpenApiCache(Arc<RwLock<Value>>, Arc<tokio::sync::Mutex<()>>);

impl OpenApiCache {
    pub fn empty() -> Self {
        Self(
            Arc::new(RwLock::new(generate_document(&[]))),
            Arc::new(tokio::sync::Mutex::new(())),
        )
    }

    pub async fn read(&self) -> Value {
        self.0.read().await.clone()
    }
    pub(crate) async fn collection_effects_lock(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.1.lock().await
    }

    pub async fn refresh(&self, state: &ApiState) -> HbResult<()> {
        self.refresh_db(&state.db).await
    }

    pub async fn refresh_db(&self, db: &herta_db::DbClient) -> HbResult<()> {
        // Serialize reads as well as publication so an older read cannot
        // overwrite a document generated after a newer schema commit.
        let mut document = self.0.write().await;
        let collections = SchemaManager::new(db).list_collections().await?;
        *document = generate_document(&collections);
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
                "bearerAuth": {"type": "http", "scheme": "bearer", "bearerFormat": "JWT"},
                "operationCredential": {"type":"apiKey","in":"header","name":"X-HB-Operation-Credential"}
            }
        }
    })
}

fn base_paths() -> Map<String, Value> {
    let mut paths = Map::new();
    paths.insert("/api/admin/file-operations".into(),json!({"get":{
        "summary":"List uncertain file writes (administrator only)","security":[{"bearerAuth":[]}],
        "parameters":[{"name":"limit","in":"query","schema":{"type":"integer","minimum":1,"maximum":500,"default":100}},
            {"name":"cursor","in":"query","schema":{"type":"string","format":"uuid"}}],
        "responses":{"200":{"description":"File metadata page in the configured namespace, without physical locations"},"401":{"description":"Authentication required"},"403":{"description":"Administrator required"}}
    }}));
    paths.insert("/api/admin/file-operations/{version}".into(),json!({"get":{
        "summary":"Get a file operation or immutable resolution receipt (administrator only)","security":[{"bearerAuth":[]}],"parameters":[path_parameter("version")],
        "responses":{"200":{"description":"Metadata, reservation state, and resolution audit when present"},"404":{"description":"Version not found"}}
    }}));
    paths.insert("/api/admin/file-operations/{version}/resolve".into(),json!({"post":{
        "summary":"Release a verified absent file write (administrator only)","security":[{"bearerAuth":[]}],"parameters":[path_parameter("version")],
        "description":"Administrator attests that the backend rejected the write and no pending request can create the object. Current HEAD absence is also required. Repeats return the original audit receipt.",
        "requestBody":{"required":true,"content":{"application/json":{"schema":{"type":"object","additionalProperties":false,"required":["resolution","note"],
            "properties":{"resolution":{"type":"string","enum":["not_written"]},"note":{"type":"string","minLength":1,"maxLength":2000}}}}}},
        "responses":{"200":{"description":"Quota released atomically with durable resolution audit"},"409":{"description":"Object exists or reservation is not uncertain"}}
    }}));
    paths.insert("/api/admin/outbox".into(),json!({"get":{
        "summary":"List outbox receipts (administrator only)","security":[{"bearerAuth":[]}],
        "parameters":[{"name":"limit","in":"query","schema":{"type":"integer","minimum":1,"maximum":500,"default":100}},
            {"name":"cursor","in":"query","schema":{"type":"string"}}],
        "responses":{"200":{"description":"Bounded receipt page, without message payloads or credentials"},"401":{"description":"Authentication required"},"403":{"description":"Administrator required"}}
    }}));
    paths.insert("/api/admin/outbox/{id}".into(),json!({"get":{
        "summary":"Get an outbox receipt (administrator only)","security":[{"bearerAuth":[]}],"parameters":[path_parameter("id")],
        "responses":{"200":{"description":"Job state and native receipt"},"404":{"description":"Job not found"}}
    }}));
    paths.insert("/api/admin/outbox/{id}/resolve".into(),json!({"post":{
        "summary":"Resolve a verified unknown delivery (administrator only)","security":[{"bearerAuth":[]}],"parameters":[path_parameter("id")],
        "requestBody":{"required":true,"content":{"application/json":{"schema":{"type":"object","additionalProperties":false,"required":["resolution","note"],
            "properties":{"resolution":{"type":"string","enum":["accepted","not_sent"]},"note":{"type":"string","minLength":1,"maxLength":2000}}}}}},
        "responses":{"200":{"description":"Updated receipt; not_sent queues a new attempt"},"409":{"description":"Job is not unknown"}}
    }}));
    paths.insert("/api/events".into(),json!({"get":{
        "summary":"Subscribe to application messages",
        "description":"One authenticated connection per exact topic; connected/message/ping/error events. No replay. Shares connection quotas with collection SSE and revalidates identity before delivery.",
        "security":[{"bearerAuth":[]}],
        "parameters":[{"name":"topic","in":"query","required":true,"schema":{"type":"string","maxLength":128}},
            {"name":"token","in":"query","required":false,"schema":{"type":"string"}}],
        "responses":{"200":{"description":"Application message stream","content":{"text/event-stream":{"schema":{"type":"string"}}}},
            "401":{"description":"Authentication required"},"403":{"description":"Application messages not enabled"},"429":{"description":"Connection quota exhausted"}}
    }}));
    paths.insert("/api/operations/{id}".into(),json!({"get":{
        "summary":"Verify a transaction outcome",
        "description":"Accessible with the original identity or the random verification credential. Returns status only, never records or Auth tokens. Responses are not cached.",
        "security":[{"bearerAuth":[]},{"operationCredential":[]}],
        "parameters":[path_parameter("id")],
        "responses":{"200":{"description":"Operation status","content":{"application/json":{"schema":{"$ref":"#/components/schemas/OperationStatusEnvelope"}}}},
            "404":{"description":"Unknown operation or verification not authorized"}}
    }}));
    paths.insert("/api/admin/mail/send".into(), json!({"post": {
        "summary": "Send an email (administrator only)",
        "description": "Success means SMTP acceptance, not final delivery. Submission is not retried automatically.",
        "security": [{"bearerAuth": []}],
        "requestBody": json_body("MailMessage"),
        "responses": {
            "200": {"description": "SMTP accepted the message", "content": {"application/json": {
                "schema": {"$ref": "#/components/schemas/MailReceiptEnvelope"}
            }}},
            "400": {"description": "Invalid message"},
            "401": {"description": "Authentication required"},
            "403": {"description": "Administrator access required"},
            "413": {"description": "Message exceeds configured limits"},
            "502": {"description": "SMTP submission failed"},
            "503": {"description": "Mail service disabled"},
            "504": {"description": "Submission timed out; acceptance may be unknown"}
        }
    }}));
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
    paths.insert(
        "/_/web-projects".into(),
        json!({
            "get": secured_operation("List deployed web projects", "WebProjectListEnvelope"),
            "post": {
                "summary": "Deploy a web project archive",
                "security": [{"bearerAuth": []}],
                "requestBody": {
                    "required": true,
                    "content": {"multipart/form-data": {"schema": {
                        "type": "object",
                        "required": ["archive"],
                        "properties": {
                            "archive": {"type": "string", "format": "binary"},
                            "alias": {"type": "string"},
                            "spaFallback": {"type": "boolean", "default": true},
                            "cacheControl": {"type": "string"},
                            "notFound": {"type": "string"}
                        }
                    }}}
                },
                "responses": {
                    "201": {"description": "Project created"},
                    "200": {"description": "Project updated"},
                    "400": {"description": "Invalid archive or settings"},
                    "401": {"description": "Authentication required"},
                    "403": {"description": "Administrator access required"},
                    "413": {"description": "Archive is too large"}
                }
            }
        }),
    );
    paths.insert(
        "/_/web-projects/{project}".into(),
        json!({
            "parameters": [path_parameter("project")],
            "get": secured_operation("Get a deployed web project", "WebProjectEnvelope"),
            "patch": {
                "summary": "Update web project routing settings",
                "security": [{"bearerAuth": []}],
                "requestBody": json_body("WebProjectPatch"),
                "responses": success_response("WebProjectEnvelope")
            },
            "delete": secured_operation("Delete a deployed web project", "GenericEnvelope")
        }),
    );
    paths.insert(
        "/_/web-projects/{project}/versions".into(),
        json!({
            "parameters": [path_parameter("project")],
            "get": secured_operation("List web project backup versions", "WebVersionsEnvelope")
        }),
    );
    paths.insert(
        "/_/web-projects/{project}/rollback".into(),
        json!({
            "parameters": [path_parameter("project")],
            "post": {
                "summary": "Rollback a web project",
                "security": [{"bearerAuth": []}],
                "requestBody": json_body("WebRollbackRequest"),
                "responses": success_response("WebProjectEnvelope")
            }
        }),
    );
    paths.insert(
        "/api/files/token".into(),
        json!({"post": {
            "summary": "Issue a short-lived file token",
            "security": [{"bearerAuth": []}],
            "requestBody": json_body("FileTokenRequest"),
            "responses": success_response("FileTokenEnvelope")
        }}),
    );
    paths.insert(
        "/api/files/{collection}/{recordId}/{field}/{filename}".into(),
        json!({
            "parameters": [
                path_parameter("collection"), path_parameter("recordId"),
                path_parameter("field"), path_parameter("filename"),
                {"name": "token", "in": "query", "required": false, "schema": {"type": "string"}},
                {"name": "Range", "in": "header", "required": false, "schema": {"type": "string"}},
                {"name": "If-None-Match", "in": "header", "required": false, "schema": {"type": "string"}}
            ],
            "get": file_download_operation("Download a record file"),
            "head": file_download_operation("Read record file metadata")
        }),
    );
    paths
}

fn base_schemas() -> Map<String, Value> {
    let mut schemas = Map::new();
    schemas.insert("OperationStatusEnvelope".into(),envelope(json!({"type":"object","additionalProperties":false,
        "required":["operationId","state"],"properties":{"operationId":{"type":"string"},"state":{"type":"string","enum":["pending","committing","committed","rolled_back","unknown"]}}})));
    schemas.insert("MailAddress".into(), json!({
        "type": "object", "additionalProperties": false, "required": ["address"],
        "properties": {"address": {"type": "string", "format": "email"}, "name": {"type": "string"}}
    }));
    schemas.insert("MailMessage".into(), json!({
        "type": "object", "additionalProperties": false, "required": ["to", "subject"],
        "description": "At least one nonempty text or html body is required. Sender and byte limits follow server mail configuration. No attachments, cc or bcc.",
        "properties": {
            "from": {"$ref": "#/components/schemas/MailAddress"},
            "to": {"type": "array", "minItems": 1, "items": {"$ref": "#/components/schemas/MailAddress"}},
            "subject": {"type": "string", "minLength": 1},
            "text": {"type": "string"}, "html": {"type": "string"},
            "headers": {"type": "object", "description": "X-* headers only; CR/LF prohibited", "additionalProperties": {"type": "string"}}
        },
        "example": {"to": [{"address": "reader@example.com"}], "subject": "HertaBase test", "text": "Hello from HertaBase"}
    }));
    schemas.insert("MailReceiptEnvelope".into(), envelope(json!({
        "type": "object", "required": ["messageId", "status"],
        "properties": {"messageId": {"type": "string"}, "status": {"type": "string", "enum": ["accepted"]}}
    })));
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
    schemas.insert(
        "WebProject".into(),
        json!({
            "type": "object",
            "required": ["name", "spaFallback", "cacheControl", "deployedAt", "deployed"],
            "properties": {
                "name": {"type": "string"},
                "alias": {"type": ["string", "null"]},
                "spaFallback": {"type": "boolean"},
                "cacheControl": {"type": "string"},
                "notFound": {"type": ["string", "null"]},
                "deployedAt": {"type": "string"},
                "deployed": {"type": "boolean"}
            }
        }),
    );
    schemas.insert(
        "WebProjectPatch".into(),
        json!({
            "type": "object",
            "properties": {
                "alias": {"type": ["string", "null"]},
                "spaFallback": {"type": "boolean"},
                "cacheControl": {"type": "string"},
                "notFound": {"type": ["string", "null"]}
            }
        }),
    );
    schemas.insert(
        "WebRollbackRequest".into(),
        json!({"type": "object", "required": ["version"], "properties": {"version": {"type": "string", "pattern": "^[0-9]{4}-[0-9]{2}-[0-9]{2}-[0-9]{2}-[0-9]{2}-[0-9]{2}$"}}}),
    );
    schemas.insert(
        "WebProjectEnvelope".into(),
        envelope(json!({"$ref": "#/components/schemas/WebProject"})),
    );
    schemas.insert(
        "WebProjectListEnvelope".into(),
        envelope(json!({"type": "array", "items": {"$ref": "#/components/schemas/WebProject"}})),
    );
    schemas.insert(
        "WebVersionsEnvelope".into(),
        envelope(json!({"type": "array", "items": {"type": "string"}})),
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
        "FileTokenRequest".into(),
        json!({
            "type": "object",
            "required": ["collection", "recordId", "field"],
            "properties": {
                "collection": {"type": "string"},
                "recordId": {"type": "string"},
                "field": {"type": "string"}
            }
        }),
    );
    schemas.insert(
        "FileTokenEnvelope".into(),
        envelope(json!({
            "type": "object",
            "required": ["token", "expiresIn"],
            "properties": {"token": {"type": "string"}, "expiresIn": {"type": "integer"}}
        })),
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
            "version": {"type":"string","readOnly":true,"description":"Opaque collection revision used by extension save operations"},
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
                "requestBody": record_body(collection, &create, true),
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
                "parameters": [{
                    "name": "appendFiles", "in": "query", "required": false,
                    "description": "Comma-separated multi-file fields to append instead of replace",
                    "schema": {"type": "string"}
                }],
                "requestBody": record_body(collection, &update, false),
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
    let mut update_properties = Map::new();
    let mut required = Vec::new();
    for field in &collection.fields {
        let schema = field_schema(field);
        record_properties.insert(field.name.clone(), schema.clone());
        if field.field_type != FieldType::File {
            request_properties.insert(field.name.clone(), schema);
        } else {
            update_properties.insert(
                field.name.clone(),
                json!({
                    "oneOf": [
                        {"type": "null"},
                        {"type": "array", "maxItems": 0}
                    ]
                }),
            );
        }
        if field.required && field.field_type != FieldType::File {
            required.push(Value::String(field.name.clone()));
        }
    }
    let record_name = format!("{}Record", collection.name);
    let create_name = format!("{}Create", collection.name);
    let update_name = format!("{}Update", collection.name);
    update_properties.extend(request_properties.clone());
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
        json!({"type": "object", "properties": update_properties}),
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
        FieldType::Text => json!({"type": "string"}),
        FieldType::File if file_is_many(field.options.as_ref()) => {
            json!({"type": "array", "items": {"type": "string"}, "readOnly": true})
        }
        FieldType::File => json!({"type": "string", "readOnly": true}),
        FieldType::Number => json!({"type": "number"}),
        FieldType::Bool => json!({"type": "boolean"}),
        FieldType::Datetime => json!({"type": "string", "format": "date-time"}),
        FieldType::Json => json!({
            "description": "Any JSON value. A top-level null clears an optional field and is invalid for a required field."
        }),
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

fn record_body(collection: &CollectionDef, data_schema: &str, create: bool) -> Value {
    let mut properties = Map::new();
    properties.insert(
        "data".into(),
        json!({"$ref": format!("#/components/schemas/{data_schema}")}),
    );
    let mut required = Vec::new();
    for field in collection
        .fields
        .iter()
        .filter(|field| field.field_type == FieldType::File)
    {
        let schema = if file_is_many(field.options.as_ref()) {
            json!({
                "type": "array",
                "maxItems": field.options.as_ref().and_then(|value| value.get("maxSelect")).and_then(Value::as_u64).unwrap_or(1),
                "items": {"type": "string", "format": "binary"}
            })
        } else {
            json!({"type": "string", "format": "binary"})
        };
        properties.insert(field.name.clone(), schema);
        if create && field.required {
            required.push(Value::String(field.name.clone()));
        }
    }
    json!({
        "required": true,
        "content": {
            "application/json": {"schema": {"$ref": format!("#/components/schemas/{data_schema}")}},
            "multipart/form-data": {
                "schema": {"type": "object", "required": required, "properties": properties},
                "encoding": {"data": {"contentType": "application/json"}}
            }
        }
    })
}

fn file_download_operation(summary: &str) -> Value {
    json!({
        "summary": summary,
        "security": [{}, {"bearerAuth": []}],
        "responses": {
            "200": {"description": "File content", "content": {"application/octet-stream": {"schema": {"type": "string", "format": "binary"}}}},
            "206": {"description": "Partial file content"},
            "304": {"description": "Not modified"},
            "401": {"description": "Authentication required"},
            "403": {"description": "Access forbidden"},
            "404": {"description": "File not found"},
            "416": {"description": "Range not satisfiable"}
        }
    })
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
            fields: vec![
                FieldDef {
                    name: "title".into(),
                    field_type: FieldType::Text,
                    required: true,
                    options: None,
                },
                FieldDef {
                    name: "metadata".into(),
                    field_type: FieldType::Json,
                    required: false,
                    options: None,
                },
            ],
            indexes: vec![],
            rules: Default::default(),
        };
        let document = generate_document(&[collection]);
        assert!(document["paths"]["/api/collections/posts/records"].is_object());
        assert_eq!(
            document["paths"]["/api/collections/posts/records/{id}"]["patch"]["parameters"][0]["name"],
            "appendFiles"
        );
        assert!(document["paths"]["/api/admin/logs"].is_object());
        assert_eq!(
            document["components"]["schemas"]["postsCreate"]["required"][0],
            "title"
        );
        assert_eq!(
            document["components"]["schemas"]["postsCreate"]["properties"]["metadata"]["description"],
            "Any JSON value. A top-level null clears an optional field and is invalid for a required field."
        );
        assert!(document["components"]["schemas"]["LogRecord"].is_object());
    }
}
