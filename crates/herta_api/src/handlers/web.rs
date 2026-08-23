use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::UNIX_EPOCH,
};

use chrono::Local;
use flate2::read::GzDecoder;
use herta_core::{HbConfig, HbError, HbResult};
use herta_db::{DbClient, WebProject, WebProjectManager};
use percent_encoding::percent_decode_str;
use salvo::{
    http::{Method, StatusCode, header},
    prelude::*,
};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;
use tokio::{
    io::{AsyncReadExt, AsyncSeekExt},
    sync::{OwnedRwLockReadGuard, RwLock},
};
use tokio_util::io::ReaderStream;
use uuid::Uuid;
use walkdir::WalkDir;

use crate::{
    handlers::auth::require_admin,
    response::{ApiFailure, ApiResponse, parse_error},
    router::ApiState,
};

const DEFAULT_CACHE_CONTROL: &str = "public, max-age=0, must-revalidate";
const VERSION_FORMAT: &str = "%Y-%m-%d-%H-%M-%S";
const FORM_OVERHEAD: usize = 1024 * 1024;

#[derive(Clone)]
pub struct WebHosting {
    web_root: PathBuf,
    backup_root: PathBuf,
    lock: Arc<RwLock<()>>,
}

#[derive(Debug, Default)]
struct DeployOptions {
    alias: Option<Option<String>>,
    spa_fallback: Option<bool>,
    cache_control: Option<String>,
    not_found: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PatchRequest {
    #[serde(default, deserialize_with = "double_option")]
    alias: Option<Option<String>>,
    #[serde(default)]
    spa_fallback: Option<bool>,
    #[serde(default)]
    cache_control: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    not_found: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
struct RollbackRequest {
    version: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectStatus {
    #[serde(flatten)]
    project: WebProject,
    deployed: bool,
}

fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

impl WebHosting {
    pub fn new(config: &HbConfig) -> HbResult<Self> {
        let data_root = PathBuf::from(&config.paths.data_dir);
        let web_root = data_root.join("web");
        let backup_root = data_root.join("web_backup");
        fs::create_dir_all(&web_root).map_err(storage_error)?;
        fs::create_dir_all(&backup_root).map_err(storage_error)?;
        Ok(Self {
            web_root,
            backup_root,
            lock: Arc::new(RwLock::new(())),
        })
    }

    async fn status(&self, project: WebProject) -> ProjectStatus {
        let deployed = self.web_root.join(&project.name).is_dir();
        ProjectStatus { project, deployed }
    }

    async fn extract_archive(&self, source: PathBuf) -> HbResult<(PathBuf, String)> {
        let stage = self.web_root.join(format!(".deploy-{}", Uuid::now_v7()));
        let task_stage = stage.clone();
        let result =
            tokio::task::spawn_blocking(move || extract_archive_sync(&source, &task_stage))
                .await
                .map_err(|_| HbError::Internal)?;
        match result {
            Ok(name) => Ok((stage, name)),
            Err(error) => {
                let _ = fs::remove_dir_all(&stage);
                Err(error)
            }
        }
    }

    async fn deploy(
        &self,
        db: &DbClient,
        archive: PathBuf,
        options: DeployOptions,
    ) -> HbResult<(WebProject, bool)> {
        let (stage, name) = self.extract_archive(archive).await?;
        let staged_project = stage.join(&name);
        let _guard = self.lock.write().await;
        let manager = WebProjectManager::new(db);
        let existing = match manager.get_optional(&name).await {
            Ok(existing) => existing,
            Err(error) => return fail_stage(&stage, error),
        };
        let mut project = existing.clone().unwrap_or_else(|| WebProject {
            name: name.clone(),
            alias: None,
            spa_fallback: true,
            cache_control: DEFAULT_CACHE_CONTROL.into(),
            not_found: None,
            deployed_at: String::new(),
        });
        apply_options(&mut project, options);
        if let Err(error) = validate_project_config(&mut project, &staged_project) {
            return fail_stage(&stage, error);
        }
        let projects = match manager.list().await {
            Ok(projects) => projects,
            Err(error) => return fail_stage(&stage, error),
        };
        if let Err(error) = validate_routes(&project, &projects) {
            return fail_stage(&stage, error);
        }
        project.deployed_at = Local::now().to_rfc3339();

        let target = self.web_root.join(&name);
        let old = self.web_root.join(format!(".old-{}", Uuid::now_v7()));
        let is_update = target.exists();
        let mut backup_path = None;
        if is_update {
            let version = Local::now().format(VERSION_FORMAT).to_string();
            let backup = self.backup_root.join(&name).join(version);
            if backup.exists() {
                let _ = fs::remove_dir_all(&stage);
                return Err(HbError::Conflict(
                    "a backup version already exists for this second".into(),
                ));
            }
            if let Err(error) = copy_tree(&target, &backup) {
                let _ = fs::remove_dir_all(&backup);
                let _ = fs::remove_dir_all(&stage);
                return Err(error);
            }
            backup_path = Some(backup);
            if let Err(error) = fs::rename(&target, &old) {
                if let Some(backup) = &backup_path {
                    let _ = fs::remove_dir_all(backup);
                }
                let _ = fs::remove_dir_all(&stage);
                return Err(storage_error(error));
            }
        }

        if let Err(error) = fs::rename(&staged_project, &target) {
            if is_update {
                let _ = fs::rename(&old, &target);
            }
            if let Some(backup) = &backup_path {
                let _ = fs::remove_dir_all(backup);
            }
            let _ = fs::remove_dir_all(&stage);
            return Err(storage_error(error));
        }

        if let Err(error) = manager.save(&project).await {
            let _ = fs::remove_dir_all(&target);
            if is_update {
                let _ = fs::rename(&old, &target);
            }
            if let Some(backup) = &backup_path {
                let _ = fs::remove_dir_all(backup);
            }
            let _ = fs::remove_dir_all(&stage);
            return Err(error);
        }
        if is_update {
            let _ = fs::remove_dir_all(&old);
        }
        let _ = fs::remove_dir_all(&stage);
        Ok((project, existing.is_none()))
    }

    async fn patch(&self, db: &DbClient, name: &str, patch: PatchRequest) -> HbResult<WebProject> {
        validate_project_name(name)?;
        let _guard = self.lock.write().await;
        let manager = WebProjectManager::new(db);
        let mut project = manager.get(name).await?;
        apply_options(
            &mut project,
            DeployOptions {
                alias: patch.alias,
                spa_fallback: patch.spa_fallback,
                cache_control: patch.cache_control,
                not_found: patch.not_found,
            },
        );
        validate_project_config(&mut project, &self.web_root.join(name))?;
        validate_routes(&project, &manager.list().await?)?;
        manager.save(&project).await?;
        Ok(project)
    }

    async fn delete(&self, db: &DbClient, name: &str) -> HbResult<()> {
        validate_project_name(name)?;
        let _guard = self.lock.write().await;
        let manager = WebProjectManager::new(db);
        manager.get(name).await?;
        let current = self.web_root.join(name);
        let trash = self.web_root.join(format!(".delete-{}", Uuid::now_v7()));
        let moved = if current.exists() {
            fs::rename(&current, &trash).map_err(storage_error)?;
            true
        } else {
            false
        };
        if let Err(error) = manager.delete(name).await {
            if moved {
                let _ = fs::rename(&trash, &current);
            }
            return Err(error);
        }
        if moved {
            fs::remove_dir_all(&trash).map_err(storage_error)?;
        }
        Ok(())
    }

    async fn versions(&self, db: &DbClient, name: &str) -> HbResult<Vec<String>> {
        validate_project_name(name)?;
        WebProjectManager::new(db).get(name).await?;
        let root = self.backup_root.join(name);
        if !root.exists() {
            return Ok(Vec::new());
        }
        let mut found_versions = fs::read_dir(root)
            .map_err(storage_error)?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.path().is_dir() && valid_version(&entry.file_name().to_string_lossy())
            })
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        found_versions.sort_unstable_by(|left, right| right.cmp(left));
        Ok(found_versions)
    }

    async fn rollback(&self, db: &DbClient, name: &str, version: &str) -> HbResult<WebProject> {
        validate_project_name(name)?;
        if !valid_version(version) {
            return Err(HbError::validation("invalid backup version"));
        }
        let _guard = self.lock.write().await;
        let manager = WebProjectManager::new(db);
        let project = manager.get(name).await?;
        let backup = self.backup_root.join(name).join(version);
        if !backup.is_dir() {
            return Err(HbError::NotFound);
        }
        let stage = self.web_root.join(format!(".rollback-{}", Uuid::now_v7()));
        if let Err(error) = copy_tree(&backup, &stage) {
            let _ = fs::remove_dir_all(&stage);
            return Err(error);
        }
        if let Err(error) = validate_deployed_tree(&stage) {
            let _ = fs::remove_dir_all(&stage);
            return Err(error);
        }
        let current = self.web_root.join(name);
        let old = self.web_root.join(format!(".old-{}", Uuid::now_v7()));
        if current.exists() {
            fs::rename(&current, &old).map_err(storage_error)?;
        }
        if let Err(error) = fs::rename(&stage, &current) {
            if old.exists() {
                let _ = fs::rename(&old, &current);
            }
            let _ = fs::remove_dir_all(&stage);
            return Err(storage_error(error));
        }
        if old.exists() {
            let _ = fs::remove_dir_all(&old);
        }
        Ok(project)
    }

    async fn serve_project(
        &self,
        req: &Request,
        res: &mut Response,
        db: &DbClient,
    ) -> HbResult<()> {
        let decoded_path = percent_decode_str(req.uri().path())
            .decode_utf8()
            .map_err(|_| HbError::NotFound)?;
        let guard = self.lock.clone().read_owned().await;
        let projects = WebProjectManager::new(db).list().await?;
        let Some((project, _root, relative)) = resolve_route(&decoded_path, &projects) else {
            res.status_code(StatusCode::NOT_FOUND);
            return Ok(());
        };
        if relative.is_none() {
            let mut location = req.uri().path().to_owned();
            location.push('/');
            if let Some(query) = req.uri().query() {
                location.push('?');
                location.push_str(query);
            }
            insert_header(res, header::LOCATION, &location)?;
            res.status_code(StatusCode::PERMANENT_REDIRECT);
            return Ok(());
        }
        let relative = relative.unwrap_or_default();
        let Some(relative_path) = request_path(&relative) else {
            res.status_code(StatusCode::NOT_FOUND);
            return Ok(());
        };
        let project_root = self.web_root.join(&project.name);
        let Some((file, status)) = select_file(&project_root, &relative_path, &project)? else {
            res.status_code(StatusCode::NOT_FOUND);
            return Ok(());
        };
        serve_file(req, res, &file, &project.cache_control, status, guard).await
    }
}

#[handler]
pub async fn list_projects(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    require_admin(req, state).await?;
    let projects = WebProjectManager::new(&state.db).list().await?;
    let mut statuses = Vec::with_capacity(projects.len());
    for project in projects {
        statuses.push(state.web.status(project).await);
    }
    res.render(Json(ApiResponse::ok(statuses)));
    Ok(())
}

#[handler]
pub async fn get_project(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    require_admin(req, state).await?;
    let project = WebProjectManager::new(&state.db)
        .get(&path(req, "project")?)
        .await?;
    res.render(Json(ApiResponse::ok(state.web.status(project).await)));
    Ok(())
}

#[handler]
pub async fn deploy_project(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    require_admin(req, state).await?;
    let limit = state.config.web.max_archive_size;
    let form = req
        .form_data_max_size(limit.saturating_add(FORM_OVERHEAD))
        .await
        .map_err(parse_error)?;
    if form.files.keys().any(|key| key != "archive") {
        return Err(ApiFailure(HbError::validation(
            "archive is the only allowed file field",
        )));
    }
    let archives = form
        .files
        .get_vec("archive")
        .ok_or_else(|| ApiFailure(HbError::validation("archive is required")))?;
    if archives.len() != 1 {
        return Err(ApiFailure(HbError::validation(
            "archive must appear exactly once",
        )));
    }
    let archive = &archives[0];
    if archive.size() > limit as u64 {
        return Err(ApiFailure(HbError::PayloadTooLarge));
    }
    let options = deploy_options(form)?;
    let (project, created) = state
        .web
        .deploy(&state.db, archive.path().clone(), options)
        .await?;
    if created {
        res.status_code(StatusCode::CREATED);
    }
    res.render(Json(ApiResponse::ok(state.web.status(project).await)));
    Ok(())
}

#[handler]
pub async fn patch_project(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    require_admin(req, state).await?;
    let name = path(req, "project")?;
    let patch: PatchRequest = req
        .parse_json_with_max_size(state.config.server.max_body_size)
        .await
        .map_err(parse_error)?;
    let project = state.web.patch(&state.db, &name, patch).await?;
    res.render(Json(ApiResponse::ok(state.web.status(project).await)));
    Ok(())
}

#[handler]
pub async fn delete_project(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    require_admin(req, state).await?;
    let name = path(req, "project")?;
    state.web.delete(&state.db, &name).await?;
    res.render(Json(ApiResponse::ok(
        json!({"name": name, "deleted": true}),
    )));
    Ok(())
}

#[handler]
pub async fn list_versions(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    require_admin(req, state).await?;
    let name = path(req, "project")?;
    let versions = state.web.versions(&state.db, &name).await?;
    res.render(Json(ApiResponse::ok(versions)));
    Ok(())
}

#[handler]
pub async fn rollback_project(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    require_admin(req, state).await?;
    let name = path(req, "project")?;
    let body: RollbackRequest = req
        .parse_json_with_max_size(state.config.server.max_body_size)
        .await
        .map_err(parse_error)?;
    let project = state.web.rollback(&state.db, &name, &body.version).await?;
    res.render(Json(ApiResponse::ok(state.web.status(project).await)));
    Ok(())
}

#[handler]
pub async fn serve_project(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    state.web.serve_project(req, res, &state.db).await?;
    Ok(())
}

fn state(depot: &Depot) -> Result<&ApiState, ApiFailure> {
    depot
        .get_typed::<ApiState>()
        .map_err(|_| ApiFailure(HbError::Internal))
}

fn path(req: &Request, name: &str) -> Result<String, ApiFailure> {
    req.param::<String>(name)
        .ok_or_else(|| parse_error(format!("missing path parameter '{name}'")))
}

fn deploy_options(form: &salvo::http::form::FormData) -> Result<DeployOptions, ApiFailure> {
    const ALLOWED: [&str; 4] = ["alias", "spaFallback", "cacheControl", "notFound"];
    if form
        .fields
        .keys()
        .any(|key| !ALLOWED.contains(&key.as_str()))
    {
        return Err(ApiFailure(HbError::validation(
            "multipart contains an unknown field",
        )));
    }
    let alias = field(form, "alias")?.map(|value| nonempty(value));
    let spa_fallback = field(form, "spaFallback")?
        .map(|value| {
            value
                .parse::<bool>()
                .map_err(|_| ApiFailure(HbError::validation("spaFallback must be true or false")))
        })
        .transpose()?;
    let cache_control = field(form, "cacheControl")?;
    let not_found = field(form, "notFound")?.map(|value| nonempty(value));
    Ok(DeployOptions {
        alias,
        spa_fallback,
        cache_control,
        not_found,
    })
}

fn field(form: &salvo::http::form::FormData, name: &str) -> Result<Option<String>, ApiFailure> {
    let Some(values) = form.fields.get_vec(name) else {
        return Ok(None);
    };
    if values.len() != 1 {
        return Err(ApiFailure(HbError::validation(format!(
            "{name} must appear at most once"
        ))));
    }
    Ok(values.first().cloned())
}

fn nonempty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn apply_options(project: &mut WebProject, options: DeployOptions) {
    if let Some(alias) = options.alias {
        project.alias = alias;
    }
    if let Some(spa_fallback) = options.spa_fallback {
        project.spa_fallback = spa_fallback;
    }
    if let Some(cache_control) = options.cache_control {
        project.cache_control = cache_control;
    }
    if let Some(not_found) = options.not_found {
        project.not_found = not_found;
    }
}

fn validate_project_config(project: &mut WebProject, root: &Path) -> HbResult<()> {
    validate_project_name(&project.name)?;
    if let Some(alias) = project.alias.as_mut() {
        *alias = validate_alias(alias)?;
    }
    header::HeaderValue::from_str(&project.cache_control)
        .map_err(|_| HbError::validation("cacheControl is not a valid HTTP header value"))?;
    if project.cache_control.trim().is_empty() {
        return Err(HbError::validation("cacheControl cannot be empty"));
    }
    if let Some(not_found) = project.not_found.as_mut() {
        let path = safe_relative(not_found)?;
        if !root.join(&path).is_file() {
            return Err(HbError::validation(
                "notFound must reference a file in the project",
            ));
        }
        *not_found = path_to_slashes(&path);
    }
    Ok(())
}

fn validate_routes(project: &WebProject, projects: &[WebProject]) -> HbResult<()> {
    let own_default = format!("/web/{}", project.name);
    let candidate_roots = [Some(own_default.as_str()), project.alias.as_deref()];
    if project
        .alias
        .as_deref()
        .is_some_and(|alias| routes_overlap(alias, &own_default))
    {
        return Err(HbError::Conflict(
            "alias conflicts with the project's default route".into(),
        ));
    }
    for other in projects.iter().filter(|other| other.name != project.name) {
        let other_default = format!("/web/{}", other.name);
        for candidate in candidate_roots.into_iter().flatten() {
            if routes_overlap(candidate, &other_default)
                || other
                    .alias
                    .as_deref()
                    .is_some_and(|alias| routes_overlap(candidate, alias))
            {
                return Err(HbError::Conflict(format!(
                    "route '{candidate}' is already in use"
                )));
            }
        }
    }
    Ok(())
}

fn routes_overlap(left: &str, right: &str) -> bool {
    left == right
        || left
            .strip_prefix(right)
            .is_some_and(|tail| tail.starts_with('/'))
        || right
            .strip_prefix(left)
            .is_some_and(|tail| tail.starts_with('/'))
}

fn validate_project_name(name: &str) -> HbResult<()> {
    if name.is_empty()
        || name.chars().count() > 64
        || matches!(name, "." | "..")
        || name.contains(['/', '\\', '?', '#', ':', '\0'])
        || Path::new(name).is_absolute()
    {
        return Err(HbError::validation("invalid web project name"));
    }
    Ok(())
}

fn validate_alias(value: &str) -> HbResult<String> {
    if !value.starts_with("/web/")
        || value == "/web/"
        || value.ends_with('/')
        || value.contains(['\\', '?', '#', '\0', '%'])
        || value
            .split('/')
            .any(|segment| matches!(segment, "." | ".."))
        || value.contains("//")
    {
        return Err(HbError::validation(
            "alias must be a canonical absolute path under /web/",
        ));
    }
    Ok(value.to_owned())
}

fn safe_relative(value: &str) -> HbResult<PathBuf> {
    if value.is_empty() || value.contains(['\\', '\0']) {
        return Err(HbError::validation("invalid relative project path"));
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(HbError::validation("invalid relative project path"));
    }
    Ok(path.to_path_buf())
}

fn valid_version(value: &str) -> bool {
    value.len() == 19 && chrono::NaiveDateTime::parse_from_str(value, VERSION_FORMAT).is_ok()
}

fn extract_archive_sync(source: &Path, stage: &Path) -> HbResult<String> {
    fs::create_dir(stage).map_err(storage_error)?;
    let mut signature = [0_u8; 6];
    let mut file = File::open(source).map_err(storage_error)?;
    let count = file.read(&mut signature).map_err(storage_error)?;
    drop(file);
    if count >= 4
        && matches!(
            &signature[..4],
            b"PK\x03\x04" | b"PK\x05\x06" | b"PK\x07\x08"
        )
    {
        extract_zip(source, stage)?;
    } else if count >= 2 && signature[..2] == [0x1f, 0x8b] {
        extract_tar_gz(source, stage)?;
    } else if count == 6 && signature == [0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c] {
        extract_7z(source, stage)?;
    } else {
        return Err(HbError::UnsupportedMediaType(
            "archive must be ZIP, tar.gz, or 7z".into(),
        ));
    }
    validate_archive_root(stage)
}

fn extract_zip(source: &Path, stage: &Path) -> HbResult<()> {
    let file = File::open(source).map_err(storage_error)?;
    let mut archive = zip::ZipArchive::new(file).map_err(archive_error)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(archive_error)?;
        let raw = entry.name().to_owned();
        validate_archive_path(&raw)?;
        if entry.is_symlink() || special_zip_entry(entry.unix_mode(), entry.is_dir()) {
            return Err(HbError::validation(
                "archive links and special files are not allowed",
            ));
        }
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| HbError::validation("archive contains an unsafe path"))?;
        let target = stage.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(storage_error)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(storage_error)?;
            }
            let mut output = File::create(target).map_err(storage_error)?;
            io::copy(&mut entry, &mut output).map_err(storage_error)?;
        }
    }
    Ok(())
}

fn special_zip_entry(mode: Option<u32>, directory: bool) -> bool {
    let Some(kind) = mode.map(|mode| mode & 0o170000).filter(|kind| *kind != 0) else {
        return false;
    };
    kind != if directory { 0o040000 } else { 0o100000 }
}

fn extract_tar_gz(source: &Path, stage: &Path) -> HbResult<()> {
    let file = File::open(source).map_err(storage_error)?;
    let mut archive = tar::Archive::new(GzDecoder::new(file));
    for entry in archive.entries().map_err(archive_error)? {
        let mut entry = entry.map_err(archive_error)?;
        let raw = entry.path_bytes();
        let raw = std::str::from_utf8(&raw)
            .map_err(|_| HbError::validation("archive paths must be UTF-8"))?;
        validate_archive_path(raw)?;
        let kind = entry.header().entry_type();
        if !(kind.is_file() || kind.is_dir()) {
            return Err(HbError::validation(
                "archive links and special files are not allowed",
            ));
        }
        if !entry.unpack_in(stage).map_err(archive_error)? {
            return Err(HbError::validation("archive contains an unsafe path"));
        }
    }
    Ok(())
}

fn extract_7z(source: &Path, stage: &Path) -> HbResult<()> {
    sevenz_rust::decompress_file_with_extract_fn(source, stage, |entry, reader, destination| {
        validate_archive_path(entry.name()).map_err(hb_to_sevenz)?;
        let attributes = entry.windows_attributes();
        let unix_kind = (attributes >> 16) & 0o170000;
        if entry.is_anti_item()
            || attributes & 0x400 != 0
            || (unix_kind != 0 && unix_kind != 0o040000 && unix_kind != 0o100000)
        {
            return Err(hb_to_sevenz(HbError::validation(
                "archive links and special files are not allowed",
            )));
        }
        if entry.is_directory() {
            fs::create_dir_all(destination).map_err(sevenz_rust::Error::io)?;
        } else {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(sevenz_rust::Error::io)?;
            }
            let mut output = File::create(destination).map_err(sevenz_rust::Error::io)?;
            io::copy(reader, &mut output).map_err(sevenz_rust::Error::io)?;
        }
        Ok(true)
    })
    .map_err(archive_error)
}

fn hb_to_sevenz(error: HbError) -> sevenz_rust::Error {
    sevenz_rust::Error::io(io::Error::new(
        io::ErrorKind::InvalidData,
        error.to_string(),
    ))
}

fn validate_archive_path(raw: &str) -> HbResult<()> {
    if raw.is_empty()
        || raw.starts_with(['/', '\\'])
        || raw.contains(['\\', '\0'])
        || raw.as_bytes().get(1) == Some(&b':')
        || raw.split('/').any(|part| matches!(part, "." | ".."))
    {
        return Err(HbError::validation("archive contains an unsafe path"));
    }
    Ok(())
}

fn validate_archive_root(stage: &Path) -> HbResult<String> {
    let entries = fs::read_dir(stage)
        .map_err(storage_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage_error)?;
    if entries.len() != 1 || !entries[0].file_type().map_err(storage_error)?.is_dir() {
        return Err(HbError::validation(
            "archive must contain exactly one root directory",
        ));
    }
    let name = entries[0]
        .file_name()
        .into_string()
        .map_err(|_| HbError::validation("project name must be UTF-8"))?;
    validate_project_name(&name)?;
    validate_deployed_tree(&entries[0].path())?;
    if !WalkDir::new(entries[0].path())
        .min_depth(1)
        .into_iter()
        .filter_map(Result::ok)
        .any(|entry| entry.file_type().is_file())
    {
        return Err(HbError::validation(
            "archive root directory cannot be empty",
        ));
    }
    Ok(name)
}

fn validate_deployed_tree(root: &Path) -> HbResult<()> {
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(storage_error)?;
        let kind = entry.file_type();
        if !(kind.is_dir() || kind.is_file()) {
            return Err(HbError::validation(
                "project contains a link or special file",
            ));
        }
    }
    Ok(())
}

fn copy_tree(source: &Path, target: &Path) -> HbResult<()> {
    validate_deployed_tree(source)?;
    if target.exists() {
        return Err(HbError::Conflict(format!(
            "path '{}' already exists",
            target.display()
        )));
    }
    fs::create_dir_all(target).map_err(storage_error)?;
    for entry in WalkDir::new(source).min_depth(1).follow_links(false) {
        let entry = entry.map_err(storage_error)?;
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|_| HbError::Internal)?;
        let destination = target.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&destination).map_err(storage_error)?;
        } else {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(storage_error)?;
            }
            fs::copy(entry.path(), destination).map_err(storage_error)?;
        }
    }
    Ok(())
}

fn resolve_route<'a>(
    path: &str,
    projects: &'a [WebProject],
) -> Option<(&'a WebProject, String, Option<String>)> {
    let mut matches = projects
        .iter()
        .flat_map(|project| {
            let default = format!("/web/{}", project.name);
            [
                (project, default),
                (project, project.alias.clone().unwrap_or_default()),
            ]
        })
        .filter(|(_, root)| {
            !root.is_empty()
                && (path == root
                    || path
                        .strip_prefix(root)
                        .is_some_and(|tail| tail.starts_with('/')))
        })
        .collect::<Vec<_>>();
    matches.sort_unstable_by_key(|(_, root)| std::cmp::Reverse(root.len()));
    let (project, root) = matches.into_iter().next()?;
    let relative = if path == root {
        None
    } else {
        Some(path[root.len() + 1..].to_owned())
    };
    Some((project, root, relative))
}

fn request_path(value: &str) -> Option<PathBuf> {
    if value.is_empty() {
        return Some(PathBuf::new());
    }
    safe_relative(value).ok()
}

fn select_file(
    root: &Path,
    relative: &Path,
    project: &WebProject,
) -> HbResult<Option<(PathBuf, StatusCode)>> {
    let canonical_root = match fs::canonicalize(root) {
        Ok(root) => root,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(storage_error(error)),
    };
    let requested = root.join(relative);
    if let Some(file) = confined_file(&canonical_root, &requested)? {
        return Ok(Some((file, StatusCode::OK)));
    }
    if requested.is_dir()
        && let Some(file) = confined_file(&canonical_root, &requested.join("index.html"))?
    {
        return Ok(Some((file, StatusCode::OK)));
    }
    if project.spa_fallback
        && let Some(file) = confined_file(&canonical_root, &root.join("index.html"))?
    {
        return Ok(Some((file, StatusCode::OK)));
    }
    if let Some(not_found) = &project.not_found
        && let Some(file) = confined_file(&canonical_root, &root.join(not_found))?
    {
        return Ok(Some((file, StatusCode::NOT_FOUND)));
    }
    Ok(None)
}

fn confined_file(canonical_root: &Path, candidate: &Path) -> HbResult<Option<PathBuf>> {
    if !candidate.is_file() {
        return Ok(None);
    }
    let canonical = fs::canonicalize(candidate).map_err(storage_error)?;
    Ok(canonical.starts_with(canonical_root).then_some(canonical))
}

async fn serve_file(
    req: &Request,
    res: &mut Response,
    path: &Path,
    cache: &str,
    status: StatusCode,
    guard: OwnedRwLockReadGuard<()>,
) -> HbResult<()> {
    let mut file = tokio::fs::File::open(path).await.map_err(storage_error)?;
    let metadata = file.metadata().await.map_err(storage_error)?;
    let size = metadata.len();
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos());
    let etag = format!("\"{size:x}-{modified:x}\"");
    let mime = mime_infer::from_path(path).first_or_octet_stream();
    insert_header(res, header::CONTENT_TYPE, mime.as_ref())?;
    insert_header(res, header::CACHE_CONTROL, cache)?;
    insert_header(res, header::ETAG, &etag)?;
    insert_header(res, header::ACCEPT_RANGES, "bytes")?;
    insert_header(res, header::X_CONTENT_TYPE_OPTIONS, "nosniff")?;
    if status == StatusCode::OK && if_none_match(req, &etag) {
        res.status_code(StatusCode::NOT_MODIFIED);
        return Ok(());
    }
    let range = if status == StatusCode::OK {
        match parse_range(req, size) {
            Ok(range) => range,
            Err(()) => {
                insert_header(res, header::CONTENT_RANGE, &format!("bytes */{size}"))?;
                res.status_code(StatusCode::RANGE_NOT_SATISFIABLE);
                return Ok(());
            }
        }
    } else {
        None
    };
    let (offset, length, response_status) = if let Some(range) = range {
        let length = range.end - range.start;
        insert_header(
            res,
            header::CONTENT_RANGE,
            &format!("bytes {}-{}/{size}", range.start, range.end - 1),
        )?;
        (range.start, length, StatusCode::PARTIAL_CONTENT)
    } else {
        (0, size, status)
    };
    insert_header(res, header::CONTENT_LENGTH, &length.to_string())?;
    res.status_code(response_status);
    if req.method() == Method::HEAD {
        return Ok(());
    }
    if offset != 0 {
        file.seek(std::io::SeekFrom::Start(offset))
            .await
            .map_err(storage_error)?;
    }
    let stream = ReaderStream::new(file.take(length));
    let guarded = futures_util::stream::unfold((stream, guard), |(mut stream, guard)| async move {
        use futures_util::StreamExt;
        stream.next().await.map(|item| (item, (stream, guard)))
    });
    res.stream(guarded);
    Ok(())
}

fn parse_range(req: &Request, size: u64) -> Result<Option<std::ops::Range<u64>>, ()> {
    let Some(value) = req.headers().get(header::RANGE) else {
        return Ok(None);
    };
    let value = value
        .to_str()
        .map_err(|_| ())?
        .strip_prefix("bytes=")
        .filter(|value| !value.contains(','))
        .ok_or(())?;
    let (start, end) = value.split_once('-').ok_or(())?;
    if size == 0 {
        return Err(());
    }
    let range = if start.is_empty() {
        let suffix = end
            .parse::<u64>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or(())?;
        size.saturating_sub(suffix.min(size))..size
    } else {
        let start = start.parse::<u64>().map_err(|_| ())?;
        if start >= size {
            return Err(());
        }
        let end = if end.is_empty() {
            size
        } else {
            end.parse::<u64>()
                .map_err(|_| ())?
                .saturating_add(1)
                .min(size)
        };
        if end <= start {
            return Err(());
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

fn insert_header(res: &mut Response, name: header::HeaderName, value: &str) -> HbResult<()> {
    let value = header::HeaderValue::from_str(value).map_err(|_| HbError::Internal)?;
    res.headers_mut().insert(name, value);
    Ok(())
}

fn path_to_slashes(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn archive_error(error: impl std::fmt::Display) -> HbError {
    HbError::validation(format!("invalid archive: {error}"))
}

fn storage_error(error: impl std::fmt::Display) -> HbError {
    HbError::Storage(error.to_string())
}

fn fail_stage<T>(stage: &Path, error: HbError) -> HbResult<T> {
    let _ = fs::remove_dir_all(stage);
    Err(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_names_and_archive_paths_reject_traversal() {
        for name in ["", ".", "..", "a/b", "a\\b", "a?b", "a#b"] {
            assert!(validate_project_name(name).is_err(), "{name}");
        }
        for path in [
            "../site/index.html",
            "/site/index.html",
            "C:/site/index.html",
            "site\\index.html",
        ] {
            assert!(validate_archive_path(path).is_err(), "{path}");
        }
        assert!(validate_project_name("customer-portal").is_ok());
        assert!(validate_archive_path("site/assets/app.js").is_ok());
    }

    #[test]
    fn aliases_are_canonical_and_cannot_overlap_routes() {
        assert_eq!(validate_alias("/web/docs").unwrap(), "/web/docs");
        for alias in [
            "/docs",
            "/web/",
            "/web/docs/",
            "/web/a//b",
            "/web/a/../b",
            "/web/a%2Fb",
        ] {
            assert!(validate_alias(alias).is_err(), "{alias}");
        }
        assert!(routes_overlap("/web/docs", "/web/docs/v2"));
        assert!(!routes_overlap("/web/docs", "/web/docs-v2"));
    }

    #[test]
    fn range_parser_supports_standard_single_ranges() {
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
    }
}
