//! Shared path language for registration, request dispatch and reserved routes.
use crate::{HbError, HbResult, JsErrorKind, JsvmConfig, extension::Registration};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};

pub fn normalize_path(path: &str) -> HbResult<String> {
    let invalid = || HbError::validation("invalid request path");
    if !path.starts_with('/') || path.contains(['\\', '?', '#']) {
        return Err(invalid());
    }
    let mut bytes = Vec::with_capacity(path.len());
    let mut input = path.bytes();
    while let Some(byte) = input.next() {
        let byte = if byte == b'%' {
            let high = input
                .next()
                .and_then(|b| (b as char).to_digit(16))
                .ok_or_else(invalid)?;
            let low = input
                .next()
                .and_then(|b| (b as char).to_digit(16))
                .ok_or_else(invalid)?;
            let decoded = (high * 16 + low) as u8;
            if matches!(decoded, b'/' | b'\\') {
                return Err(invalid());
            }
            decoded
        } else {
            byte
        };
        if byte.is_ascii_control() {
            return Err(invalid());
        }
        bytes.push(byte);
    }
    let decoded = String::from_utf8(bytes).map_err(|_| invalid())?;
    if decoded.contains("//") || decoded.split('/').any(|part| matches!(part, "." | "..")) {
        return Err(invalid());
    }
    Ok(if decoded == "/" {
        decoded
    } else {
        decoded.trim_end_matches('/').to_owned()
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum NativeMiddleware {
    #[serde(rename = "requireAuth")]
    RequireAuth,
    #[serde(rename = "requireAdmin")]
    RequireAdmin,
    #[serde(rename = "bodyLimit")]
    BodyLimit { bytes: usize },
    #[serde(rename = "rateLimit")]
    RateLimit {
        limit: usize,
        #[serde(rename = "windowMs")]
        window_ms: u64,
        #[serde(default)]
        key: RateKey,
    },
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RateKey {
    #[default]
    Ip,
    Auth,
}

#[derive(Debug, Clone)]
pub struct CustomRoute {
    pub id: usize,
    pub method: String,
    pub path: String,
    pub middleware: Vec<NativeMiddleware>,
}

impl CustomRoute {
    pub fn parse(registration: &Registration, config: &JsvmConfig) -> HbResult<Self> {
        let opts = &registration.options;
        let method = opts["method"]
            .as_str()
            .ok_or_else(|| HbError::validation("route method must be a string"))?;
        if !matches!(method, "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD") {
            return Err(HbError::validation("unsupported custom route method"));
        }
        let path = normalize_path(
            opts["path"]
                .as_str()
                .ok_or_else(|| HbError::validation("route path must be a string"))?,
        )?;
        let mut names = HashSet::new();
        for part in path.split('/').skip(1) {
            if part.contains(['{', '}'])
                && !(parameter(part)
                    && part[1..part.len() - 1]
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_')
                    && names.insert(part))
            {
                return Err(HbError::validation("invalid or duplicate path parameter"));
            }
        }
        if !config.route_prefixes.iter().any(|prefix| {
            let prefix = prefix.trim_end_matches('/');
            path == prefix
                || path
                    .strip_prefix(prefix)
                    .is_some_and(|rest| rest.starts_with('/'))
        }) {
            return Err(JsErrorKind::Denied.into());
        }
        for reserved in [
            "/_",
            "/api/collections",
            "/api/auth",
            "/api/admin",
            "/api/files",
            "/api/realtime",
            "/api/events",
            "/api/operations",
            "/api-doc",
            "/swagger-ui",
            "/webui",
            "/web",
        ] {
            if overlaps_prefix(&path, reserved) {
                return Err(JsErrorKind::RouteConflict.into());
            }
        }
        let middleware: Vec<NativeMiddleware> = serde_json::from_value(
            opts.get("middleware")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([])),
        )
        .map_err(|error| HbError::validation(error.to_string()))?;
        for item in &middleware {
            match item {
                NativeMiddleware::BodyLimit { bytes: 0 } => {
                    return Err(HbError::validation("body limit must be positive"));
                }
                NativeMiddleware::RateLimit {
                    limit, window_ms, ..
                } if *limit == 0
                    || *limit > 1_000_000
                    || *window_ms == 0
                    || *window_ms > 86_400_000 =>
                {
                    return Err(HbError::validation("invalid rate limit"));
                }
                _ => {}
            }
        }
        Ok(Self {
            id: registration.id,
            method: method.into(),
            path,
            middleware,
        })
    }

    pub fn match_path(&self, path: &str) -> Option<BTreeMap<String, String>> {
        let pattern: Vec<_> = self.path.split('/').collect();
        let parts: Vec<_> = path.split('/').collect();
        if pattern.len() != parts.len() {
            return None;
        }
        let mut params = BTreeMap::new();
        for (pattern, value) in pattern.into_iter().zip(parts) {
            if parameter(pattern) && !value.is_empty() {
                params.insert(pattern[1..pattern.len() - 1].into(), value.into());
            } else if pattern != value {
                return None;
            }
        }
        Some(params)
    }
    pub fn overlaps(&self, other: &Self) -> bool {
        let a: Vec<_> = self.path.split('/').collect();
        let b: Vec<_> = other.path.split('/').collect();
        self.method == other.method
            && a.len() == b.len()
            && a.into_iter().zip(b).all(|(a, b)| compatible(a, b))
    }
}
fn parameter(part: &str) -> bool {
    part.starts_with('{') && part.ends_with('}') && part.len() > 2
}
fn compatible(a: &str, b: &str) -> bool {
    a == b || parameter(a) || parameter(b)
}
fn overlaps_prefix(path: &str, prefix: &str) -> bool {
    let a: Vec<_> = path.split('/').collect();
    let b: Vec<_> = prefix.split('/').collect();
    a.len() >= b.len() && a.into_iter().zip(b).all(|(a, b)| compatible(a, b))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_decode_once_and_reject_ambiguous_separators() {
        assert_eq!(normalize_path("/api/%E4%BD%A0/").unwrap(), "/api/你");
        assert_eq!(normalize_path("/api/%252f").unwrap(), "/api/%2f");
        for path in [
            "/api/%2f",
            "/api/%5C",
            "/api/%2e%2e/x",
            "/api//x",
            "/api/%00",
            "/api/%ff",
            "/api/%2",
        ] {
            assert!(normalize_path(path).is_err(), "{path}");
        }
    }
    #[test]
    fn conflicts_compare_matching_sets() {
        let make = |path| {
            CustomRoute::parse(
                &Registration {
                    id: 0,
                    options: serde_json::json!({"method":"GET","path":path}),
                    ..Default::default()
                },
                &JsvmConfig::default(),
            )
        };
        assert!(make("/api/{kind}/x").is_err()); // intersects all reserved API prefixes
        assert!(
            make("/api/custom/{id}")
                .unwrap()
                .overlaps(&make("/api/custom/fixed").unwrap())
        );
        assert_eq!(
            make("/api/custom/{id}")
                .unwrap()
                .match_path("/api/custom/42")
                .unwrap()["id"],
            "42"
        );
    }
}
