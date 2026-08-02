use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    pub fn surreal_kind(&self, options: Option<&Value>) -> String {
        match self {
            Self::Text | Self::File | Self::Select | Self::Email | Self::Url => "string".into(),
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
