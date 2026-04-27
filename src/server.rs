use anyhow::Result;
use axum::{
    body::Body,
    extract::{Query, Path, State},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::config::Config;
use crate::db::{Database, FileRecord, SearchResult, Stats, TagRecord};

pub async fn create_router(db: Database, config: Config) -> Result<Router> {
    let state = Arc::new(AppState { db, config });

    let app = Router::new()
        .route("/", get(serve_index))
        .route("/health", get(health_check))
        
        .route("/api/files", get(list_files))
        .route("/api/files/search", get(search_files))
        .route("/api/files/:id", get(get_file))
        .route("/api/files/:id/tags", get(get_file_tags))
        .route("/api/files/:path", delete(delete_file))
        
        .route("/api/tags", get(list_tags))
        .route("/api/tags", post(create_tag))
        .route("/api/tags/:name", delete(remove_tag))
        
        .route("/api/files/:path/tags/:tag", put(tag_file))
        .route("/api/files/:path/tags/:tag", delete(untag_file))
        
        .route("/api/stats", get(get_stats))
        
        .route("/api/sync/manifest", get(get_sync_manifest))
        
        .with_state(state);

    Ok(app)
}

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub config: Config,
}

async fn serve_index() -> impl IntoResponse {
    Html(include_str!("web/index.html"))
}

async fn health_check() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}

#[derive(Debug, Deserialize)]
pub struct ListFilesQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn list_files(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListFilesQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);

    match state.db.list_files(limit, offset).await {
        Ok(files) => Json(files).into_response(),
        Err(e) => {
            tracing::error!("Error listing files: {}", e);
            Json(json!({ "error": e.to_string() })).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    q: String,
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn search_files(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);

    match state.db.search_files(&query.q, limit, offset).await {
        Ok(result) => Json(result).into_response(),
        Err(e) => {
            tracing::error!("Error searching files: {}", e);
            Json(json!({ "error": e.to_string() })).into_response()
        }
    }
}

async fn get_file(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    Json(json!({ "error": "not implemented" }))
}

async fn delete_file(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
) -> impl IntoResponse {
    match state.db.delete_file(&path).await {
        Ok(_) => Json(json!({ "deleted": true })).into_response(),
        Err(e) => {
            tracing::error!("Error deleting file: {}", e);
            Json(json!({ "error": e.to_string() })).into_response()
        }
    }
}

async fn get_file_tags(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
) -> impl IntoResponse {
    match state.db.get_file_tags(&path).await {
        Ok(tags) => Json(tags).into_response(),
        Err(e) => {
            tracing::error!("Error getting file tags: {}", e);
            Json(json!({ "error": e.to_string() })).into_response()
        }
    }
}

async fn list_tags(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.list_tags().await {
        Ok(tags) => Json(tags).into_response(),
        Err(e) => {
            tracing::error!("Error listing tags: {}", e);
            Json(json!({ "error": e.to_string() })).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateTagRequest {
    name: String,
    color: Option<String>,
}

async fn create_tag(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateTagRequest>,
) -> impl IntoResponse {
    let color = payload.color.unwrap_or_else(|| "#6366f1".to_string());

    match state.db.create_tag(&payload.name, &color).await {
        Ok(tag) => Json(tag).into_response(),
        Err(e) => {
            tracing::error!("Error creating tag: {}", e);
            Json(json!({ "error": e.to_string() })).into_response()
        }
    }
}

async fn remove_tag(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.db.delete_tag(&name).await {
        Ok(_) => Json(json!({ "deleted": true })).into_response(),
        Err(e) => {
            tracing::error!("Error removing tag: {}", e);
            Json(json!({ "error": e.to_string() })).into_response()
        }
    }
}

async fn tag_file(
    State(state): State<Arc<AppState>>,
    Path((path, tag)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.db.tag_file(&path, &tag).await {
        Ok(_) => Json(json!({ "tagged": true })).into_response(),
        Err(e) => {
            tracing::error!("Error tagging file: {}", e);
            Json(json!({ "error": e.to_string() })).into_response()
        }
    }
}

async fn untag_file(
    State(state): State<Arc<AppState>>,
    Path((path, tag)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.db.untag_file(&path, &tag).await {
        Ok(_) => Json(json!({ "untagged": true })).into_response(),
        Err(e) => {
            tracing::error!("Error untagging file: {}", e);
            Json(json!({ "error": e.to_string() })).into_response()
        }
    }
}

async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.get_stats().await {
        Ok(stats) => Json(stats).into_response(),
        Err(e) => {
            tracing::error!("Error getting stats: {}", e);
            Json(json!({ "error": e.to_string() })).into_response()
        }
    }
}

async fn get_sync_manifest(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.get_manifest().await {
        Ok(manifest) => {
            let items: Vec<_> = manifest.into_iter().map(|(path, hash, size, mtime)| {
                json!({ "path": path, "hash": hash, "size": size, "mtime": mtime })
            }).collect();
            Json(items).into_response()
        }
        Err(e) => {
            tracing::error!("Error getting manifest: {}", e);
            Json(json!({ "error": e.to_string() })).into_response()
        }
    }
}