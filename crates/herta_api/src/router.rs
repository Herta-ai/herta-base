use std::sync::Arc;

use herta_auth::AuthService;
use herta_core::HbConfig;
use herta_db::DbClient;
use salvo::{oapi::swagger_ui::SwaggerUi, prelude::*};

use crate::{
    docs::OpenApiCache,
    handlers::{auth, collections, docs, records},
};

#[derive(Clone)]
pub struct ApiState {
    pub db: DbClient,
    pub config: Arc<HbConfig>,
    pub docs: OpenApiCache,
    pub auth: AuthService,
}

impl ApiState {
    pub async fn new(db: DbClient, config: HbConfig) -> herta_core::HbResult<Self> {
        let auth = AuthService::new(db.clone(), &config).await?;
        let state = Self {
            db,
            config: Arc::new(config),
            docs: OpenApiCache::empty(),
            auth,
        };
        state.docs.refresh(&state).await?;
        Ok(state)
    }
}

pub fn build_router() -> Router {
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

    Router::new()
        .push(records_router)
        .push(collections_router)
        .push(default_auth)
        .push(collection_auth)
        .push(admin_auth)
        .push(Router::with_path("api-doc/openapi.json").get(docs::openapi))
        .push(SwaggerUi::new("/api-doc/openapi.json").into_router("/swagger-ui"))
}
