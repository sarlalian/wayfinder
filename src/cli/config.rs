// ABOUTME: Configuration management for wayfinder application
// ABOUTME: Handles loading and merging configuration from files and environment variables

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub default_output_dir: Option<PathBuf>,

    #[serde(default)]
    pub max_concurrent_tasks: usize,

    #[serde(default)]
    pub aws: AwsConfig,

    #[serde(default)]
    pub smtp: SmtpConfig,

    #[serde(default)]
    pub slack: SlackConfig,

    #[serde(default)]
    pub template_vars: HashMap<String, String>,

    #[serde(default)]
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AwsConfig {
    pub region: Option<String>,
    pub profile: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub session_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpConfig {
    pub server: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub use_tls: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SlackConfig {
    pub webhook_url: Option<String>,
    pub bot_token: Option<String>,
    pub default_channel: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_output_dir: None,
            max_concurrent_tasks: 4,
            aws: AwsConfig::default(),
            smtp: SmtpConfig::default(),
            slack: SlackConfig::default(),
            template_vars: HashMap::new(),
            logging: LoggingConfig::default(),
        }
    }
}


impl Default for SmtpConfig {
    fn default() -> Self {
        Self {
            server: None,
            port: Some(587),
            username: None,
            password: None,
            use_tls: true,
        }
    }
}


impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "pretty".to_string(),
        }
    }
}

impl Config {
    /// Load configuration from file path or default locations
    pub fn load(path: Option<PathBuf>) -> Result<Self> {
        let config_path = match path {
            Some(p) => p,
            None => Self::find_config_file()?,
        };

        if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)?;
            let mut config: Config = serde_yaml::from_str(&contents)?;

            // Merge with environment variables
            config.merge_env()?;

            Ok(config)
        } else {
            let mut config = Config::default();
            config.merge_env()?;
            Ok(config)
        }
    }

    /// Find configuration file in standard locations
    fn find_config_file() -> Result<PathBuf> {
        let possible_paths = vec![
            PathBuf::from("wayfinder.yaml"),
            PathBuf::from("wayfinder.yml"),
            PathBuf::from(".wayfinder.yaml"),
            PathBuf::from(".wayfinder.yml"),
        ];

        // Check home directory
        if let Some(home_dir) = dirs::home_dir() {
            let home_config = home_dir.join(".wayfinder").join("config.yaml");
            if home_config.exists() {
                return Ok(home_config);
            }
        }

        // Check current directory
        for path in possible_paths {
            if path.exists() {
                return Ok(path);
            }
        }

        // Return default path (may not exist)
        Ok(PathBuf::from("wayfinder.yaml"))
    }

    /// Merge environment variables into configuration
    fn merge_env(&mut self) -> Result<()> {
        // AWS configuration
        if let Ok(region) = std::env::var("AWS_REGION") {
            self.aws.region = Some(region);
        }
        if let Ok(profile) = std::env::var("AWS_PROFILE") {
            self.aws.profile = Some(profile);
        }
        if let Ok(access_key) = std::env::var("AWS_ACCESS_KEY_ID") {
            self.aws.access_key_id = Some(access_key);
        }
        if let Ok(secret_key) = std::env::var("AWS_SECRET_ACCESS_KEY") {
            self.aws.secret_access_key = Some(secret_key);
        }
        if let Ok(session_token) = std::env::var("AWS_SESSION_TOKEN") {
            self.aws.session_token = Some(session_token);
        }

        // SMTP configuration
        if let Ok(server) = std::env::var("SMTP_SERVER") {
            self.smtp.server = Some(server);
        }
        if let Ok(port) = std::env::var("SMTP_PORT") {
            self.smtp.port = Some(port.parse()?);
        }
        if let Ok(username) = std::env::var("SMTP_USERNAME") {
            self.smtp.username = Some(username);
        }
        if let Ok(password) = std::env::var("SMTP_PASSWORD") {
            self.smtp.password = Some(password);
        }

        // Slack configuration
        if let Ok(webhook_url) = std::env::var("SLACK_WEBHOOK_URL") {
            self.slack.webhook_url = Some(webhook_url);
        }
        if let Ok(bot_token) = std::env::var("SLACK_BOT_TOKEN") {
            self.slack.bot_token = Some(bot_token);
        }
        if let Ok(channel) = std::env::var("SLACK_DEFAULT_CHANNEL") {
            self.slack.default_channel = Some(channel);
        }

        // Logging configuration
        if let Ok(level) = std::env::var("WAYFINDER_LOG_LEVEL") {
            self.logging.level = level;
        }
        if let Ok(format) = std::env::var("WAYFINDER_LOG_FORMAT") {
            self.logging.format = format;
        }

        // Max concurrent tasks
        if let Ok(max_tasks) = std::env::var("WAYFINDER_MAX_CONCURRENT") {
            self.max_concurrent_tasks = max_tasks.parse()?;
        }

        Ok(())
    }

    /// Merge additional variables into template variables
    pub fn merge_variables(&mut self, vars: HashMap<String, String>) {
        self.template_vars.extend(vars);
    }
}
