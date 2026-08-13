use std::{env, fs, path::PathBuf};

use anyhow::{Context, bail};
use serde::Deserialize;
use xiaomi_client::Client;

const DEFAULT_CONFIG_PATH: &str = "config.toml";
const CONFIG_PATH_ENV: &str = "MCP_CONFIG_PATH";

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Config {
    pub(crate) server: ServerConfig,
    pub(crate) xiaomi: XiaomiConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ServerConfig {
    #[serde(default = "default_bind_address")]
    pub(crate) bind_address: String,
    pub(crate) authorization_token: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct XiaomiConfig {
    #[serde(default, rename = "token")]
    legacy_token: Option<String>,
    pub(crate) sid: Option<String>,
    pub(crate) device_id: Option<String>,
    pub(crate) region: Option<String>,
}

impl XiaomiConfig {
    pub(crate) fn client(&self) -> anyhow::Result<Client> {
        let mut client = Client::new()
            .context("failed to initialize Xiaomi client")?;

        if let Some(sid) = non_empty(&self.sid) {
            client = client.with_sid(sid.to_string());
        }
        if let Some(device_id) = non_empty(&self.device_id) {
            client = client.with_device_id(device_id.to_string());
        }
        if let Some(region) = non_empty(&self.region) {
            client = client.with_region(region.to_string());
        }

        Ok(client)
    }
}

impl Config {
    pub(crate) fn load() -> anyhow::Result<Self> {
        let path = config_path();
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read configuration file {}", path.display()))?;
        let config = toml::from_str::<Self>(&contents)
            .with_context(|| format!("failed to parse configuration file {}", path.display()))?;

        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.server.authorization_token.trim().is_empty() {
            bail!("server.authorization_token must not be empty");
        }
        if self.server.bind_address.trim().is_empty() {
            bail!("server.bind_address must not be empty");
        }

        if self.xiaomi.legacy_token.is_some() {
            bail!(
                "xiaomi.token is no longer supported; remove it from config.toml and enter auth in the server console"
            );
        }

        Ok(())
    }
}

fn config_path() -> PathBuf {
    env::var_os(CONFIG_PATH_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH))
}

fn default_bind_address() -> String {
    "127.0.0.1:8080".to_string()
}

fn non_empty(value: &Option<String>) -> Option<&str> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn parses_valid_configuration() {
        let config: Config = toml::from_str(
            r#"
                [server]
                authorization_token = "mcp-secret"

                [xiaomi]
                region = "de"
            "#,
        )
        .unwrap();

        config.validate().unwrap();
        assert_eq!(config.server.bind_address, "127.0.0.1:8080");
        assert_eq!(config.xiaomi.region.as_deref(), Some("de"));
    }

    #[test]
    fn rejects_empty_authorization_token() {
        let config: Config = toml::from_str(
            r#"
                [server]
                authorization_token = "  "

                [xiaomi]
            "#,
        )
        .unwrap();

        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_legacy_xiaomi_token() {
        let config: Config = toml::from_str(
            r#"
                [server]
                authorization_token = "mcp-secret"

                [xiaomi]
                token = "123:pass-token"
            "#,
        )
        .unwrap();

        assert!(config.validate().is_err());
    }
}
