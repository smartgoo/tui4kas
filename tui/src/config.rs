use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default = "default_network")]
    pub network: String,
    #[serde(default = "default_refresh_interval_ms")]
    pub refresh_interval_ms: u64,
    #[serde(default = "default_true")]
    pub analyze_from_pruning_point: bool,
}

fn default_network() -> String {
    "mainnet".to_string()
}

fn default_refresh_interval_ms() -> u64 {
    1000
}

fn default_true() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            url: None,
            network: default_network(),
            refresh_interval_ms: default_refresh_interval_ms(),
            analyze_from_pruning_point: true,
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".tui4kas")
            .join("config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)?;
        let config: Self = toml::from_str(&contents)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&path, contents)?;
        Ok(())
    }

    pub fn valid_networks() -> &'static [&'static str] {
        &["mainnet", "testnet-10", "testnet-11"]
    }

    pub fn cycle_network(&mut self) {
        let networks = Self::valid_networks();
        let idx = networks
            .iter()
            .position(|n| *n == self.network)
            .unwrap_or(0);
        self.network = networks[(idx + 1) % networks.len()].to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let config = AppConfig::default();
        assert_eq!(config.url, None);
        assert_eq!(config.network, "mainnet");
        assert_eq!(config.refresh_interval_ms, 1000);
        assert!(config.analyze_from_pruning_point);
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let config = AppConfig {
            url: Some("ws://127.0.0.1:17110".to_string()),
            network: "testnet-10".to_string(),
            refresh_interval_ms: 500,
            analyze_from_pruning_point: false,
        };
        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: AppConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.url, Some("ws://127.0.0.1:17110".to_string()));
        assert_eq!(deserialized.network, "testnet-10");
        assert_eq!(deserialized.refresh_interval_ms, 500);
        assert!(!deserialized.analyze_from_pruning_point);
    }

    #[test]
    fn cycle_network() {
        let mut config = AppConfig::default();
        assert_eq!(config.network, "mainnet");
        config.cycle_network();
        assert_eq!(config.network, "testnet-10");
        config.cycle_network();
        assert_eq!(config.network, "testnet-11");
        config.cycle_network();
        assert_eq!(config.network, "mainnet");
    }

    #[test]
    fn deserialize_minimal_config() {
        let toml_str = r#"
network = "mainnet"
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.url, None);
        assert_eq!(config.network, "mainnet");
        assert_eq!(config.refresh_interval_ms, 1000);
    }

    #[test]
    fn deserialize_with_url() {
        let toml_str = r#"
url = "ws://10.0.0.1:17110"
network = "testnet-11"
refresh_interval_ms = 2000
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.url, Some("ws://10.0.0.1:17110".to_string()));
        assert_eq!(config.network, "testnet-11");
        assert_eq!(config.refresh_interval_ms, 2000);
    }
}
