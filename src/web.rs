use anyhow::Result;

pub async fn serve_web_ui() -> &'static str {
    include_str!("web/index.html")
}
