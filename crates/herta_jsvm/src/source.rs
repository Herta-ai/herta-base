use herta_core::{
    HbResult, JsError, JsErrorKind, JsvmConfig, extension::Registration, sandbox_fs::SandboxDir,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Read, path::Path, sync::Arc};

#[derive(Debug, Clone)]
pub struct Source {
    pub path: String,
    pub text: String,
}

#[derive(Debug)]
pub struct Snapshot {
    pub hash: String,
    pub sources: Vec<Source>,
    pub registrations: Arc<Vec<Registration>>,
    pub environment: BTreeMap<String, String>,
}

impl Snapshot {
    pub fn read(path: &Path, config: &JsvmConfig) -> HbResult<Self> {
        let directory = SandboxDir::open(path, false).map_err(load_error)?;
        let mut sources = Vec::new();
        let mut bytes = 0;
        let mut entries = 0;
        discover(
            &directory,
            "",
            0,
            config,
            &mut sources,
            &mut bytes,
            &mut entries,
        )?;
        sources.sort_by(|a, b| a.path.cmp(&b.path));
        let mut hash = Sha256::new();
        for source in &sources {
            hash.update(source.path.as_bytes());
            hash.update([0]);
            hash.update(source.text.as_bytes());
            hash.update([0]);
        }
        let environment: BTreeMap<_, _> = config
            .env_allowlist
            .iter()
            .filter_map(|name| std::env::var(name).ok().map(|value| (name.clone(), value)))
            .collect();
        for (name, value) in &environment {
            hash.update(name);
            hash.update([0]);
            hash.update(value);
            hash.update([0]);
        }
        Ok(Self {
            hash: format!("{:x}", hash.finalize()),
            sources,
            registrations: Arc::new(Vec::new()),
            environment,
        })
    }
}

fn discover(
    directory: &SandboxDir,
    prefix: &str,
    depth: usize,
    config: &JsvmConfig,
    sources: &mut Vec<Source>,
    bytes: &mut usize,
    entries: &mut usize,
) -> HbResult<()> {
    if depth > 32 {
        return Err(load_error("script directory nesting exceeds 32"));
    }
    let maximum = config.max_script_files.saturating_mul(64);
    for (name, is_directory, is_file) in directory
        .entries(maximum.saturating_sub(*entries))
        .map_err(load_error)?
    {
        *entries += 1;
        if name.starts_with(['.', '_']) {
            continue;
        }
        let relative = format!("{prefix}{name}");
        if is_directory {
            // The descriptor used to enumerate a child is also the authority used to open it.
            let child = directory.child(&name, false).map_err(load_error)?;
            discover(
                &child,
                &format!("{relative}/"),
                depth + 1,
                config,
                sources,
                bytes,
                entries,
            )?;
        } else if is_file && name.ends_with(".js") {
            if sources.len() >= config.max_script_files {
                return Err(load_error("script file count limit exceeded"));
            }
            let mut text = String::new();
            directory
                .read(&name)
                .map_err(load_error)?
                .take(config.max_script_bytes as u64 + 1)
                .read_to_string(&mut text)
                .map_err(load_error)?;
            *bytes = bytes
                .checked_add(text.len())
                .ok_or_else(|| load_error("source size overflow"))?;
            if text.len() > config.max_script_bytes || *bytes > config.max_source_bytes {
                return Err(load_error("script source byte limit exceeded"));
            }
            sources.push(Source {
                path: relative,
                text,
            });
        }
    }
    Ok(())
}

pub(crate) fn load_error(error: impl std::fmt::Display) -> herta_core::HbError {
    JsError::diagnostic(JsErrorKind::Load, error.to_string()).into()
}
