use std::net::SocketAddr;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use url::Url;

pub const DEFAULT_STALE_AFTER_SECS: u64 = 20;
pub const DEFAULT_PING_INTERVAL_SECS: u64 = 10;
pub const DEFAULT_MAX_MESSAGE_BYTES: usize = 64 * 1024;
pub const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 5;
pub const DEFAULT_REPORT_INTERVAL_SECS: u64 = 5;
pub const DEFAULT_HISTORY_RETENTION_HOURS: u64 = 72;
pub const DEFAULT_HISTORY_WRITE_INTERVAL_SECS: u64 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    message: String,
}

impl ConfigError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ConfigError {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerConfig {
    pub listen: SocketAddr,
    pub public_base_url: String,
    pub shared_token: String,
    pub history_db_path: PathBuf,
    pub snapshot_path: PathBuf,
    pub stale_after_secs: u64,
    pub ping_interval_secs: u64,
    pub max_message_bytes: usize,
    pub refresh_interval_secs: u64,
    pub ignored_filesystems: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentConfig {
    pub node_id: String,
    pub node_label: String,
    pub server: String,
    pub token: String,
    pub report_interval_secs: u64,
    pub hostname_override: Option<String>,
    pub tags: Vec<String>,
}

pub fn parse_server_config(input: &str) -> Result<ServerConfig, ConfigError> {
    let raw: RawServerConfigFile =
        toml::from_str(input).map_err(|error| ConfigError::new(error.to_string()))?;
    raw.validate()
}

pub fn parse_agent_config(input: &str) -> Result<AgentConfig, ConfigError> {
    let raw: RawAgentConfigFile =
        toml::from_str(input).map_err(|error| ConfigError::new(error.to_string()))?;
    raw.validate()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawServerConfigFile {
    server: RawServerSection,
    #[serde(default)]
    ui: RawUiSection,
    #[serde(default)]
    filters: RawFiltersSection,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawServerSection {
    listen: String,
    public_base_url: String,
    shared_token: String,
    #[serde(default = "default_history_db_path")]
    history_db_path: PathBuf,
    #[serde(default = "default_snapshot_path")]
    snapshot_path: PathBuf,
    #[serde(default = "default_stale_after_secs")]
    stale_after_secs: u64,
    #[serde(default = "default_ping_interval_secs")]
    ping_interval_secs: u64,
    #[serde(default = "default_max_message_bytes")]
    max_message_bytes: usize,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawUiSection {
    #[serde(default = "default_refresh_interval_secs")]
    refresh_interval_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawFiltersSection {
    #[serde(default = "default_ignored_filesystems")]
    ignored_filesystems: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAgentConfigFile {
    agent: RawAgentSection,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAgentSection {
    node_id: String,
    node_label: String,
    server: String,
    token: String,
    #[serde(default = "default_report_interval_secs")]
    report_interval_secs: u64,
    hostname_override: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

impl RawServerConfigFile {
    fn validate(self) -> Result<ServerConfig, ConfigError> {
        let listen = self
            .server
            .listen
            .parse::<SocketAddr>()
            .map_err(|error| ConfigError::new(format!("invalid server.listen: {error}")))?;
        validate_url(
            "server.public_base_url",
            &self.server.public_base_url,
            &["http", "https"],
        )?;
        validate_non_empty("server.shared_token", &self.server.shared_token)?;

        if self.server.stale_after_secs < 5 {
            return Err(ConfigError::new(
                "server.stale_after_secs must be at least 5 seconds",
            ));
        }
        if self.server.ping_interval_secs < 1 {
            return Err(ConfigError::new(
                "server.ping_interval_secs must be at least 1 second",
            ));
        }
        if self.server.max_message_bytes < 1024 {
            return Err(ConfigError::new(
                "server.max_message_bytes must be at least 1024 bytes",
            ));
        }
        if self.ui.refresh_interval_secs < 1 {
            return Err(ConfigError::new(
                "ui.refresh_interval_secs must be at least 1 second",
            ));
        }

        Ok(ServerConfig {
            listen,
            public_base_url: self.server.public_base_url,
            shared_token: self.server.shared_token,
            history_db_path: self.server.history_db_path,
            snapshot_path: self.server.snapshot_path,
            stale_after_secs: self.server.stale_after_secs,
            ping_interval_secs: self.server.ping_interval_secs,
            max_message_bytes: self.server.max_message_bytes,
            refresh_interval_secs: self.ui.refresh_interval_secs,
            ignored_filesystems: normalize_string_list(self.filters.ignored_filesystems),
        })
    }
}

impl RawAgentConfigFile {
    fn validate(self) -> Result<AgentConfig, ConfigError> {
        validate_identifier("agent.node_id", &self.agent.node_id)?;
        validate_non_empty("agent.node_label", &self.agent.node_label)?;
        validate_url("agent.server", &self.agent.server, &["ws", "wss"])?;
        validate_non_empty("agent.token", &self.agent.token)?;

        if self.agent.report_interval_secs < 1 {
            return Err(ConfigError::new(
                "agent.report_interval_secs must be at least 1 second",
            ));
        }

        if let Some(hostname) = &self.agent.hostname_override {
            validate_non_empty("agent.hostname_override", hostname)?;
        }

        Ok(AgentConfig {
            node_id: self.agent.node_id.trim().to_string(),
            node_label: self.agent.node_label.trim().to_string(),
            server: self.agent.server,
            token: self.agent.token,
            report_interval_secs: self.agent.report_interval_secs,
            hostname_override: self
                .agent
                .hostname_override
                .map(|value| value.trim().to_string()),
            tags: normalize_string_list(self.agent.tags),
        })
    }
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), ConfigError> {
    if value.trim().is_empty() {
        return Err(ConfigError::new(format!("{field} must not be empty")));
    }
    Ok(())
}

fn validate_identifier(field: &str, value: &str) -> Result<(), ConfigError> {
    validate_non_empty(field, value)?;
    if value.len() > 128 {
        return Err(ConfigError::new(format!(
            "{field} must be <= 128 characters"
        )));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Err(ConfigError::new(format!(
            "{field} must use only ASCII letters, numbers, '-', '_' or '.'"
        )));
    }
    Ok(())
}

fn validate_url(field: &str, value: &str, schemes: &[&str]) -> Result<(), ConfigError> {
    let parsed =
        Url::parse(value).map_err(|error| ConfigError::new(format!("invalid {field}: {error}")))?;
    if !schemes.iter().any(|scheme| *scheme == parsed.scheme()) {
        return Err(ConfigError::new(format!(
            "{field} must use one of these schemes: {}",
            schemes.join(", ")
        )));
    }
    Ok(())
}

fn normalize_string_list(values: Vec<String>) -> Vec<String> {
    let mut values: Vec<String> = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect();
    values.sort();
    values.dedup();
    values
}

fn default_history_db_path() -> PathBuf {
    PathBuf::from("./data/history.sqlite3")
}

fn default_snapshot_path() -> PathBuf {
    PathBuf::from("./data/snapshot.json")
}

fn default_stale_after_secs() -> u64 {
    DEFAULT_STALE_AFTER_SECS
}

fn default_ping_interval_secs() -> u64 {
    DEFAULT_PING_INTERVAL_SECS
}

fn default_max_message_bytes() -> usize {
    DEFAULT_MAX_MESSAGE_BYTES
}

fn default_refresh_interval_secs() -> u64 {
    DEFAULT_REFRESH_INTERVAL_SECS
}

fn default_report_interval_secs() -> u64 {
    DEFAULT_REPORT_INTERVAL_SECS
}

fn default_ignored_filesystems() -> Vec<String> {
    vec![
        "devtmpfs".to_string(),
        "overlay".to_string(),
        "tmpfs".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_MAX_MESSAGE_BYTES, parse_agent_config, parse_server_config};

    #[test]
    fn parses_server_config_with_defaults() {
        let config = parse_server_config(
            r#"
            [server]
            listen = "127.0.0.1:8080"
            public_base_url = "http://127.0.0.1:8080"
            shared_token = "token"
            "#,
        )
        .expect("server config should parse");

        assert_eq!(config.listen.to_string(), "127.0.0.1:8080");
        assert_eq!(config.shared_token, "token");
        assert_eq!(config.max_message_bytes, DEFAULT_MAX_MESSAGE_BYTES);
        assert_eq!(
            config.ignored_filesystems,
            vec!["devtmpfs", "overlay", "tmpfs"]
        );
    }

    #[test]
    fn rejects_invalid_server_listen_address() {
        let error = parse_server_config(
            r#"
            [server]
            listen = "oops"
            public_base_url = "http://127.0.0.1:8080"
            shared_token = "token"
            "#,
        )
        .expect_err("invalid config should fail");

        assert!(error.to_string().contains("server.listen"));
    }

    #[test]
    fn rejects_invalid_agent_server_scheme() {
        let error = parse_agent_config(
            r#"
            [agent]
            node_id = "hk-01"
            node_label = "Hong Kong 01"
            server = "http://127.0.0.1:8080/ws"
            token = "token"
            "#,
        )
        .expect_err("invalid agent config should fail");

        assert!(error.to_string().contains("agent.server"));
    }

    #[test]
    fn parses_agent_config() {
        let config = parse_agent_config(
            r#"
            [agent]
            node_id = "hk-01"
            node_label = "Hong Kong 01"
            server = "ws://127.0.0.1:8080/ws"
            token = "token"
            report_interval_secs = 7
            hostname_override = "hk-01.internal"
            tags = [" edge ", "apac"]
            "#,
        )
        .expect("agent config should parse");

        assert_eq!(config.node_id, "hk-01");
        assert_eq!(config.report_interval_secs, 7);
        assert_eq!(config.tags, vec!["apac", "edge"]);
    }
}
