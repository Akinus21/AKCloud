use anyhow::Result;
use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;

use crate::config::Config;
use crate::db::{Database, FileRecord, SearchResult, Stats, TagRecord};
use crate::tagger::compute_file_hash;

pub async fn create_router(db: Database, config: Config) -> Result<Router> {
    let state = Arc::new(AppState { db, config });

    let app = Router::new()
        .route("/", get(serve_index))
        .route("/health", get(health_check))
        
        .route("/api/files", get(list_files))
        .route("/api/files", post(upload_file))
        .route("/api/files/search", get(search_files))
        .route("/api/files/:id", get(get_file))
        .route("/api/files/:id/tags", get(get_file_tags))
        .route("/api/files/:path", delete(delete_file))
        .route("/api/files/:path/download", get(download_file))
        
        .route("/api/tags", get(list_tags))
        .route("/api/tags", post(create_tag))
        .route("/api/tags/:name", delete(remove_tag))
        
        .route("/api/file-tags/:path/:tag", put(tag_file))
        .route("/api/file-tags/:path/:tag", delete(untag_file))
        
        .route("/api/stats", get(get_stats))
        
        .route("/api/sync/manifest", get(get_sync_manifest))
        .route("/api/sync/files/:path", post(sync_upload_file))
        .route("/api/sync/files/:path", get(sync_download_file))
        
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

async fn upload_file(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let upload_path = state.config.storage.upload_path.clone();

    while let Some(field) = multipart.next_field().await.unwrap() {
        let filename = field.file_name().unwrap_or("unknown").to_string();
        let filepath = upload_path.join(&filename);

        let data = match field.bytes().await {
            Ok(d) => d,
            Err(e) => {
                tracing::error!("Failed to read field: {}", e);
                return Json(json!({ "error": format!("Failed to read field: {}", e) })).into_response();
            }
        };

        let mut file = match File::create(&filepath).await {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("Failed to create file: {}", e);
                return Json(json!({ "error": format!("Failed to create file: {}", e) })).into_response();
            }
        };

        if let Err(e) = file.write_all(&data).await {
            tracing::error!("Failed to write data: {}", e);
            return Json(json!({ "error": format!("Failed to write data: {}", e) })).into_response();
        }

        let path_str = filepath.to_string_lossy().to_string();
        let hash = match compute_file_hash(&filepath).await {
            Ok(h) => h,
            Err(e) => {
                tracing::error!("Failed to hash file: {}", e);
                return Json(json!({ "error": format!("Failed to hash file: {}", e) })).into_response();
            }
        };

        let metadata = std::fs::metadata(&filepath).unwrap();
        let size = metadata.len() as i64;
        let mtime = metadata.modified().unwrap()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;

        match state.db.upsert_file(&path_str, &filename, size, &hash, mtime).await {
            Ok(file_id) => {
                tracing::info!("Uploaded file: {} (id: {})", filename, file_id);
            }
            Err(e) => {
                tracing::error!("Failed to save file record: {}", e);
                return Json(json!({ "error": format!("Failed to save file record: {}", e) })).into_response();
            }
        }

        return Json(json!({
            "success": true,
            "filename": filename,
            "path": path_str,
            "hash": hash
        })).into_response();
    }

    Json(json!({ "error": "No file provided" })).into_response()
}

async fn download_file(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
) -> impl IntoResponse {
    let upload_path = state.config.storage.upload_path.clone();
    let filepath = upload_path.join(&path);
    
    if !filepath.exists() {
        return Json(json!({ "error": "File not found" })).into_response();
    }

    let file = match File::open(&filepath).await {
        Ok(f) => f,
        Err(e) => {
            tracing::error!("Failed to open file: {}", e);
            return Json(json!({ "error": format!("Failed to open file: {}", e) })).into_response();
        }
    };

    let filename = filepath.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    IntoResponse::into_response(
        Response::builder()
            .header("Content-Disposition", format!("attachment; filename=\"{}\"", filename))
            .header("Content-Type", "application/octet-stream")
            .body(body)
            .unwrap_or_else(|_| Json(json!({ "error": "Failed to build response" })).into_response())
    )
}

async fn sync_upload_file(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let upload_path = state.config.storage.upload_path.clone();
    let filepath = upload_path.join(&path);

    if let Some(parent) = filepath.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            tracing::error!("Failed to create directory: {}", e);
            return Json(json!({ "error": format!("Failed to create directory: {}", e) })).into_response();
        }
    }

    while let Some(field) = multipart.next_field().await.unwrap() {
        let data = match field.bytes().await {
            Ok(d) => d,
            Err(e) => {
                tracing::error!("Failed to read field: {}", e);
                return Json(json!({ "error": format!("Failed to read field: {}", e) })).into_response();
            }
        };

        let mut file = match File::create(&filepath).await {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("Failed to create file: {}", e);
                return Json(json!({ "error": format!("Failed to create file: {}", e) })).into_response();
            }
        };

        if let Err(e) = file.write_all(&data).await {
            tracing::error!("Failed to write data: {}", e);
            return Json(json!({ "error": format!("Failed to write data: {}", e) })).into_response();
        }

        let filename = filepath.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());

        let hash = compute_file_hash(&filepath).await.unwrap_or_default();
        let metadata = std::fs::metadata(&filepath).unwrap();
        let size = metadata.len() as i64;
        let mtime = metadata.modified().unwrap()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;

        state.db.upsert_file(&path, &filename, size, &hash, mtime).await.ok();

        return Json(json!({ "success": true, "hash": hash })).into_response();
    }

    Json(json!({ "error": "No file provided" })).into_response()
}

async fn sync_download_file(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
) -> impl IntoResponse {
    let upload_path = state.config.storage.upload_path.clone();
    let filepath = upload_path.join(&path);

    if !filepath.exists() {
        return Json(json!({ "error": "File not found" })).into_response();
    }

    let file = match File::open(&filepath).await {
        Ok(f) => f,
        Err(e) => {
            tracing::error!("Failed to open file: {}", e);
            return Json(json!({ "error": format!("Failed to open file: {}", e) })).into_response();
        }
    };

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    IntoResponse::into_response(
        Response::builder()
            .header("Content-Type", "application/octet-stream")
            .body(body)
            .unwrap_or_else(|_| Json(json!({ "error": "Failed to build response" })).into_response())
    )
}