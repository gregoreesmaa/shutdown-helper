use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub port: u16,
    pub bind_address: String,
    pub auth_token: String,
    pub log_dir: String,
}

impl Config {
    pub fn load() -> Result<Self> {
        let exe_path = std::env::current_exe()?;
        let exe_dir = exe_path.parent().ok_or_else(|| anyhow::anyhow!("Could not find executable directory"))?;
        let config_path = exe_dir.join("config.toml");

        if !config_path.exists() {
            let default_config = Config {
                port: 8080,
                bind_address: "127.0.0.1".to_string(),
                auth_token: "change-me-secret-token".to_string(),
                log_dir: "logs".to_string(),
            };
            let toml = toml::to_string_pretty(&default_config)?;
            fs::write(&config_path, toml)?;
            return Ok(default_config);
        }

        let content = fs::read_to_string(&config_path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}
