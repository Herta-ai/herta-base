use std::sync::Arc;

use herta_core::HbConfig;
use herta_db::DbClient;
use salvo::{oapi::swagger_ui::SwaggerUi, prelude::*};

use crate::{
    docs::OpenApiCache,
    handlers::{collections, docs, records},
};

#[derive(Clone)]
pub struct ApiState {
    pub db: DbClient,
    pub config: Arc<HbConfig>,
    pub docs: OpenApiCache,
}

impl ApiState {
    pub async fn new(db: DbClient, config: HbConfig) -> herta_core::HbResult<Self> {
        let state = Self {
            db,
            config: Arc::new(config),
            docs: OpenApiCache::empty(),
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

    Router::new()
        .push(records_router)
        .push(collections_router)
        .push(Router::with_path("api-doc/openapi.json").get(docs::openapi))
        .push(SwaggerUi::new("/api-doc/openapi.json").into_router("/swagger-ui"))
}
