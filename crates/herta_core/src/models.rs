use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// Credential/security-state fields never exposed through record or Auth views.
pub const AUTH_SENSITIVE_FIELDS: &[&str] = &[
    "_hb_auth_issuance",
    "password",
    "passwordConfirm",
    "password_hash",
    "token_key",
    "failed_attempts",
    "locked_until",
    "accessToken",
    "refreshToken",
    "access_token",
    "refresh_token",
];
/// Owned by AuthService, including canonical response metadata. Profile writes
/// and Auth schema declarations cannot shadow these fields.
pub const AUTH_MANAGED_FIELDS: &[&str] = &[
    "_hb_auth_issuance",
    "id",
    "email",
    "password",
    "passwordConfirm",
    "password_hash",
    "token_key",
    "verified",
    "role",
    "failed_attempts",
    "locked_until",
    "accessToken",
    "refreshToken",
    "access_token",
    "refresh_token",
    "created_at",
    "updated_at",
    "deleted_at",
    "createdAt",
    "updatedAt",
    "admin",
    "collection",
];

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

    pub fn validate(&self) -> crate::HbResult<()> {
        if self.page() == 0 {
            return Err(crate::HbError::validation("page must be at least 1"));
        }
        if !(1..=500).contains(&self.per_page()) {
            return Err(crate::HbError::validation(
                "perPage must be between 1 and 500",
            ));
        }
        if (self.page() - 1)
            .checked_mul(self.per_page())
            .is_none_or(|offset| offset > i64::MAX as u64)
        {
            return Err(crate::HbError::validation("pagination offset is too large"));
        }
        Ok(())
    }
}

/// One query contract for HTTP pagination, extensions, and restricted SELECT.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct RecordQuery {
    pub filter: Option<String>,
    pub bindings: serde_json::Map<String, Value>,
    pub sort: Option<String>,
    pub limit: u64,
    pub offset: u64,
    pub fields: Option<Vec<String>>,
    pub expand: Option<String>,
}

impl Default for RecordQuery {
    fn default() -> Self {
        Self {
            filter: None,
            bindings: Default::default(),
            sort: None,
            limit: 30,
            offset: 0,
            fields: None,
            expand: None,
        }
    }
}

impl RecordQuery {
    pub fn from_list(params: &ListParams) -> crate::HbResult<Self> {
        params.validate()?;
        Ok(Self {
            filter: params.filter.clone(),
            sort: params.sort.clone(),
            limit: params.per_page(),
            offset: (params.page() - 1) * params.per_page(),
            expand: params.expand.clone(),
            ..Self::default()
        })
    }

    pub fn validate(&self) -> crate::HbResult<()> {
        if !(1..=500).contains(&self.limit) || self.offset > i64::MAX as u64 {
            return Err(crate::HbError::validation(
                "limit must be 1..500 and offset must fit a signed 64-bit integer",
            ));
        }
        if self.bindings.len() > 64
            || self
                .fields
                .as_ref()
                .is_some_and(|fields| fields.is_empty() || fields.len() > 128)
        {
            return Err(crate::HbError::validation(
                "too many query bindings or invalid projection size",
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
