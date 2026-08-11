use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use herta_db::{FieldDef, FieldType, ListParams, RecordManager, SchemaManager, file_is_many};
use salvo::prelude::*;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    handlers::auth::{identity, rule_context},
    response::{ApiFailure, ApiResponse, parse_error},
    router::ApiState,
};

#[handler]
pub async fn list(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    let identity = identity(req, state).await?;
    let collection = path(req, "collection")?;
    let params = ListParams {
        page: req.query("page"),
        per_page: req.query("perPage"),
        sort: req.query("sort"),
        filter: req.query("filter"),
        expand: req.query("expand"),
    };
    let (records, total) = RecordManager::new(&state.db)
        .list_authorized(&collection, &params, &rule_context(&identity, Value::Null))
        .await?;
    res.render(Json(ApiResponse::with_meta(
        records,
        json!({"total": total, "page": params.page(), "perPage": params.per_page()}),
    )));
    Ok(())
}

#[handler]
pub async fn get(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    let identity = identity(req, state).await?;
    let record = RecordManager::new(&state.db)
        .get_authorized(
            &path(req, "collection")?,
            &path(req, "id")?,
            req.query::<String>("expand").as_deref(),
            &rule_context(&identity, Value::Null),
        )
        .await?;
    res.render(Json(ApiResponse::ok(record)));
    Ok(())
}

#[handler]
pub async fn create(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    let identity = identity(req, state).await?;
    let collection = path(req, "collection")?;
    let record = if is_multipart(req) {
        create_multipart(req, state, &collection, &identity).await?
    } else {
        let mut body: Value = req
            .parse_json_with_max_size(state.config.server.max_body_size)
            .await
            .map_err(parse_error)?;
        prepare_json_body(&state.db, &collection, &mut body).await?;
        RecordManager::new(&state.db)
            .create_authorized(&collection, body.clone(), &rule_context(&identity, body))
            .await?
    };
    res.status_code(StatusCode::CREATED);
    res.render(Json(ApiResponse::ok(record)));
    Ok(())
}

#[handler]
pub async fn update(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    let identity = identity(req, state).await?;
    let collection = path(req, "collection")?;
    let id = path(req, "id")?;
    let record = if is_multipart(req) {
        update_multipart(req, state, &collection, &id, &identity).await?
    } else {
        let mut body: Value = req
            .parse_json_with_max_size(state.config.server.max_body_size)
            .await
            .map_err(parse_error)?;
        prepare_json_body(&state.db, &collection, &mut body).await?;
        RecordManager::new(&state.db)
            .update_authorized(
                &collection,
                &id,
                body.clone(),
                &rule_context(&identity, body),
            )
            .await?
    };
    res.render(Json(ApiResponse::ok(record)));
    Ok(())
}

#[handler]
pub async fn delete(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    let identity = identity(req, state).await?;
    let record = RecordManager::new(&state.db)
        .delete_authorized(
            &path(req, "collection")?,
            &path(req, "id")?,
            &rule_context(&identity, Value::Null),
        )
        .await?;
    res.render(Json(ApiResponse::ok(record)));
    Ok(())
}

fn state(depot: &Depot) -> Result<&ApiState, ApiFailure> {
    depot
        .get_typed::<ApiState>()
        .map_err(|_| ApiFailure(herta_core::HbError::Internal))
}

fn path(req: &Request, name: &str) -> Result<String, ApiFailure> {
    req.param::<String>(name)
        .ok_or_else(|| parse_error(format!("missing path parameter '{name}'")))
}

fn is_multipart(req: &Request) -> bool {
    req.content_type().is_some_and(|content_type| {
        content_type.type_() == mime::MULTIPART && content_type.subtype() == mime::FORM_DATA
    })
}

#[derive(Debug)]
struct UploadPart {
    field: String,
    source: PathBuf,
    reference: String,
}

async fn create_multipart(
    req: &mut Request,
    state: &ApiState,
    collection: &str,
    identity: &herta_auth::AuthIdentity,
) -> Result<Value, ApiFailure> {
    let form = req
        .form_data_max_size(state.config.server.max_body_size)
        .await
        .map_err(parse_error)?;
    let mut data = parse_multipart_data(form)?;
    let id = Uuid::now_v7().to_string();
    let schema = SchemaManager::new(&state.db)
        .get_collection(collection)
        .await?;
    reject_file_references(&schema, &data)?;
    normalize_file_clears(&schema, &mut data);
    let uploads = collect_uploads(form, &schema, state.config.storage.max_file_size)?;
    apply_uploaded_references(&mut data, &schema.fields, &uploads)?;
    let context = rule_context(identity, data.clone());
    RecordManager::new(&state.db)
        .preflight_create_authorized(collection, data.clone(), &context)
        .await?;
    let mut created_keys = Vec::new();
    for upload in &uploads {
        let key = storage_key(collection, &id, &upload.field, &upload.reference);
        created_keys.push(key.clone());
        if let Err(error) = state.storage.put_file(&key, &upload.source).await {
            compensate_new_objects(state, &created_keys).await;
            return Err(ApiFailure(error));
        }
    }
    match RecordManager::new(&state.db)
        .create_authorized_with_id(collection, &id, data, &context)
        .await
    {
        Ok(record) => Ok(record),
        Err(error) => {
            compensate_new_objects(state, &created_keys).await;
            Err(ApiFailure(error))
        }
    }
}

async fn update_multipart(
    req: &mut Request,
    state: &ApiState,
    collection: &str,
    id: &str,
    identity: &herta_auth::AuthIdentity,
) -> Result<Value, ApiFailure> {
    let form = req
        .form_data_max_size(state.config.server.max_body_size)
        .await
        .map_err(parse_error)?;
    let mut data = parse_multipart_data(form)?;
    let manager = RecordManager::new(&state.db);
    let schema = SchemaManager::new(&state.db)
        .get_collection(collection)
        .await?;
    reject_file_references(&schema, &data)?;
    normalize_file_clears(&schema, &mut data);
    let uploads = collect_uploads(form, &schema, state.config.storage.max_file_size)?;
    apply_uploaded_references(&mut data, &schema.fields, &uploads)?;
    let context = rule_context(identity, data.clone());
    manager
        .preflight_update_authorized(collection, id, data.clone(), &context)
        .await?;
    let old_record = manager.get(collection, id, None).await?;
    let replaced_fields: HashSet<String> = data
        .as_object()
        .into_iter()
        .flat_map(|object| object.keys())
        .filter(|name| {
            schema
                .fields
                .iter()
                .any(|field| field.name == **name && field.field_type == FieldType::File)
        })
        .cloned()
        .collect();
    let mut created_keys = Vec::new();
    for upload in &uploads {
        let key = storage_key(collection, id, &upload.field, &upload.reference);
        created_keys.push(key.clone());
        if let Err(error) = state.storage.put_file(&key, &upload.source).await {
            compensate_new_objects(state, &created_keys).await;
            return Err(ApiFailure(error));
        }
    }
    match manager
        .update_authorized(collection, id, data, &context)
        .await
    {
        Ok(record) => {
            for field in &replaced_fields {
                if let Some(value) = old_record.get(field) {
                    for old_name in file_names(value) {
                        let old_key = storage_key(collection, id, field, old_name);
                        if let Err(error) = state.storage.delete(&old_key).await {
                            tracing::warn!(collection, id, field, error = %error, "failed to remove replaced file");
                        }
                    }
                }
            }
            Ok(record)
        }
        Err(error) => {
            compensate_new_objects(state, &created_keys).await;
            Err(ApiFailure(error))
        }
    }
}

fn parse_multipart_data(form: &salvo::http::form::FormData) -> Result<Value, ApiFailure> {
    if form.fields.keys().any(|key| key != "data") {
        return Err(ApiFailure(herta_core::HbError::validation(
            "multipart only permits a data field besides file parts",
        )));
    }
    let values = form.fields.get_vec("data");
    if values.is_some_and(|values| values.len() > 1) {
        return Err(ApiFailure(herta_core::HbError::validation(
            "multipart data field may appear only once",
        )));
    }
    let data = values
        .and_then(|values| values.first())
        .map(|value| serde_json::from_str(value).map_err(parse_error))
        .transpose()?
        .unwrap_or_else(|| json!({}));
    if !data.is_object() {
        return Err(ApiFailure(herta_core::HbError::validation(
            "multipart data field must contain a JSON object",
        )));
    }
    Ok(data)
}

fn collect_uploads(
    form: &salvo::http::form::FormData,
    schema: &herta_db::CollectionDef,
    max_file_size: usize,
) -> Result<Vec<UploadPart>, ApiFailure> {
    let fields: HashMap<&str, &FieldDef> = schema
        .fields
        .iter()
        .map(|field| (field.name.as_str(), field))
        .collect();
    let mut uploads = Vec::new();
    for (name, parts) in form.files.iter_all() {
        let Some(field) = fields.get(name.as_str()) else {
            return Err(ApiFailure(herta_core::HbError::NotFound));
        };
        if field.field_type != FieldType::File {
            return Err(ApiFailure(herta_core::HbError::UnsupportedMediaType(
                format!("field '{name}' is not a file field"),
            )));
        }
        let max_select = herta_db::validation::file_max_select(field);
        if parts.len() > max_select {
            return Err(ApiFailure(herta_core::HbError::validation(format!(
                "file field '{name}' allows at most {max_select} file(s)",
            ))));
        }
        for part in parts {
            validate_upload_part(field, part, max_file_size)?;
            let extension = safe_extension(part.name().unwrap_or(""));
            let reference = extension.map_or_else(
                || Uuid::now_v7().to_string(),
                |extension| format!("{}.{}", Uuid::now_v7(), extension),
            );
            uploads.push(UploadPart {
                field: name.clone(),
                source: part.path().clone(),
                reference,
            });
        }
    }
    Ok(uploads)
}

fn validate_upload_part(
    field: &FieldDef,
    part: &salvo::http::form::FilePart,
    max_file_size: usize,
) -> Result<(), ApiFailure> {
    let max_size = field
        .options
        .as_ref()
        .and_then(|options| options.get("maxSize"))
        .and_then(Value::as_u64)
        .map_or(max_file_size as u64, |value| {
            value.min(max_file_size as u64)
        });
    if part.size() > max_size {
        return Err(ApiFailure(herta_core::HbError::PayloadTooLarge));
    }
    let extension = safe_extension(part.name().unwrap_or(""));
    if let Some(extensions) = field
        .options
        .as_ref()
        .and_then(|options| options.get("extensions"))
        .and_then(Value::as_array)
        && extension.as_deref().is_none_or(|extension| {
            !extensions.iter().any(|value| {
                value
                    .as_str()
                    .is_some_and(|allowed| allowed.eq_ignore_ascii_case(extension))
            })
        })
    {
        return Err(ApiFailure(herta_core::HbError::UnsupportedMediaType(
            format!("file extension is not allowed for field '{}'", field.name),
        )));
    }
    if let Some(mime_types) = field
        .options
        .as_ref()
        .and_then(|options| options.get("mimeTypes"))
        .and_then(Value::as_array)
    {
        let allowed = |candidate: &mime::Mime| {
            mime_types.iter().any(|allowed| {
                allowed
                    .as_str()
                    .and_then(|allowed| allowed.parse::<mime::Mime>().ok())
                    .is_some_and(|allowed| mime_matches(&allowed, candidate))
            })
        };
        let declared_allowed = part.content_type().as_ref().is_some_and(&allowed);
        let inferred_allowed = extension
            .as_deref()
            .and_then(|extension| mime_infer::from_ext(extension).first())
            .is_none_or(|inferred| allowed(&inferred));
        if !declared_allowed || !inferred_allowed {
            return Err(ApiFailure(herta_core::HbError::UnsupportedMediaType(
                format!("MIME type is not allowed for field '{}'", field.name),
            )));
        }
    }
    Ok(())
}

fn mime_matches(allowed: &mime::Mime, candidate: &mime::Mime) -> bool {
    allowed.type_() == candidate.type_()
        && (allowed.subtype() == mime::STAR || allowed.subtype() == candidate.subtype())
}

fn safe_extension(name: &str) -> Option<String> {
    let extension = std::path::Path::new(name)
        .extension()?
        .to_str()?
        .to_ascii_lowercase();
    (!extension.is_empty()
        && extension.len() <= 32
        && extension
            .chars()
            .all(|character| character.is_ascii_alphanumeric()))
    .then_some(extension)
}

fn apply_uploaded_references(
    data: &mut Value,
    fields: &[FieldDef],
    uploads: &[UploadPart],
) -> Result<(), ApiFailure> {
    let object = data.as_object_mut().ok_or_else(|| {
        ApiFailure(herta_core::HbError::validation(
            "record data must be an object",
        ))
    })?;
    let mut grouped: HashMap<&str, Vec<&str>> = HashMap::new();
    for upload in uploads {
        grouped
            .entry(upload.field.as_str())
            .or_default()
            .push(upload.reference.as_str());
    }
    for (field, references) in grouped {
        let definition = fields
            .iter()
            .find(|candidate| candidate.name == field)
            .expect("uploads were schema checked");
        if file_is_many(definition.options.as_ref()) {
            object.insert(
                field.into(),
                Value::Array(
                    references
                        .into_iter()
                        .map(|value| Value::String(value.into()))
                        .collect(),
                ),
            );
        } else {
            object.insert(field.into(), Value::String(references[0].into()));
        }
    }
    Ok(())
}

async fn prepare_json_body(
    db: &herta_db::DbClient,
    collection: &str,
    body: &mut Value,
) -> Result<(), ApiFailure> {
    let schema = SchemaManager::new(db).get_collection(collection).await?;
    reject_file_references(&schema, body)?;
    normalize_file_clears(&schema, body);
    Ok(())
}

fn reject_file_references(
    schema: &herta_db::CollectionDef,
    body: &Value,
) -> Result<(), ApiFailure> {
    for field in schema
        .fields
        .iter()
        .filter(|field| field.field_type == FieldType::File)
    {
        if let Some(value) = body.get(&field.name) {
            let empty = value.is_null() || value.as_array().is_some_and(Vec::is_empty);
            if !empty {
                return Err(ApiFailure(herta_core::HbError::UnsupportedMediaType(
                    format!("file field '{}' must be uploaded as multipart", field.name),
                )));
            }
        }
    }
    Ok(())
}

fn normalize_file_clears(schema: &herta_db::CollectionDef, body: &mut Value) {
    let Some(object) = body.as_object_mut() else {
        return;
    };
    for field in schema
        .fields
        .iter()
        .filter(|field| field.field_type == FieldType::File)
    {
        let Some(value) = object.get_mut(&field.name) else {
            continue;
        };
        if value.is_null() || value.as_array().is_some_and(Vec::is_empty) {
            *value = if file_is_many(field.options.as_ref()) {
                Value::Array(Vec::new())
            } else {
                Value::Null
            };
        }
    }
}

fn file_names(value: &Value) -> Vec<&str> {
    value
        .as_str()
        .into_iter()
        .chain(
            value
                .as_array()
                .into_iter()
                .flat_map(|values| values.iter().filter_map(Value::as_str)),
        )
        .collect()
}

async fn compensate_new_objects(state: &ApiState, keys: &[String]) {
    for key in keys {
        if let Err(error) = state.storage.delete(key).await {
            tracing::warn!(key, error = %error, "failed to compensate uploaded object");
        }
    }
}

fn storage_key(collection: &str, record_id: &str, field: &str, filename: &str) -> String {
    format!("records/{collection}/{record_id}/{field}/{filename}")
}
