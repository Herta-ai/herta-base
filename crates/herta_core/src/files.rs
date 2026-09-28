//! Public extension-file metadata. Physical storage locations never cross the bridge.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileItem {
    pub key: String,
    pub size: u64,
    pub content_type: String,
    pub version: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePage {
    pub items: Vec<FileItem>,
    pub next_cursor: Option<String>,
}

/// An administrator attests that the backend has finished the failed write and
/// will not subsequently create its immutable object. HEAD absence is insufficient.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileResolve {
    pub resolution: FileResolution,
    pub note: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileResolution {
    NotWritten,
}

/// A portable, literal logical key: it is never URL-decoded or a host path.
pub fn validate_key(key: &str, allow_empty: bool) -> crate::HbResult<()> {
    if (key.is_empty() && !allow_empty)
        || key.len() > 1024
        || key
            .chars()
            .any(|c| c.is_control() || "\\:%<>\"|?*".contains(c))
        || (!key.is_empty()
            && key.split('/').any(|part| {
                let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
                part.is_empty()
                    || part == "."
                    || part == ".."
                    || part.ends_with(['.', ' '])
                    || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                    || stem
                        .strip_prefix("COM")
                        .or_else(|| stem.strip_prefix("LPT"))
                        .is_some_and(|n| {
                            matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
                        })
            }))
    {
        return Err(crate::JsErrorKind::FileDenied.into());
    }
    Ok(())
}
