use anyhow::Result;
use std::net::SocketAddr;

pub mod discovery;
pub mod identity;
// pub mod transport;
// pub mod protocol;

use crate::config::Config;
use crate::db::Database;

pub async fn run_sync(_db: Database, config: Config) -> Result<()> {
    tracing::info!("Starting P2P sync service");

    let node_id = config
        .sync
        .node_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    tracing::info!("Node ID: {}", node_id);

    if config.sync.enabled {
        let listen_addr = SocketAddr::new("0.0.0.0".parse().unwrap(), config.sync.listen_port);

        tracing::info!("Listening for P2P connections on {}", listen_addr);
    }

    Ok(())
}
