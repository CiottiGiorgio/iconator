use crate::{AppState, CachedSvg, constants::IMMUTABLE};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use iconator::{get_icon_for_file, get_icon_for_folder};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct PathQuery {
    path: String,
}

pub async fn file_id(Query(q): Query<PathQuery>) -> impl IntoResponse {
    match get_icon_for_file(q.path) {
        Some(id) => id.to_string().into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn folder_id(Query(q): Query<PathQuery>) -> impl IntoResponse {
    match get_icon_for_folder(q.path) {
        Some(id) => id.to_string().into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

// Returns the SVG for an icon ID.
pub async fn svg(
    State(state): State<AppState>,
    Path(id): Path<u64>,
    req_headers: HeaderMap,
) -> impl IntoResponse {
    let dir = state.icons_dir.clone();
    let svg = state
        .cache
        .try_get_with(id, async move {
            let bytes = tokio::fs::read(dir.join(format!("{id}.svg"))).await?;
            let etag = format!("\"{}\"", blake3::hash(&bytes).to_hex());
            Ok::<_, std::io::Error>(Arc::new(CachedSvg { bytes, etag }))
        })
        .await;
    let Ok(svg) = svg else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let headers = [
        (header::CACHE_CONTROL, IMMUTABLE),
        (header::ETAG, svg.etag.as_str()),
    ];

    let matches = req_headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.split(',')
                .map(|t| t.trim().trim_start_matches("W/"))
                .any(|t| t == "*" || t == svg.etag)
        });
    if matches {
        return (StatusCode::NOT_MODIFIED, headers).into_response();
    }

    (
        headers,
        [(header::CONTENT_TYPE, "image/svg+xml")],
        svg.bytes.clone(),
    )
        .into_response()
}
