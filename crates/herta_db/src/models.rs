//! Compatibility re-exports; database-specific conversions stay in this crate.
pub use herta_core::models::*;
use serde_json::Value;
use surrealdb::types::RecordId;

#[derive(Debug, Clone, Default)]
pub struct RuleContext {
    pub admin: bool,
    pub auth: Value,
    pub auth_record: Option<RecordId>,
    pub request_body: Value,
}

pub(crate) trait SurrealField {
    fn surreal_kind(&self) -> String;
}
impl SurrealField for FieldDef {
    fn surreal_kind(&self) -> String {
        let base = self.field_type.surreal_base_kind(self.options.as_ref());

        // SurrealDB rejects option<any>; any already accepts NONE for an absent field.
        if self.required || self.field_type == FieldType::Json {
            base
        } else {
            format!("option<{base}>")
        }
    }
}

trait SurrealFieldType {
    fn surreal_base_kind(&self, options: Option<&Value>) -> String;
}
impl SurrealFieldType for FieldType {
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
