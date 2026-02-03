use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub serial: SerialConfig,
    pub database: DatabaseConfig,
    pub notification: NotificationConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SerialConfig {
    pub port_name: String,
    pub baud_rate: u32,
    pub timeout_ms: u64,
    pub max_retry_count: u32,
    pub retry_delay_ms: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub path: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NotificationConfig {
    pub bark_server_url: String,
    pub bark_device_key: String,
    pub enabled: bool,
}

impl Config {
    pub fn load(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)
            .context(format!("Failed to read config file: {}", path))?;

        let config: Config = toml::from_str(&content)
            .context("Failed to parse config file")?;

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        // Validate baud rate
        if self.serial.baud_rate == 0 {
            anyhow::bail!("Invalid baud_rate: must be greater than 0");
        }

        // Validate timeout
        if self.serial.timeout_ms == 0 {
            anyhow::bail!("Invalid timeout_ms: must be greater than 0");
        }

        // Validate retry settings
        if self.serial.max_retry_count == 0 {
            anyhow::bail!("Invalid max_retry_count: must be greater than 0");
        }

        if self.serial.retry_delay_ms == 0 {
            anyhow::bail!("Invalid retry_delay_ms: must be greater than 0");
        }

        // Validate database path
        if self.database.path.is_empty() {
            anyhow::bail!("Database path cannot be empty");
        }

        // Validate notification config if enabled
        if self.notification.enabled {
            if self.notification.bark_server_url.is_empty() {
                anyhow::bail!("Bark server URL cannot be empty when notifications are enabled");
            }
            if self.notification.bark_device_key.is_empty() {
                anyhow::bail!("Bark device key cannot be empty when notifications are enabled");
            }
        }

        Ok(())
    }
}
