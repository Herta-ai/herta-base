use std::path::Path;

use rust_embed::Embed;
use salvo::{
    http::{Method, StatusCode, header},
    prelude::*,
};

const INDEX_HTML: &str = "index.html";
const NO_CACHE: &str = "no-cache";
const IMMUTABLE_CACHE: &str = "public, max-age=31536000, immutable";

#[derive(Embed)]
#[folder = "../../frontend/admin-ui/dist/"]
struct AdminUiAssets;

pub fn router() -> Router {
    Router::new()
        .push(
            Router::with_path("webui")
                .get(serve_entry)
                .head(serve_entry),
        )
        .push(Router::with_path("webui/{**rest}").get(serve).head(serve))
}

#[handler]
async fn serve_entry(req: &mut Request, res: &mut Response) {
    if req.uri().path().ends_with('/') {
        serve_asset(req, res, INDEX_HTML);
    } else {
        res.render(Redirect::permanent("/webui/"));
    }
}

#[handler]
async fn serve(req: &mut Request, res: &mut Response) {
    let path = req
        .uri()
        .path()
        .strip_prefix("/webui/")
        .unwrap_or_default()
        .trim_start_matches('/');

    if path.is_empty() {
        serve_asset(req, res, INDEX_HTML);
        return;
    }

    if AdminUiAssets::get(path).is_some() {
        serve_asset(req, res, path);
    } else if is_asset_request(path) {
        res.status_code(StatusCode::NOT_FOUND);
    } else {
        serve_asset(req, res, INDEX_HTML);
    }
}

fn serve_asset(req: &Request, res: &mut Response, path: &str) {
    let Some(content) = AdminUiAssets::get(path) else {
        res.status_code(StatusCode::NOT_FOUND);
        return;
    };

    let cache_control = if path == INDEX_HTML {
        NO_CACHE
    } else if path.starts_with("assets/") {
        IMMUTABLE_CACHE
    } else {
        NO_CACHE
    };
    let content_length = content.data.len().to_string();

    res.status_code(StatusCode::OK);
    res.headers_mut().insert(
        header::CONTENT_TYPE,
        content
            .metadata
            .mimetype()
            .parse()
            .expect("rust-embed returned an invalid MIME type"),
    );
    res.headers_mut().insert(
        header::CACHE_CONTROL,
        cache_control.parse().expect("cache policy must be valid"),
    );
    res.headers_mut().insert(
        header::CONTENT_LENGTH,
        content_length
            .parse()
            .expect("content length must be valid"),
    );
    res.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        "nosniff".parse().expect("header value must be valid"),
    );

    if req.method() != Method::HEAD {
        res.body(content.data.into_owned());
    }
}

fn is_asset_request(path: &str) -> bool {
    path.starts_with("assets/") || Path::new(path).extension().is_some()
}

#[cfg(test)]
mod tests {
    use salvo::{
        http::{StatusCode, header},
        prelude::Service,
        test::{ResponseExt, TestClient},
    };

    use super::*;

    #[tokio::test]
    async fn redirects_webui_to_the_canonical_trailing_slash_url() {
        let service = Service::new(router());
        let response = TestClient::get("http://localhost/webui")
            .send(&service)
            .await;

        assert_eq!(response.status_code, Some(StatusCode::PERMANENT_REDIRECT));
        assert_eq!(response.headers().get(header::LOCATION).unwrap(), "/webui/");
    }

    #[tokio::test]
    async fn serves_index_for_the_root_and_spa_routes() {
        let service = Service::new(router());
        for url in [
            "http://localhost/webui/",
            "http://localhost/webui/login",
            "http://localhost/webui/collections/posts",
        ] {
            let mut response = TestClient::get(url).send(&service).await;
            assert_eq!(response.status_code, Some(StatusCode::OK));
            assert_eq!(
                response.headers().get(header::CONTENT_TYPE).unwrap(),
                "text/html"
            );
            assert_eq!(
                response.headers().get(header::CACHE_CONTROL).unwrap(),
                NO_CACHE
            );
            assert!(
                response
                    .take_string()
                    .await
                    .unwrap()
                    .contains("id=\"root\"")
            );
        }
    }

    #[tokio::test]
    async fn serves_embedded_assets_with_their_mime_and_cache_policy() {
        let service = Service::new(router());
        for (extension, expected_mime) in [
            ("js", "text/javascript"),
            ("css", "text/css"),
            ("svg", "image/svg+xml"),
        ] {
            let asset = AdminUiAssets::iter()
                .find(|path| path.ends_with(&format!(".{extension}")))
                .unwrap_or_else(|| panic!("missing embedded .{extension} asset"));
            let mut response = TestClient::get(format!("http://localhost/webui/{asset}"))
                .send(&service)
                .await;

            assert_eq!(response.status_code, Some(StatusCode::OK));
            assert!(
                response
                    .headers()
                    .get(header::CONTENT_TYPE)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with(expected_mime)
            );
            let expected_cache = if asset.starts_with("assets/") {
                IMMUTABLE_CACHE
            } else {
                NO_CACHE
            };
            assert_eq!(
                response.headers().get(header::CACHE_CONTROL).unwrap(),
                expected_cache
            );
            assert!(!response.take_bytes(None).await.unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn missing_static_assets_return_not_found_instead_of_index() {
        let service = Service::new(router());
        let response = TestClient::get("http://localhost/webui/assets/missing.js")
            .send(&service)
            .await;

        assert_eq!(response.status_code, Some(StatusCode::NOT_FOUND));
    }

    #[tokio::test]
    async fn head_returns_asset_headers_without_a_body() {
        let service = Service::new(router());
        let asset = AdminUiAssets::iter()
            .find(|path| path.ends_with(".js"))
            .expect("missing embedded JavaScript asset");
        let mut response = TestClient::head(format!("http://localhost/webui/{asset}"))
            .send(&service)
            .await;

        assert_eq!(response.status_code, Some(StatusCode::OK));
        assert!(response.headers().get(header::CONTENT_LENGTH).is_some());
        assert!(response.take_bytes(None).await.unwrap().is_empty());
    }
}
