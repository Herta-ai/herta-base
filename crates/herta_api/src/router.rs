use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use herta_auth::AuthService;
use herta_core::HbConfig;
use herta_db::DbClient;
use herta_storage::{Storage, storage_from_config};
use salvo::{oapi::swagger_ui::SwaggerUi, prelude::*};

use crate::{
    docs::OpenApiCache,
    handlers::{auth, collections, docs, files, logs, realtime, records, web},
};

pub struct ApiState {
    pub db: DbClient,
    pub config: Arc<HbConfig>,
    pub docs: OpenApiCache,
    pub auth: AuthService,
    pub storage: Arc<dyn Storage>,
    pub realtime: RealtimeLimiter,
    pub web: web::WebHosting,
}

pub type SharedApiState = Arc<ApiState>;

impl ApiState {
    pub async fn new(db: DbClient, config: HbConfig) -> herta_core::HbResult<Self> {
        let storage = storage_from_config(&config)?;
        Self::new_with_storage(db, config, storage).await
    }

    pub async fn new_with_storage(
        db: DbClient,
        config: HbConfig,
        storage: Arc<dyn Storage>,
    ) -> herta_core::HbResult<Self> {
        let auth = AuthService::new(db.clone(), &config).await?;
        let realtime = RealtimeLimiter::new(
            config.realtime.max_connections,
            config.realtime.max_connections_per_ip,
        );
        let web = web::WebHosting::new(&config)?;
        let state = Self {
            db,
            config: Arc::new(config),
            docs: OpenApiCache::empty(),
            auth,
            storage,
            realtime,
            web,
        };
        state.docs.refresh(&state).await?;
        Ok(state)
    }
}

#[derive(Clone)]
pub struct RealtimeLimiter {
    inner: Arc<Mutex<ConnectionCounts>>,
    max_connections: usize,
    max_connections_per_ip: usize,
}

#[derive(Default)]
struct ConnectionCounts {
    total: usize,
    by_ip: HashMap<String, usize>,
}

impl RealtimeLimiter {
    fn new(max_connections: usize, max_connections_per_ip: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ConnectionCounts::default())),
            max_connections,
            max_connections_per_ip,
        }
    }

    pub fn try_acquire(&self, ip: String) -> herta_core::HbResult<RealtimePermit> {
        let mut counts = self
            .inner
            .lock()
            .map_err(|_| herta_core::HbError::Internal)?;
        let ip_count = counts.by_ip.get(&ip).copied().unwrap_or(0);
        if counts.total >= self.max_connections || ip_count >= self.max_connections_per_ip {
            return Err(herta_core::HbError::RateLimited);
        }
        counts.total += 1;
        counts.by_ip.insert(ip.clone(), ip_count + 1);
        Ok(RealtimePermit {
            limiter: self.clone(),
            ip,
        })
    }
}

pub struct RealtimePermit {
    limiter: RealtimeLimiter,
    ip: String,
}

impl Drop for RealtimePermit {
    fn drop(&mut self) {
        let Ok(mut counts) = self.limiter.inner.lock() else {
            return;
        };
        counts.total = counts.total.saturating_sub(1);
        if let Some(count) = counts.by_ip.get_mut(&self.ip) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                counts.by_ip.remove(&self.ip);
            }
        }
    }
}

pub fn build_router() -> Router {
    build_router_with_logger(None)
}

pub fn build_router_with_logger(
    request_logger: Option<crate::handlers::logging::RequestLogger>,
) -> Router {
    let records_router = Router::with_path("api/collections/{collection}/records")
        .get(records::list)
        .post(records::create)
        .push(
            Router::with_path("{id}")
                .get(records::get)
                .patch(records::update)
                .delete(records::delete),
        );
    let collections_router = Router::with_path("_/collections")
        .get(collections::list)
        .post(collections::create)
        .push(
            Router::with_path("{name}")
                .get(collections::get)
                .patch(collections::update)
                .delete(collections::delete),
        );
    let default_auth = Router::with_path("api/auth")
        .push(Router::with_path("register").post(auth::register_default))
        .push(Router::with_path("login").post(auth::login_default))
        .push(Router::with_path("refresh").post(auth::refresh_default))
        .push(Router::with_path("me").get(auth::me_default));
    let collection_auth = Router::with_path("api/auth/{collection}")
        .push(Router::with_path("register").post(auth::register_collection))
        .push(Router::with_path("login").post(auth::login_collection))
        .push(Router::with_path("refresh").post(auth::refresh_collection))
        .push(Router::with_path("me").get(auth::me_collection));
    let admin_auth = Router::with_path("api/admin/auth")
        .push(Router::with_path("login").post(auth::login_admin))
        .push(Router::with_path("refresh").post(auth::refresh_admin))
        .push(Router::with_path("me").get(auth::me_admin));
    let admin_logs = Router::with_path("api/admin/logs").get(logs::list);
    let web_projects = Router::with_path("_/web-projects")
        .get(web::list_projects)
        .post(web::deploy_project)
        .push(
            Router::with_path("{project}")
                .get(web::get_project)
                .patch(web::patch_project)
                .delete(web::delete_project)
                .push(Router::with_path("versions").get(web::list_versions))
                .push(Router::with_path("rollback").post(web::rollback_project)),
        );
    let files_router = Router::with_path("api/files")
        .push(Router::with_path("token").post(files::token))
        .push(
            Router::with_path("{collection}/{recordId}/{field}/{filename}")
                .get(files::download)
                .head(files::download),
        );

    let mut root = Router::new();
    if let Some(logger) = request_logger {
        root = root.hoop(logger);
    }
    root.push(Router::with_path("api/realtime/{collection}").get(realtime::subscribe))
        .push(records_router)
        .push(collections_router)
        .push(default_auth)
        .push(collection_auth)
        .push(admin_auth)
        .push(admin_logs)
        .push(web_projects)
        .push(files_router)
        .push(
            Router::with_path("web/{**rest}")
                .get(web::serve_project)
                .head(web::serve_project),
        )
        .push(Router::with_path("api-doc/openapi.json").get(docs::openapi))
        .push(SwaggerUi::new("/api-doc/openapi.json").into_router("/swagger-ui"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realtime_limiter_enforces_both_limits_and_releases_permits() {
        let limiter = RealtimeLimiter::new(2, 1);
        let first = limiter.try_acquire("192.0.2.1".into()).unwrap();
        assert!(matches!(
            limiter.try_acquire("192.0.2.1".into()),
            Err(herta_core::HbError::RateLimited)
        ));
        let second = limiter.try_acquire("192.0.2.2".into()).unwrap();
        assert!(matches!(
            limiter.try_acquire("192.0.2.3".into()),
            Err(herta_core::HbError::RateLimited)
        ));
        drop(first);
        assert!(limiter.try_acquire("192.0.2.1".into()).is_ok());
        drop(second);
    }
}
