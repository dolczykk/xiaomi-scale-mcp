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
    #[serde(default)]
    pub(crate) logging: LoggingConfig,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct LoggingConfig {
    pub(crate) enabled: bool,
    pub(crate) level: LogLevel,
    pub(crate) output: LogOutput,
    pub(crate) directory: PathBuf,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            level: LogLevel::Info,
            output: LogOutput::File,
            directory: PathBuf::from("./data/logs"),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LogOutput {
    Console,
    #[default]
    File,
    Both,
}

impl LogOutput {
    pub(crate) const fn writes_to_file(self) -> bool {
        matches!(self, Self::File | Self::Both)
    }
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

        if self.logging.enabled
            && self.logging.output.writes_to_file()
            && self.logging.directory.as_os_str().is_empty()
        {
            bail!("logging.directory must not be empty when file logging is enabled");
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
    use std::path::Path;

    use super::{Config, LogLevel, LogOutput};

    fn parse_logging_config(logging: &str) -> Result<Config, toml::de::Error> {
        toml::from_str(&format!(
            r#"
                [server]
                authorization_token = "mcp-secret"

                [xiaomi]

                [logging]
                {logging}
            "#,
        ))
    }

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
        assert!(config.logging.enabled);
        assert_eq!(config.logging.level, LogLevel::Info);
        assert_eq!(config.logging.output, LogOutput::File);
        assert_eq!(config.logging.directory, Path::new("./data/logs"));
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

    #[test]
    fn parses_logging_configuration() {
        let config = parse_logging_config(
            r#"
                enabled = false
                level = "debug"
                output = "both"
                directory = "custom-logs"
            "#,
        )
        .unwrap();

        config.validate().unwrap();
        assert!(!config.logging.enabled);
        assert_eq!(config.logging.level, LogLevel::Debug);
        assert_eq!(config.logging.output, LogOutput::Both);
        assert_eq!(config.logging.directory, Path::new("custom-logs"));
    }
}
