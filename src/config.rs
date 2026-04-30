use std::net::IpAddr;
use std::path::PathBuf;

fn get_home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USER_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/data"))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub sync: SyncConfig,
    pub graveyard: GraveyardConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ServerConfig {
    pub host: IpAddr,
    pub port: u16,
    pub external_url: Option<String>,
    pub api_keys: Vec<ApiKey>,
    pub cors_origins: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ApiKey {
    pub name: String,
    pub key: String,
    pub read_only: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct StorageConfig {
    pub watch_paths: Vec<PathBuf>,
    pub upload_path: PathBuf,
    pub db_path: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SyncConfig {
    pub enabled: bool,
    pub listen_port: u16,
    pub relay_server: Option<String>,
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct GraveyardConfig {
    pub path: PathBuf,
    pub ttl_days: i64,
    pub max_size_mb: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LoggingConfig {
    pub dir: PathBuf,
    pub max_files: u32,
    pub level: String,
}

impl Default for Config {
    fn default() -> Self {
        let home = get_home_dir();

        Self {
            server: ServerConfig {
                host: "0.0.0.0".parse().unwrap(),
                port: 8080,
                external_url: None,
                api_keys: vec![ApiKey {
                    name: "admin".to_string(),
                    key: uuid::Uuid::new_v4().to_string(),
                    read_only: false,
                }],
                cors_origins: vec!["*".to_string()],
            },
            storage: StorageConfig {
                watch_paths: vec![PathBuf::from("/data/storage")],
                upload_path: PathBuf::from("/data/storage"),
                db_path: home.join("aktags.db"),
            },
            sync: SyncConfig {
                enabled: false,
                listen_port: 22000,
                relay_server: None,
                node_id: None,
            },
            graveyard: GraveyardConfig {
                path: home.join("graveyard"),
                ttl_days: 30,
                max_size_mb: 500,
            },
            logging: LoggingConfig {
                dir: home.join("logs"),
                max_files: 5,
                level: "info".to_string(),
            },
        }
    }
}

impl Config {
    pub fn load(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        if path.exists() {
            let contents = std::fs::read_to_string(path)?;
            Ok(toml::from_str(&contents)?)
        } else {
            let config = Self::default();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, toml::to_string_pretty(&config)?)?;
            Ok(config)
        }
    }

    pub fn node_id(&mut self) -> String {
        if let Some(ref id) = self.sync.node_id {
            return id.clone();
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.sync.node_id = Some(id.clone());
        id
    }
}

pub fn get_config_dir() -> PathBuf {
    directories::ProjectDirs::from("com", "aktags", "akcloud")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/config"))
}

pub fn get_data_dir() -> PathBuf {
    directories::ProjectDirs::from("com", "aktags", "akcloud")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/data"))
}
