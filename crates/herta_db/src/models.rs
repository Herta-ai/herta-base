use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use surrealdb::types::RecordId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CollectionDef {
    pub name: String,
    #[serde(rename = "type")]
    pub collection_type: CollectionType,
    pub schema_mode: SchemaMode,
    #[serde(default)]
    pub fields: Vec<FieldDef>,
    #[serde(default)]
    pub indexes: Vec<IndexDef>,
    #[serde(default)]
    pub rules: CollectionRules,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct CollectionRules {
    pub list: ApiRule,
    pub view: ApiRule,
    pub create: ApiRule,
    pub update: ApiRule,
    pub delete: ApiRule,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ApiRule {
    #[default]
    AdminOnly,
    Boolean(bool),
    Expression(String),
}

impl Serialize for ApiRule {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::AdminOnly => serializer.serialize_none(),
            Self::Boolean(value) => serializer.serialize_bool(*value),
            Self::Expression(value) => serializer.serialize_str(value),
        }
    }
}

impl<'de> Deserialize<'de> for ApiRule {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Option::<Value>::deserialize(deserializer)?;
        match value {
            None | Some(Value::Null) => Ok(Self::AdminOnly),
            Some(Value::Bool(value)) => Ok(Self::Boolean(value)),
            Some(Value::String(value)) => Ok(Self::Expression(value)),
            Some(_) => Err(serde::de::Error::custom(
                "API rule must be null, a boolean, or a string expression",
            )),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RuleContext {
    pub admin: bool,
    pub auth: Value,
    pub auth_record: Option<RecordId>,
    pub request_body: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CollectionType {
    Base,
    Auth,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SchemaMode {
    #[serde(rename = "schema-less")]
    Schemaless,
    Strict,
    Mixed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FieldDef {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: FieldType,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub options: Option<Value>,
}

impl FieldDef {
    pub(crate) fn surreal_kind(&self) -> String {
        let base = self.field_type.surreal_base_kind(self.options.as_ref());

        // SurrealDB rejects option<any>; any already accepts NONE for an absent field.
        if self.required || self.field_type == FieldType::Json {
            base
        } else {
            format!("option<{base}>")
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    Text,
    Number,
    Bool,
    Datetime,
    Json,
    File,
    Relation,
    Select,
    Email,
    Url,
}

impl FieldType {
    fn surreal_base_kind(&self, options: Option<&Value>) -> String {
        match self {
            Self::Text | Self::Select | Self::Email | Self::Url => "string".into(),
            Self::File if file_is_many(options) => "array<string>".into(),
            Self::File => "string".into(),
            Self::Number => "number".into(),
            Self::Bool => "bool".into(),
            Self::Datetime => "datetime".into(),
            Self::Json => "any".into(),
            Self::Relation => {
                let collection = options
                    .and_then(|value| value.get("collection"))
                    .and_then(Value::as_str)
                    .unwrap_or("any");
                if relation_is_many(options) {
                    format!("array<record<{collection}>>")
                } else {
                    format!("record<{collection}>")
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexDef {
    pub name: String,
    pub fields: Vec<String>,
    #[serde(default)]
    pub unique: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateCollectionRequest {
    #[serde(default)]
    pub fields: Vec<FieldDef>,
    #[serde(default)]
    pub indexes: Vec<IndexDef>,
    pub rules: Option<CollectionRules>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct ListParams {
    pub page: Option<u64>,
    #[serde(rename = "perPage")]
    pub per_page: Option<u64>,
    pub sort: Option<String>,
    pub filter: Option<String>,
    pub expand: Option<String>,
}

impl ListParams {
    pub fn page(&self) -> u64 {
        self.page.unwrap_or(1)
    }

    pub fn per_page(&self) -> u64 {
        self.per_page.unwrap_or(30)
    }

    pub fn validate(&self) -> herta_core::HbResult<()> {
        if self.page() == 0 {
            return Err(herta_core::HbError::validation("page must be at least 1"));
        }
        if !(1..=500).contains(&self.per_page()) {
            return Err(herta_core::HbError::validation(
                "perPage must be between 1 and 500",
            ));
        }
        Ok(())
    }
}

pub fn relation_is_many(options: Option<&Value>) -> bool {
    options
        .and_then(|value| value.get("maxSelect"))
        .and_then(Value::as_u64)
        .is_none_or(|max| max != 1)
}

pub fn file_is_many(options: Option<&Value>) -> bool {
    options
        .and_then(|value| value.get("maxSelect"))
        .and_then(Value::as_u64)
        .unwrap_or(1)
        > 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(field_type: FieldType, required: bool) -> FieldDef {
        FieldDef {
            name: "value".into(),
            field_type,
            required,
            options: None,
        }
    }

    #[test]
    fn field_definition_renders_required_and_optional_surreal_kinds() {
        assert_eq!(field(FieldType::Text, true).surreal_kind(), "string");
        assert_eq!(
            field(FieldType::Text, false).surreal_kind(),
            "option<string>"
        );
        assert_eq!(field(FieldType::Json, true).surreal_kind(), "any");
        assert_eq!(field(FieldType::Json, false).surreal_kind(), "any");
    }
}
