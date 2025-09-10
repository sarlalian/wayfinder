// ABOUTME: Configuration structures for the reporting engine
// ABOUTME: Defines report templates, delivery channels, and reporting settings

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::ReportPeriod;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportingConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub workflow_completion_reports: bool,
    #[serde(default)]
    pub task_completion_reports: bool,
    #[serde(default)]
    pub report_successful_tasks: bool,
    #[serde(default)]
    pub periodic_reports: bool,
    #[serde(default)]
    pub templates: Vec<ReportTemplate>,
    #[serde(default)]
    pub global_variables: HashMap<String, String>,
    #[serde(default)]
    pub retention_days: Option<u32>,
    #[serde(default)]
    pub rate_limits: RateLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportTemplate {
    pub name: String,
    pub generator_type: String,
    pub template_content: Option<String>,
    pub template_file: Option<String>,
    #[serde(default)]
    pub trigger_on_workflow_completion: bool,
    #[serde(default)]
    pub trigger_on_task_failure: bool,
    #[serde(default)]
    pub periodic_schedule: Option<PeriodicSchedule>,
    #[serde(default)]
    pub delivery_channels: Vec<DeliveryChannel>,
    #[serde(default)]
    pub variables: HashMap<String, String>,
    #[serde(default)]
    pub filters: ReportFilters,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryChannel {
    pub channel_type: String,
    #[serde(default)]
    pub config: HashMap<String, serde_yaml::Value>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub retry_config: Option<RetryConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodicSchedule {
    pub period: ReportPeriod,
    #[serde(default)]
    pub time_of_day: Option<String>, // Format: "14:30" (24-hour)
    #[serde(default)]
    pub day_of_week: Option<u8>, // 0 = Sunday, 6 = Saturday
    #[serde(default)]
    pub day_of_month: Option<u8>, // 1-31
    #[serde(default)]
    pub timezone: Option<String>, // IANA timezone identifier
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReportFilters {
    #[serde(default)]
    pub workflow_status: Option<Vec<String>>,
    #[serde(default)]
    pub task_status: Option<Vec<String>>,
    #[serde(default)]
    pub workflow_names: Option<Vec<String>>,
    #[serde(default)]
    pub task_types: Option<Vec<String>>,
    #[serde(default)]
    pub min_duration_seconds: Option<u64>,
    #[serde(default)]
    pub max_duration_seconds: Option<u64>,
    #[serde(default)]
    pub tags: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    #[serde(default = "default_retry_attempts")]
    pub max_attempts: u32,
    #[serde(default = "default_retry_delay")]
    pub initial_delay_seconds: u64,
    #[serde(default = "default_retry_multiplier")]
    pub backoff_multiplier: f64,
    #[serde(default = "default_retry_max_delay")]
    pub max_delay_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RateLimits {
    #[serde(default)]
    pub reports_per_minute: Option<u32>,
    #[serde(default)]
    pub reports_per_hour: Option<u32>,
    #[serde(default)]
    pub reports_per_day: Option<u32>,
    #[serde(default)]
    pub per_channel_limits: HashMap<String, ChannelRateLimit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelRateLimit {
    #[serde(default)]
    pub reports_per_minute: Option<u32>,
    #[serde(default)]
    pub reports_per_hour: Option<u32>,
    #[serde(default)]
    pub reports_per_day: Option<u32>,
}

// Email delivery channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailDeliveryConfig {
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub subject_template: Option<String>,
    #[serde(default)]
    pub smtp_server: Option<String>,
    #[serde(default)]
    pub smtp_port: Option<u16>,
    #[serde(default)]
    pub smtp_username: Option<String>,
    #[serde(default)]
    pub smtp_password: Option<String>,
    #[serde(default)]
    pub use_tls: bool,
}

// Slack delivery channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackDeliveryConfig {
    pub webhook_url: String,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub icon_emoji: Option<String>,
    #[serde(default)]
    pub message_template: Option<String>,
}

// File delivery channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDeliveryConfig {
    pub path: String,
    #[serde(default)]
    pub filename_template: Option<String>,
    #[serde(default = "default_true")]
    pub create_directories: bool,
    #[serde(default)]
    pub append_mode: bool,
    #[serde(default)]
    pub compress: bool,
    #[serde(default)]
    pub rotation: Option<FileRotationConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRotationConfig {
    #[serde(default)]
    pub max_size_mb: Option<u64>,
    #[serde(default)]
    pub max_age_days: Option<u32>,
    #[serde(default)]
    pub max_files: Option<u32>,
}

// Terminal delivery channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalDeliveryConfig {
    #[serde(default = "default_true")]
    pub colored: bool,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub timestamp: bool,
}

fn default_true() -> bool {
    true
}

fn default_retry_attempts() -> u32 {
    3
}

fn default_retry_delay() -> u64 {
    5
}

fn default_retry_multiplier() -> f64 {
    2.0
}

fn default_retry_max_delay() -> u64 {
    300
}

impl Default for ReportingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            workflow_completion_reports: false,
            task_completion_reports: false,
            report_successful_tasks: false,
            periodic_reports: false,
            templates: Vec::new(),
            global_variables: HashMap::new(),
            retention_days: Some(30),
            rate_limits: RateLimits::default(),
        }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: default_retry_attempts(),
            initial_delay_seconds: default_retry_delay(),
            backoff_multiplier: default_retry_multiplier(),
            max_delay_seconds: default_retry_max_delay(),
        }
    }
}

impl ReportTemplate {
    pub fn new(name: String, generator_type: String) -> Self {
        Self {
            name,
            generator_type,
            template_content: None,
            template_file: None,
            trigger_on_workflow_completion: false,
            trigger_on_task_failure: false,
            periodic_schedule: None,
            delivery_channels: Vec::new(),
            variables: HashMap::new(),
            filters: ReportFilters::default(),
            enabled: true,
        }
    }

    pub fn matches_period(&self, period: &ReportPeriod) -> bool {
        if let Some(schedule) = &self.periodic_schedule {
            schedule.period == *period
        } else {
            false
        }
    }

    pub fn should_trigger_for_workflow(&self, workflow_name: &str, status: &str) -> bool {
        if !self.enabled || !self.trigger_on_workflow_completion {
            return false;
        }

        // Check workflow name filter
        if let Some(ref names) = self.filters.workflow_names {
            if !names.contains(&workflow_name.to_string()) {
                return false;
            }
        }

        // Check status filter
        if let Some(ref statuses) = self.filters.workflow_status {
            if !statuses.contains(&status.to_string()) {
                return false;
            }
        }

        true
    }

    pub fn should_trigger_for_task(&self, task_type: &str, status: &str) -> bool {
        if !self.enabled || !self.trigger_on_task_failure {
            return false;
        }

        // Only trigger on failures for task completion
        if !matches!(status, "Failed" | "Timeout" | "Cancelled") {
            return false;
        }

        // Check task type filter
        if let Some(ref types) = self.filters.task_types {
            if !types.contains(&task_type.to_string()) {
                return false;
            }
        }

        // Check status filter
        if let Some(ref statuses) = self.filters.task_status {
            if !statuses.contains(&status.to_string()) {
                return false;
            }
        }

        true
    }
}

impl DeliveryChannel {
    pub fn new_email(to: Vec<String>) -> Self {
        let mut config = HashMap::new();
        config.insert(
            "to".to_string(),
            serde_yaml::Value::Sequence(to.into_iter().map(serde_yaml::Value::String).collect()),
        );

        Self {
            channel_type: "email".to_string(),
            config,
            enabled: true,
            retry_config: Some(RetryConfig::default()),
        }
    }

    pub fn new_slack(webhook_url: String) -> Self {
        let mut config = HashMap::new();
        config.insert(
            "webhook_url".to_string(),
            serde_yaml::Value::String(webhook_url),
        );

        Self {
            channel_type: "slack".to_string(),
            config,
            enabled: true,
            retry_config: Some(RetryConfig::default()),
        }
    }

    pub fn new_file(path: String) -> Self {
        let mut config = HashMap::new();
        config.insert("path".to_string(), serde_yaml::Value::String(path));

        Self {
            channel_type: "file".to_string(),
            config,
            enabled: true,
            retry_config: None,
        }
    }

    pub fn new_terminal() -> Self {
        Self {
            channel_type: "terminal".to_string(),
            config: HashMap::new(),
            enabled: true,
            retry_config: None,
        }
    }

    pub fn get_config<T>(&self) -> Result<T, crate::reporting::error::ReportingError>
    where
        T: serde::de::DeserializeOwned,
    {
        let config_value = serde_yaml::Value::Mapping(
            self.config
                .iter()
                .map(|(k, v)| (serde_yaml::Value::String(k.clone()), v.clone()))
                .collect(),
        );

        serde_yaml::from_value(config_value).map_err(|e| {
            crate::reporting::error::ReportingError::ConfigError {
                message: format!("Failed to parse delivery channel config: {}", e),
            }
        })
    }
}

impl PeriodicSchedule {
    pub fn hourly() -> Self {
        Self {
            period: ReportPeriod::Hourly,
            time_of_day: None,
            day_of_week: None,
            day_of_month: None,
            timezone: None,
        }
    }

    pub fn daily_at(time: &str) -> Self {
        Self {
            period: ReportPeriod::Daily,
            time_of_day: Some(time.to_string()),
            day_of_week: None,
            day_of_month: None,
            timezone: None,
        }
    }

    pub fn weekly_on(day: u8, time: &str) -> Self {
        Self {
            period: ReportPeriod::Weekly,
            time_of_day: Some(time.to_string()),
            day_of_week: Some(day),
            day_of_month: None,
            timezone: None,
        }
    }

    pub fn monthly_on(day: u8, time: &str) -> Self {
        Self {
            period: ReportPeriod::Monthly,
            time_of_day: Some(time.to_string()),
            day_of_week: None,
            day_of_month: Some(day),
            timezone: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_template_creation() {
        let template = ReportTemplate::new("daily_summary".to_string(), "summary".to_string());
        assert_eq!(template.name, "daily_summary");
        assert_eq!(template.generator_type, "summary");
        assert!(template.enabled);
    }

    #[test]
    fn test_delivery_channel_constructors() {
        let email = DeliveryChannel::new_email(vec!["admin@example.com".to_string()]);
        assert_eq!(email.channel_type, "email");
        assert!(email.enabled);

        let slack = DeliveryChannel::new_slack("https://hooks.slack.com/test".to_string());
        assert_eq!(slack.channel_type, "slack");

        let file = DeliveryChannel::new_file("/tmp/reports.json".to_string());
        assert_eq!(file.channel_type, "file");

        let terminal = DeliveryChannel::new_terminal();
        assert_eq!(terminal.channel_type, "terminal");
    }

    #[test]
    fn test_periodic_schedule_constructors() {
        let hourly = PeriodicSchedule::hourly();
        assert!(matches!(hourly.period, ReportPeriod::Hourly));

        let daily = PeriodicSchedule::daily_at("09:00");
        assert!(matches!(daily.period, ReportPeriod::Daily));
        assert_eq!(daily.time_of_day, Some("09:00".to_string()));

        let weekly = PeriodicSchedule::weekly_on(1, "10:00"); // Monday at 10:00
        assert!(matches!(weekly.period, ReportPeriod::Weekly));
        assert_eq!(weekly.day_of_week, Some(1));

        let monthly = PeriodicSchedule::monthly_on(15, "12:00"); // 15th at noon
        assert!(matches!(monthly.period, ReportPeriod::Monthly));
        assert_eq!(monthly.day_of_month, Some(15));
    }

    #[test]
    fn test_template_filtering() {
        let mut template = ReportTemplate::new("test".to_string(), "summary".to_string());
        template.trigger_on_workflow_completion = true;
        template.filters.workflow_names = Some(vec!["important_workflow".to_string()]);

        assert!(template.should_trigger_for_workflow("important_workflow", "Success"));
        assert!(!template.should_trigger_for_workflow("other_workflow", "Success"));
    }

    #[test]
    fn test_task_failure_filtering() {
        let mut template = ReportTemplate::new("failures".to_string(), "detailed".to_string());
        template.trigger_on_task_failure = true;

        assert!(template.should_trigger_for_task("command", "Failed"));
        assert!(template.should_trigger_for_task("http", "Timeout"));
        assert!(!template.should_trigger_for_task("command", "Success"));
    }

    #[test]
    fn test_config_serialization() {
        let config = ReportingConfig::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let parsed: ReportingConfig = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(config.enabled, parsed.enabled);
        assert_eq!(config.retention_days, parsed.retention_days);
    }
}
