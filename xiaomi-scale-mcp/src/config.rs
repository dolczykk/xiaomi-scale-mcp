use std::{env, fs, path::PathBuf};

use anyhow::{Context, bail};
use serde::Deserialize;

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

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct XiaomiConfig {
    pub(crate) token: String,
    pub(crate) sid: Option<String>,
    pub(crate) device_id: Option<String>,
    pub(crate) region: Option<String>,
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

        let token = self.xiaomi.token.trim();
        let (user_id, pass_token) = token
            .split_once(':')
            .context("xiaomi.token must use the userId:passToken format")?;

        if user_id.is_empty() || pass_token.is_empty() {
            bail!("xiaomi.token must contain a non-empty user ID and pass token");
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
                token = "123:pass-token"
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
                token = "123:pass-token"
            "#,
        )
        .unwrap();

        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_malformed_xiaomi_token() {
        let config: Config = toml::from_str(
            r#"
                [server]
                authorization_token = "mcp-secret"

                [xiaomi]
                token = "malformed"
            "#,
        )
        .unwrap();

        assert!(config.validate().is_err());
    }
}
