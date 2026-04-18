use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub port: u16,
    pub auth_token: String,
    pub log_dir: String,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = "config.toml";
        if !Path::new(config_path).exists() {
            let default_config = Config {
                port: 8080,
                auth_token: "change-me-secret-token".to_string(),
                log_dir: "logs".to_string(),
            };
            let toml = toml::to_string_pretty(&default_config)?;
            fs::write(config_path, toml)?;
            return Ok(default_config);
        }

        let content = fs::read_to_string(config_path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}
