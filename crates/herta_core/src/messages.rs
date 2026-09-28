//! Application messages are separate from database change notifications.
use crate::{HbError, HbResult, JsErrorKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UserTarget {
    pub collection: String,
    pub id: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RoleTarget {
    pub collection: String,
    pub role: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Audience {
    Users(Vec<UserTarget>),
    Roles(Vec<RoleTarget>),
    Connections(Vec<String>),
}
impl Audience {
    pub fn validate(&self, limit: usize) -> HbResult<()> {
        let valid = |value: &str| {
            !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
        };
        let (length, values) = match self {
            Self::Users(users) => (
                users.len(),
                users
                    .iter()
                    .all(|user| valid(&user.collection) && valid(&user.id)),
            ),
            Self::Roles(roles) => (
                roles.len(),
                roles
                    .iter()
                    .all(|role| valid(&role.collection) && valid(&role.role)),
            ),
            Self::Connections(ids) => (ids.len(), ids.iter().all(|id| valid(id))),
        };
        if length == 0 || length > limit || !values {
            return Err(HbError::validation("invalid application message audience"));
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishRequest {
    pub topic: String,
    pub data: Value,
    pub audience: Option<Audience>,
}
impl PublishRequest {
    pub fn audience(&self, limit: usize) -> HbResult<&Audience> {
        let audience = self
            .audience
            .as_ref()
            .ok_or_else(|| HbError::from(JsErrorKind::Denied))?;
        audience.validate(limit)?;
        validate_topic(&self.topic)?;
        Ok(audience)
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Message {
    pub id: String,
    pub topic: String,
    pub data: Value,
    pub timestamp: String,
}
#[derive(Serialize, Default, Debug)]
pub struct PublishReceipt {
    pub queued: usize,
    pub dropped: usize,
}
pub fn validate_topic(topic: &str) -> HbResult<()> {
    if topic.is_empty()
        || topic.len() > 128
        || !topic.as_bytes()[0].is_ascii_alphanumeric()
        || !topic
            .bytes()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, b'.' | b'_' | b'/' | b'-'))
    {
        return Err(HbError::validation("invalid application message topic"));
    }
    Ok(())
}
