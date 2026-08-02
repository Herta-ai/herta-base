use std::collections::HashSet;

use email_address::EmailAddress;
use herta_core::{HbError, HbResult};
use regex::Regex;
use serde_json::{Map, Value, json};

use crate::models::{
    CollectionDef, CollectionType, FieldDef, FieldType, IndexDef, SchemaMode,
    UpdateCollectionRequest, relation_is_many,
};

const SYSTEM_FIELDS: [&str; 4] = ["id", "created_at", "updated_at", "deleted_at"];

pub fn validate_identifier(kind: &str, value: &str) -> HbResult<()> {
    let valid = Regex::new(r"^[A-Za-z][A-Za-z0-9_]*$").expect("identifier regex is valid");
    if !valid.is_match(value) {
        return Err(HbError::validation(format!(
            "{kind} must start with a letter and contain only letters, digits, and underscores"
        )));
    }
    Ok(())
}

pub fn quote_identifier(value: &str) -> String {
    format!("`{value}`")
}

pub fn validate_collection(def: &CollectionDef) -> HbResult<()> {
    validate_identifier("collection name", &def.name)?;
    if def.name.starts_with('_') {
        return Err(HbError::validation(
            "collection name cannot start with an underscore",
        ));
    }
    if def.collection_type != CollectionType::Base {
        return Err(HbError::validation(
            "auth collections are not available until Phase 2",
        ));
    }

    let mut fields = HashSet::new();
    for field in &def.fields {
        validate_field(field)?;
        if !fields.insert(field.name.as_str()) {
            return Err(HbError::validation(format!(
                "duplicate field '{}'",
                field.name
            )));
        }
    }

    let mut indexes = HashSet::new();
    for index in &def.indexes {
        validate_index(index, &fields)?;
        if !indexes.insert(index.name.as_str()) {
            return Err(HbError::validation(format!(
                "duplicate index '{}'",
                index.name
            )));
        }
    }
    Ok(())
}

pub fn validate_patch(existing: &CollectionDef, patch: &UpdateCollectionRequest) -> HbResult<()> {
    let known_fields: HashSet<&str> = existing
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    let mut new_fields = HashSet::new();
    for field in &patch.fields {
        validate_field(field)?;
        if known_fields.contains(field.name.as_str()) || !new_fields.insert(field.name.as_str()) {
            return Err(HbError::Conflict(format!(
                "field '{}' already exists",
                field.name
            )));
        }
    }

    let all_fields: HashSet<&str> = known_fields
        .iter()
        .copied()
        .chain(new_fields.iter().copied())
        .collect();
    let known_indexes: HashSet<&str> = existing
        .indexes
        .iter()
        .map(|index| index.name.as_str())
        .collect();
    let mut new_indexes = HashSet::new();
    for index in &patch.indexes {
        validate_index(index, &all_fields)?;
        if known_indexes.contains(index.name.as_str()) || !new_indexes.insert(index.name.as_str()) {
            return Err(HbError::Conflict(format!(
                "index '{}' already exists",
                index.name
            )));
        }
    }

    if patch.fields.is_empty() && patch.indexes.is_empty() {
        return Err(HbError::validation(
            "at least one new field or index is required",
        ));
    }
    Ok(())
}

fn validate_field(field: &FieldDef) -> HbResult<()> {
    validate_identifier("field name", &field.name)?;
    if SYSTEM_FIELDS.contains(&field.name.as_str()) {
        return Err(HbError::validation(format!(
            "field '{}' is reserved",
            field.name
        )));
    }

    match field.field_type {
        FieldType::Relation => {
            let target = field
                .options
                .as_ref()
                .and_then(|value| value.get("collection"))
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    HbError::validation(format!(
                        "relation field '{}' requires options.collection",
                        field.name
                    ))
                })?;
            validate_identifier("relation collection", target)?;
            if let Some(max) = field
                .options
                .as_ref()
                .and_then(|value| value.get("maxSelect"))
                .and_then(Value::as_u64)
                && max == 0
            {
                return Err(HbError::validation("relation maxSelect must be positive"));
            }
        }
        FieldType::Select => {
            let values = field
                .options
                .as_ref()
                .and_then(|value| value.get("values"))
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    HbError::validation(format!(
                        "select field '{}' requires options.values",
                        field.name
                    ))
                })?;
            if values.is_empty() || values.iter().any(|value| !value.is_string()) {
                return Err(HbError::validation(
                    "select options.values must be a non-empty string array",
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_index(index: &IndexDef, fields: &HashSet<&str>) -> HbResult<()> {
    validate_identifier("index name", &index.name)?;
    if index.fields.is_empty() {
        return Err(HbError::validation("index fields cannot be empty"));
    }
    for field in &index.fields {
        if !fields.contains(field.as_str()) && !SYSTEM_FIELDS.contains(&field.as_str()) {
            return Err(HbError::validation(format!(
                "index '{}' references unknown field '{}'",
                index.name, field
            )));
        }
    }
    Ok(())
}

pub fn validate_record(def: &CollectionDef, value: &mut Value, is_create: bool) -> HbResult<()> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| HbError::validation("record body must be a JSON object"))?;

    for system in SYSTEM_FIELDS {
        if object.contains_key(system) {
            return Err(HbError::validation(format!(
                "system field '{system}' is read-only"
            )));
        }
    }

    if is_create {
        for field in &def.fields {
            if field.required && object.get(&field.name).is_none_or(Value::is_null) {
                return Err(field_error(&field.name, "is required"));
            }
        }
    }

    if def.schema_mode == SchemaMode::Strict {
        for key in object.keys() {
            if !def.fields.iter().any(|field| field.name == *key) {
                return Err(field_error(key, "is not defined in the collection schema"));
            }
        }
    }

    for field in &def.fields {
        if let Some(field_value) = object.get(&field.name) {
            validate_field_value(field, field_value)?;
        }
    }
    Ok(())
}

fn validate_field_value(field: &FieldDef, value: &Value) -> HbResult<()> {
    if value.is_null() && !field.required {
        return Ok(());
    }
    let valid = match field.field_type {
        FieldType::Text | FieldType::File => value.is_string(),
        FieldType::Number => value.is_number(),
        FieldType::Bool => value.is_boolean(),
        FieldType::Datetime => value.as_str().is_some_and(|value| value.contains('T')),
        FieldType::Json => value.is_object() || value.is_array(),
        FieldType::Email => value
            .as_str()
            .is_some_and(|value| value.parse::<EmailAddress>().is_ok()),
        FieldType::Url => value
            .as_str()
            .is_some_and(|value| url::Url::parse(value).is_ok()),
        FieldType::Select => value.as_str().is_some_and(|candidate| {
            field
                .options
                .as_ref()
                .and_then(|value| value.get("values"))
                .and_then(Value::as_array)
                .is_some_and(|values| values.iter().any(|value| value.as_str() == Some(candidate)))
        }),
        FieldType::Relation => {
            let target = field
                .options
                .as_ref()
                .and_then(|value| value.get("collection"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            if relation_is_many(field.options.as_ref()) {
                value.as_array().is_some_and(|values| {
                    values.iter().all(|value| valid_relation_id(value, target))
                })
            } else {
                valid_relation_id(value, target)
            }
        }
    };

    if valid {
        Ok(())
    } else {
        Err(field_error(&field.name, "has an invalid value"))
    }
}

fn valid_relation_id(value: &Value, target: &str) -> bool {
    value
        .as_str()
        .and_then(|value| value.split_once(':'))
        .is_some_and(|(table, id)| table == target && !id.is_empty())
}

fn field_error(field: &str, reason: &str) -> HbError {
    HbError::Validation {
        message: format!("field '{field}' {reason}"),
        details: Some(json!({ "field": field, "reason": reason })),
    }
}

pub fn relation_fields_to_record_ids(
    def: &CollectionDef,
    object: &Map<String, Value>,
) -> Vec<(String, String, bool)> {
    def.fields
        .iter()
        .filter(|field| field.field_type == FieldType::Relation)
        .filter_map(|field| {
            object.get(&field.name).map(|_| {
                (
                    field.name.clone(),
                    field
                        .options
                        .as_ref()
                        .and_then(|value| value.get("collection"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    relation_is_many(field.options.as_ref()),
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CollectionType;

    fn strict_collection() -> CollectionDef {
        CollectionDef {
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
        }
    }

    #[test]
    fn strict_records_reject_unknown_fields() {
        let mut value = json!({"title": "ok", "extra": true});
        assert!(validate_record(&strict_collection(), &mut value, true).is_err());
    }

    #[test]
    fn identifiers_reject_surrealql_fragments() {
        assert!(validate_identifier("field", "name; DELETE posts").is_err());
    }
}
