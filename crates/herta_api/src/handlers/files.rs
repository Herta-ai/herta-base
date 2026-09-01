use std::ops::Range;

use herta_auth::AuthIdentity;
use herta_core::{HbError, HbResult};
use herta_db::{FieldType, RecordManager, SchemaManager};
use salvo::{
    http::{Method, header},
    prelude::*,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    handlers::auth::{identity, rule_context},
    response::{ApiFailure, ApiResponse, parse_error},
    router::{ApiState, SharedApiState},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileTokenRequest {
    collection: String,
    record_id: String,
    field: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileTokenResponse {
    token: String,
    expires_in: u64,
}

#[handler]
pub async fn token(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    let identity = identity(req, state).await?;
    if matches!(identity, AuthIdentity::Anonymous) {
        return Err(ApiFailure(HbError::AuthRequired));
    }
    let body: FileTokenRequest = req
        .parse_json_with_max_size(state.config.server.max_body_size)
        .await
        .map_err(parse_error)?;
    let record_id = normalize_record_id(&body.collection, &body.record_id)?;
    let schema = SchemaManager::new(&state.db)
        .get_collection(&body.collection)
        .await?;
    let field = schema
        .fields
        .iter()
        .find(|field| field.name == body.field && field.field_type == FieldType::File)
        .ok_or(HbError::NotFound)?;
    let record = RecordManager::new(&state.db)
        .get_authorized(
            &body.collection,
            &record_id,
            None,
            &rule_context(&identity, Value::Null),
        )
        .await?;
    if record
        .get(&field.name)
        .is_none_or(|value| !record_contains_any_file(value))
    {
        return Err(ApiFailure(HbError::NotFound));
    }
    let issued =
        state
            .auth
            .issue_file_token(&identity, &body.collection, &record_id, &body.field)?;
    res.render(Json(ApiResponse::ok(FileTokenResponse {
        token: issued.token,
        expires_in: issued.expires_in,
    })));
    Ok(())
}

#[handler]
pub async fn download(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    let collection = path(req, "collection")?;
    let record_id = normalize_record_id(&collection, &path(req, "recordId")?)?;
    let field_name = path(req, "field")?;
    let filename = path(req, "filename")?;
    if !herta_db::validation::valid_file_reference(&Value::String(filename.clone())) {
        return Err(ApiFailure(HbError::NotFound));
    }

    let schema = SchemaManager::new(&state.db)
        .get_collection(&collection)
        .await?;
    let field = schema
        .fields
        .iter()
        .find(|field| field.name == field_name && field.field_type == FieldType::File)
        .ok_or(HbError::NotFound)?;

    let record = if req.headers().contains_key(header::AUTHORIZATION) {
        let identity = identity(req, state).await?;
        RecordManager::new(&state.db)
            .get_authorized(
                &collection,
                &record_id,
                None,
                &rule_context(&identity, Value::Null),
            )
            .await?
    } else if let Some(file_token) = req.query::<String>("token") {
        let scope = state.auth.verify_file_token(&file_token).await?;
        if scope.collection != collection
            || scope.record_id != record_id
            || scope.field != field_name
        {
            return Err(ApiFailure(HbError::Forbidden));
        }
        RecordManager::new(&state.db)
            .get(&collection, &record_id, None)
            .await?
    } else {
        RecordManager::new(&state.db)
            .get_authorized(
                &collection,
                &record_id,
                None,
                &rule_context(&AuthIdentity::Anonymous, Value::Null),
            )
            .await?
    };
    if !record_contains_file(&record, &field.name, &filename) {
        return Err(ApiFailure(HbError::NotFound));
    }

    let key = storage_key(&collection, &record_id, &field.name, &filename);
    let metadata = state.storage.head(&key).await?;
    let etag = metadata.e_tag.as_deref().map(quote_etag);
    if etag.as_deref().is_some_and(|etag| if_none_match(req, etag)) {
        res.status_code(StatusCode::NOT_MODIFIED);
        set_common_headers(res, &filename, metadata.size, etag.as_deref(), false)?;
        return Ok(());
    }

    let requested_range = match parse_range(req, metadata.size) {
        Ok(range) => range,
        Err(error) => {
            insert_header(
                res,
                header::CONTENT_RANGE,
                &format!("bytes */{}", metadata.size),
            )?;
            return Err(ApiFailure(error));
        }
    };
    let partial = requested_range.is_some();
    let content_range = requested_range
        .as_ref()
        .map(|range| format!("bytes {}-{}/{}", range.start, range.end - 1, metadata.size));
    let response_range = requested_range.clone().unwrap_or(0..metadata.size);
    let content_length = response_range.end.saturating_sub(response_range.start);
    set_common_headers(res, &filename, content_length, etag.as_deref(), partial)?;
    if let Some(content_range) = content_range {
        insert_header(res, header::CONTENT_RANGE, &content_range)?;
        res.status_code(StatusCode::PARTIAL_CONTENT);
    }
    if req.method() != Method::HEAD {
        let object = state.storage.get(&key, requested_range).await?;
        res.stream(object.stream);
    }
    Ok(())
}

fn set_common_headers(
    res: &mut Response,
    filename: &str,
    content_length: u64,
    etag: Option<&str>,
    _partial: bool,
) -> HbResult<()> {
    let mime = mime_infer::from_path(filename).first_or_octet_stream();
    let disposition = if active_content(&mime) {
        "attachment"
    } else {
        "inline"
    };
    insert_header(res, header::CONTENT_TYPE, mime.as_ref())?;
    insert_header(res, header::CONTENT_LENGTH, &content_length.to_string())?;
    insert_header(res, header::ACCEPT_RANGES, "bytes")?;
    insert_header(
        res,
        header::CACHE_CONTROL,
        "private, max-age=0, must-revalidate",
    )?;
    insert_header(res, header::X_CONTENT_TYPE_OPTIONS, "nosniff")?;
    insert_header(
        res,
        header::CONTENT_DISPOSITION,
        &format!("{disposition}; filename=\"{filename}\""),
    )?;
    if let Some(etag) = etag {
        insert_header(res, header::ETAG, etag)?;
    }
    Ok(())
}

fn insert_header(res: &mut Response, name: header::HeaderName, value: &str) -> HbResult<()> {
    let value = header::HeaderValue::from_str(value).map_err(|_| HbError::Internal)?;
    res.headers_mut().insert(name, value);
    Ok(())
}

fn active_content(mime: &mime::Mime) -> bool {
    mime.type_() == mime::TEXT && mime.subtype() == mime::HTML
        || mime.type_() == mime::IMAGE && mime.subtype() == mime::SVG
        || mime.type_() == mime::APPLICATION
            && matches!(
                mime.subtype().as_str(),
                "javascript" | "x-javascript" | "wasm" | "xhtml+xml" | "xml"
            )
        || mime.type_() == mime::TEXT
            && matches!(mime.subtype().as_str(), "javascript" | "css" | "xml")
}

fn record_contains_file(record: &Value, field: &str, filename: &str) -> bool {
    record.get(field).is_some_and(|value| {
        value.as_str() == Some(filename)
            || value
                .as_array()
                .is_some_and(|values| values.iter().any(|value| value.as_str() == Some(filename)))
    })
}

fn record_contains_any_file(value: &Value) -> bool {
    value.as_str().is_some_and(|value| !value.is_empty())
        || value
            .as_array()
            .is_some_and(|values| values.iter().any(|value| value.as_str().is_some()))
}

fn parse_range(req: &Request, size: u64) -> HbResult<Option<Range<u64>>> {
    let Some(value) = req.headers().get(header::RANGE) else {
        return Ok(None);
    };
    let value = value.to_str().map_err(|_| HbError::RangeNotSatisfiable)?;
    let value = value
        .strip_prefix("bytes=")
        .filter(|value| !value.contains(','))
        .ok_or(HbError::RangeNotSatisfiable)?;
    let (start, end) = value.split_once('-').ok_or(HbError::RangeNotSatisfiable)?;
    if size == 0 {
        return Err(HbError::RangeNotSatisfiable);
    }
    let range = if start.is_empty() {
        let suffix = end
            .parse::<u64>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or(HbError::RangeNotSatisfiable)?;
        size.saturating_sub(suffix.min(size))..size
    } else {
        let start = start
            .parse::<u64>()
            .map_err(|_| HbError::RangeNotSatisfiable)?;
        if start >= size {
            return Err(HbError::RangeNotSatisfiable);
        }
        let end = if end.is_empty() {
            size
        } else {
            end.parse::<u64>()
                .map_err(|_| HbError::RangeNotSatisfiable)?
                .saturating_add(1)
                .min(size)
        };
        if end <= start {
            return Err(HbError::RangeNotSatisfiable);
        }
        start..end
    };
    Ok(Some(range))
}

fn if_none_match(req: &Request, etag: &str) -> bool {
    req.headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .map(str::trim)
                .any(|candidate| candidate == "*" || candidate == etag)
        })
}

fn quote_etag(etag: &str) -> String {
    if etag.starts_with('"') && etag.ends_with('"') {
        etag.into()
    } else {
        format!("\"{}\"", etag.replace('"', ""))
    }
}

fn storage_key(collection: &str, record_id: &str, field: &str, filename: &str) -> String {
    format!("records/{collection}/{record_id}/{field}/{filename}")
}

fn normalize_record_id(collection: &str, value: &str) -> HbResult<String> {
    match value.split_once(':') {
        Some((table, id)) if table == collection && valid_record_key(id) => Ok(id.into()),
        Some(_) => Err(HbError::NotFound),
        None if valid_record_key(value) => Ok(value.into()),
        None => Err(HbError::NotFound),
    }
}

fn valid_record_key(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn state(depot: &Depot) -> Result<&ApiState, ApiFailure> {
    depot
        .get_typed::<SharedApiState>()
        .map(AsRef::as_ref)
        .map_err(|_| ApiFailure(HbError::Internal))
}

fn path(req: &Request, name: &str) -> Result<String, ApiFailure> {
    req.param::<String>(name)
        .ok_or_else(|| parse_error(format!("missing path parameter '{name}'")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_parser_supports_bounded_open_and_suffix_ranges() {
        let request = |value: &str| {
            let mut req = Request::new();
            req.headers_mut()
                .insert(header::RANGE, header::HeaderValue::from_str(value).unwrap());
            req
        };
        assert_eq!(parse_range(&request("bytes=2-5"), 10).unwrap(), Some(2..6));
        assert_eq!(parse_range(&request("bytes=7-"), 10).unwrap(), Some(7..10));
        assert_eq!(parse_range(&request("bytes=-3"), 10).unwrap(), Some(7..10));
        assert!(parse_range(&request("bytes=20-30"), 10).is_err());
        assert!(parse_range(&request("bytes=0-1,3-4"), 10).is_err());
    }

    #[test]
    fn active_content_is_forced_to_download() {
        for value in [
            "text/html",
            "image/svg+xml",
            "application/javascript",
            "text/css",
            "application/wasm",
        ] {
            assert!(active_content(&value.parse().unwrap()), "{value}");
        }
        assert!(!active_content(&"image/png".parse().unwrap()));
        assert!(!active_content(&"text/plain".parse().unwrap()));
    }
}
