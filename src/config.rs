use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_base_url")]
    pub base_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_path")]
    pub path: String,
    #[serde(default = "default_fallback_db_path")]
    pub fallback_path: String,
}

fn default_name() -> String {
    "AyanomiBancho".to_string()
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    5003
}

fn default_base_url() -> String {
    "https://hatsuneakiko.io.vn".to_string()
}

fn default_db_path() -> String {
    "data/roseflower.db".to_string()
}

fn default_fallback_db_path() -> String {
    "../ayanomibancho!/data/roseflower.db".to_string()
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            name: default_name(),
            host: default_host(),
            port: default_port(),
            base_url: default_base_url(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: default_db_path(),
            fallback_path: default_fallback_db_path(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
        }
    }
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        if path.as_ref().exists() {
            let content = fs::read_to_string(path)?;
            let cfg: Config = toml::from_str(&content)?;
            Ok(cfg)
        } else {
            Ok(Config::default())
        }
    }

    /// Resolves the actual database path to use, checking primary then fallback then creating parent directories.
    pub fn resolve_db_path(&self) -> String {
        if Path::new(&self.database.path).exists() {
            self.database.path.clone()
        } else if Path::new(&self.database.fallback_path).exists() {
            self.database.fallback_path.clone()
        } else {
            self.database.path.clone()
        }
    }
}
