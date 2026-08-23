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
    #[serde(default = "default_allowed_hosts")]
    pub(crate) allowed_hosts: Vec<String>,
    pub(crate) authorization_token: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct XiaomiConfig {
    pub(crate) sid: Option<String>,
    pub(crate) region: Option<String>,
}

impl XiaomiConfig {
    pub(crate) fn client(&self) -> anyhow::Result<Client> {
        let mut client = Client::new().context("failed to initialize Xiaomi client")?;

        if let Some(sid) = non_empty(&self.sid) {
            client = client.with_sid(sid.to_string());
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

        if self.server.allowed_hosts.is_empty()
            || self
                .server
                .allowed_hosts
                .iter()
                .any(|host| host.trim().is_empty())
        {
            bail!("server.allowed_hosts must contain only non-empty hosts");
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

fn default_allowed_hosts() -> Vec<String> {
    vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ]
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
        assert_eq!(
            config.server.allowed_hosts,
            ["localhost", "127.0.0.1", "::1"]
        );
        assert_eq!(config.xiaomi.region.as_deref(), Some("de"));
    }

    #[test]
    fn accepts_explicit_allowed_hosts() {
        let config: Config = toml::from_str(
            r#"
                [server]
                authorization_token = "mcp-secret"
                allowed_hosts = ["localhost", "192.168.1.11"]

                [xiaomi]
            "#,
        )
        .unwrap();

        config.validate().unwrap();
        assert_eq!(config.server.allowed_hosts, ["localhost", "192.168.1.11"]);
    }

    #[test]
    fn rejects_empty_allowed_host_entries() {
        let config: Config = toml::from_str(
            r#"
                [server]
                authorization_token = "mcp-secret"
                allowed_hosts = ["localhost", ""]

                [xiaomi]
            "#,
        )
        .unwrap();

        assert!(config.validate().is_err());
    }
}
