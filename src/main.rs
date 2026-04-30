use anyhow::Result;
use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub mod config;
pub mod db;
pub mod graveyard;
pub mod server;
pub mod sync;
pub mod tagger;
pub mod web;

use config::{get_config_dir, Config};
use server::create_router;

#[derive(Debug, Clone, Parser)]
#[command(name = "aktags-cloud")]
#[command(about = "AKCloud - Self-hosted file sync and tagging server", long_about = None)]
pub struct Cli {
    #[arg(short, long, default_value = None)]
    pub config: Option<PathBuf>,

    #[arg(short, long, default_value = "8080")]
    pub port: Option<u16>,

    #[arg(short, long, default_value = "0.0.0.0")]
    pub host: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum Commands {
    Server,
    Daemon,
    Sync,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let config_path = cli
        .config
        .unwrap_or_else(|| get_config_dir().join("config.toml"));
    let mut config = Config::load(&config_path).map_err(|e| anyhow::anyhow!("{}", e))?;

    if let Some(port) = cli.port {
        config.server.port = port;
    }
    if let Some(host) = cli.host {
        config.server.host = host.parse()?;
    }

    let log_dir = config.logging.dir.clone();
    std::fs::create_dir_all(&log_dir)?;

    let file_appender = tracing_appender::rolling::daily(&log_dir, "aktags-cloud.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| config.logging.level.clone().into()),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(non_blocking))
        .init();

    std::mem::forget(_guard);

    tracing::info!("AKCloud starting up");
    tracing::info!("Config: {:?}", config_path);
    tracing::info!("Node ID: {}", config.node_id());

    let db = db::Database::new(&config.storage.db_path).await?;
    db.run_migrations().await?;

    match cli.command {
        Some(Commands::Server) | None => {
            let app = create_router(db.clone(), config.clone()).await?;
            let addr = SocketAddr::new(config.server.host, config.server.port);
            tracing::info!("Starting server on {}", addr);

            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(listener, app).await?;
        }
        Some(Commands::Daemon) => {
            tracing::info!("Starting file watcher daemon");
            tagger::run_daemon(db.clone(), config.clone()).await?;
        }
        Some(Commands::Sync) => {
            tracing::info!("Starting P2P sync service");
            sync::run_sync(db.clone(), config.clone()).await?;
        }
    }

    Ok(())
}
