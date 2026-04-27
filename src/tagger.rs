use anyhow::Result;
use std::path::Path;
use sha2::{Sha256, Digest};
use md5::{Md5, Digest as Md5Digest};

pub async fn compute_file_hash(path: &Path) -> Result<String> {
    let bytes = tokio::fs::read(path).await?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

pub async fn compute_file_md5(path: &Path) -> Result<String> {
    let bytes = tokio::fs::read(path).await?;
    let mut hasher = Md5::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

pub fn guess_file_type(name: &str) -> Option<String> {
    let ext = Path::new(name)
        .extension()?
        .to_string_lossy()
        .to_lowercase();

    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "svg" | "bmp" | "ico" | "tiff" | "tif" => {
            Some("image".to_string())
        }
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" => {
            Some("video".to_string())
        }
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "wma" | "m4a" => {
            Some("audio".to_string())
        }
        "pdf" | "doc" | "docx" | "txt" | "rtf" | "odt" | "xls" | "xlsx" | "ppt" | "pptx" => {
            Some("document".to_string())
        }
        "zip" | "tar" | "gz" | "rar" | "7z" | "bz2" | "xz" => {
            Some("archive".to_string())
        }
        "rs" | "js" | "ts" | "py" | "go" | "java" | "c" | "cpp" | "h" | "hpp" | "cs" | "rb" | "php" | "swift" | "kt" => {
            Some("code".to_string())
        }
        "html" | "css" | "scss" | "sass" | "less" | "json" | "xml" | "yaml" | "yml" | "toml" | "md" => {
            Some("web".to_string())
        }
        _ => None,
    }
}

pub fn suggest_tags(name: &Path) -> Vec<String> {
    let mut tags = Vec::new();
    let name_lower = name.file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let year_patterns = ["2024", "2025", "2026"];
    for year in year_patterns {
        if name_lower.contains(year) {
            tags.push(year.to_string());
        }
    }

    if let Some(ext) = name.extension() {
        if let Some(ext_str) = ext.to_str() {
            tags.push(ext_str.to_lowercase());
        }
    }

    if let Some(file_type) = guess_file_type(&name.to_string_lossy()) {
        tags.push(file_type);
    }

    tags.sort();
    tags.dedup();
    tags
}

pub async fn run_daemon(_db: crate::db::Database, _config: crate::config::Config) -> Result<()> {
    tracing::info!("File watcher daemon placeholder - use notify for actual file watching");
    std::future::pending::<()>().await;
    Ok(())
}