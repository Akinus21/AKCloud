use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Peer {
    pub node_id: String,
    pub display_name: Option<String>,
    pub addr: SocketAddr,
    pub last_seen: std::time::Instant,
}

pub struct DiscoveryService {
    peers: Arc<parking_lot::RwLock<Vec<Peer>>>,
}

impl DiscoveryService {
    pub fn new() -> Self {
        Self {
            peers: Arc::new(parking_lot::RwLock::new(Vec::new())),
        }
    }

    pub fn register_peer(&self, node_id: String, display_name: Option<String>, addr: SocketAddr) {
        let mut peers = self.peers.write();
        
        if let Some(existing) = peers.iter_mut().find(|p| p.node_id == node_id) {
            existing.addr = addr;
            existing.last_seen = std::time::Instant::now();
            if display_name.is_some() {
                existing.display_name = display_name;
            }
        } else {
            peers.push(Peer {
                node_id,
                display_name,
                addr,
                last_seen: std::time::Instant::now(),
            });
        }
    }

    pub fn get_peers(&self) -> Vec<Peer> {
        self.peers.read().clone()
    }

    pub fn get_peer(&self, node_id: &str) -> Option<Peer> {
        self.peers.read().iter()
            .find(|p| p.node_id == node_id)
            .cloned()
    }

    pub fn remove_peer(&self, node_id: &str) {
        let mut peers = self.peers.write();
        peers.retain(|p| p.node_id != node_id);
    }
}

impl Default for DiscoveryService {
    fn default() -> Self {
        Self::new()
    }
}