use axum::{Router, routing::get};
use constants::{CACHE_BYTES, FILE_ENDPOINT, FOLDER_ENDPOINT, SVG_ENDPOINT};
use methods::get::{file_id, folder_id, svg};
use moka::future::Cache;
use std::{path::PathBuf, sync::Arc};

mod constants;
mod methods;

pub struct CachedSvg {
    pub bytes: Vec<u8>,
    // Quoted BLAKE3 digest of `bytes`, ready to use as the ETag header value.
    pub etag: String,
}

#[derive(Clone)]
pub struct AppState {
    // A OnceLock could also work here, but an Arc<PathBuf>> is also fine.
    // As long as we avoid allocating Strings and reading from env with each request.
    pub icons_dir: Arc<PathBuf>,
    pub cache: Cache<u64, Arc<CachedSvg>>,
}

fn app(icons_dir: PathBuf) -> Router {
    let state = AppState {
        icons_dir: Arc::new(icons_dir),
        cache: Cache::builder()
            .max_capacity(CACHE_BYTES)
            .weigher(|_, v: &Arc<CachedSvg>| {
                v.bytes
                    .len()
                    .try_into()
                    .expect("SVG size larger than a u32")
            })
            .build(),
    };

    Router::new()
        .route(FILE_ENDPOINT, get(file_id))
        .route(FOLDER_ENDPOINT, get(folder_id))
        .route(SVG_ENDPOINT, get(svg))
        .with_state(state)
}

#[tokio::main]
async fn main() {
    let icons_dir = std::env::var("ICONS_DIR").unwrap_or_else(|_| "icons".into());

    let addr = std::env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app(icons_dir.into())).await.unwrap();
}

#[cfg(test)]
mod test {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode, header},
    };
    use tower::ServiceExt;

    async fn send(uri: String, if_none_match: Option<&str>) -> axum::response::Response {
        let mut req = Request::get(uri);
        if let Some(etag) = if_none_match {
            req = req.header(header::IF_NONE_MATCH, etag);
        }
        app("icons".into())
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    async fn get_file(path: Option<&str>) -> axum::response::Response {
        send(query(FILE_ENDPOINT, path), None).await
    }

    async fn get_folder(path: Option<&str>) -> axum::response::Response {
        send(query(FOLDER_ENDPOINT, path), None).await
    }

    async fn get_svg(id: &str, if_none_match: Option<&str>) -> axum::response::Response {
        send(SVG_ENDPOINT.replace("{id}", id), if_none_match).await
    }

    fn query(endpoint: &str, path: Option<&str>) -> String {
        match path {
            Some(path) => format!("{endpoint}?path={path}"),
            None => endpoint.to_owned(),
        }
    }

    async fn text(res: axum::response::Response) -> String {
        String::from_utf8(to_bytes(res.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap()
    }

    #[tokio::test]
    async fn common_languages() {
        for (name, id) in [
            ("test.js", "296"),
            ("test.ts", "633"),
            ("test.jsx", "508"),
            ("test.tsx", "529"),
            ("test.json", "284"),
            ("test.yaml", "708"),
            ("test.yml", "708"),
            ("test.md", "361"),
            ("test.css", "116"),
            ("test.html", "255"),
            ("test.rs", "525"),
        ] {
            let res = get_file(Some(&format!("./{name}"))).await;
            assert_eq!(res.status(), StatusCode::OK, "{name}");
            assert_eq!(text(res).await, id, "{name}");
        }
    }

    #[tokio::test]
    async fn common_folders() {
        for (name, id) in [(".github", "862"), ("src", "1054"), ("tests", "1084")] {
            let res = get_folder(Some(&format!("./{name}"))).await;
            assert_eq!(res.status(), StatusCode::OK, "{name}");
            assert_eq!(text(res).await, id, "{name}");
        }
    }

    #[tokio::test]
    async fn bad_requests() {
        assert_eq!(get_file(None).await.status(), StatusCode::BAD_REQUEST);
        assert_eq!(get_folder(None).await.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            get_svg("not-a-number", None).await.status(),
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn client_side_caching() {
        let res = get_svg("525", None).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()[header::CONTENT_TYPE], "image/svg+xml");
        assert!(
            res.headers()[header::CACHE_CONTROL]
                .to_str()
                .unwrap()
                .contains("immutable")
        );
        let etag = res.headers()[header::ETAG].to_str().unwrap().to_owned();

        // Revalidating with the ETag yields an empty 304 that repeats the validators.
        let res = get_svg("525", Some(&etag)).await;
        assert_eq!(res.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(res.headers()[header::ETAG], etag.as_str());
        assert!(text(res).await.is_empty());

        // A stale ETag gets the full body again.
        let res = get_svg("525", Some("\"stale\"")).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert!(!text(res).await.is_empty());
    }
}
