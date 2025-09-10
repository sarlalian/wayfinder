// ABOUTME: Task configuration structures and parameter definitions
// ABOUTME: Defines all task types and their specific configuration parameters

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskConfig {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub task_type: TaskType,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default = "default_required")]
    pub required: bool,
    #[serde(alias = "retry")]
    pub retry_config: Option<RetryConfig>,
    pub when: Option<String>,
    #[serde(with = "humantime_serde", default)]
    pub timeout: Option<Duration>,
    pub config: serde_yaml::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    Command,
    Compress,
    Decompress,
    Checksum,
    ValidateChecksum,
    S3Upload,
    S3Download,
    Email,
    Slack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TaskParameters {
    Command(CommandConfig),
    Compress(CompressConfig),
    Decompress(DecompressConfig),
    Checksum(ChecksumConfig),
    ValidateChecksum(ValidateChecksumConfig),
    S3Upload(S3UploadConfig),
    S3Download(S3DownloadConfig),
    Email(EmailConfig),
    Slack(SlackConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,
    #[serde(with = "humantime_serde", default = "default_initial_delay")]
    pub initial_delay: Duration,
}

// Command task configuration - supports both simple commands and shell scripts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandConfig {
    /// Command to execute (for simple command mode). Mutually exclusive with `script`.
    #[serde(default)]
    pub command: Option<String>,

    /// Arguments for the command (only used with `command`, ignored in script mode)
    #[serde(default)]
    pub args: Vec<String>,

    /// Multi-line shell script (for script mode). Supports full template processing.
    /// Mutually exclusive with `command`.
    #[serde(default)]
    pub script: Option<String>,

    /// Shell interpreter to use for script execution (default: /bin/bash)
    #[serde(default = "default_shell")]
    pub shell: String,

    pub working_dir: Option<PathBuf>,
    pub env_file: Option<PathBuf>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_capture_output")]
    pub capture_output: bool,
    pub stdout_file: Option<PathBuf>,
    pub stderr_file: Option<PathBuf>,
    #[serde(with = "humantime_serde", default)]
    pub timeout: Option<Duration>,
    /// Expected exit codes (defaults to [0])
    #[serde(default = "default_expected_exit_codes")]
    pub expected_exit_codes: Vec<i32>,
    /// Timeout in seconds for task execution
    pub timeout_seconds: Option<u64>,
}

// Compression task configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressConfig {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub compression_type: CompressionType,
    pub compression_level: Option<u32>,
    #[serde(default)]
    pub preserve_original: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecompressConfig {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub compression_type: Option<CompressionType>,
    #[serde(default)]
    pub preserve_original: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompressionType {
    Bzip2,
    Xz,
    Lza,
}

// Checksum task configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecksumConfig {
    pub input_path: PathBuf,
    pub output_path: Option<PathBuf>,
    #[serde(default = "default_checksum_algorithm")]
    pub algorithm: ChecksumAlgorithm,
    #[serde(default)]
    pub base64_encode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateChecksumConfig {
    pub input_path: PathBuf,
    pub checksum_file: PathBuf,
    #[serde(default = "default_checksum_algorithm")]
    pub algorithm: ChecksumAlgorithm,
    #[serde(default)]
    pub base64_encoded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChecksumAlgorithm {
    Sha256,
    Sha512,
    Md5,
}

// S3 task configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3UploadConfig {
    pub local_path: PathBuf,
    pub bucket: String,
    pub key: String,
    pub acl: Option<String>,
    #[serde(default)]
    pub tags: HashMap<String, String>,
    pub content_type: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    #[serde(default)]
    pub server_side_encryption: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3DownloadConfig {
    pub bucket: String,
    pub key: String,
    pub local_path: PathBuf,
    #[serde(default)]
    pub create_dirs: bool,
    #[serde(default)]
    pub overwrite: bool,
}

// Email task configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    pub from: String,
    pub to: Vec<String>,
    pub cc: Option<Vec<String>>,
    pub bcc: Option<Vec<String>>,
    pub subject: String,
    pub body: String,
    pub html_body: Option<String>,
    pub attachments: Option<Vec<PathBuf>>,
    #[serde(default = "default_email_method")]
    pub method: EmailMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmailMethod {
    Smtp,
    Ses,
}

// Slack task configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    pub channel: Option<String>,
    pub message: String,
    pub username: Option<String>,
    pub icon_emoji: Option<String>,
    pub icon_url: Option<String>,
    #[serde(default)]
    pub attachments: Vec<SlackAttachment>,
    #[serde(default = "default_slack_method")]
    pub method: SlackMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackAttachment {
    pub color: Option<String>,
    pub title: Option<String>,
    pub text: Option<String>,
    pub fields: Option<Vec<SlackField>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackField {
    pub title: String,
    pub value: String,
    #[serde(default)]
    pub short: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SlackMethod {
    Webhook,
    BotToken,
}

// Default value functions
fn default_required() -> bool {
    true
}

fn default_max_attempts() -> u32 {
    1
}

fn default_backoff_multiplier() -> f64 {
    2.0
}

fn default_initial_delay() -> Duration {
    Duration::from_secs(1)
}

fn default_checksum_algorithm() -> ChecksumAlgorithm {
    ChecksumAlgorithm::Sha256
}

fn default_email_method() -> EmailMethod {
    EmailMethod::Smtp
}

fn default_slack_method() -> SlackMethod {
    SlackMethod::Webhook
}

fn default_capture_output() -> bool {
    true
}

fn default_shell() -> String {
    "/bin/bash".to_string()
}

fn default_expected_exit_codes() -> Vec<i32> {
    vec![0]
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: default_max_attempts(),
            backoff_multiplier: default_backoff_multiplier(),
            initial_delay: default_initial_delay(),
        }
    }
}

impl TaskType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskType::Command => "command",
            TaskType::Compress => "compress",
            TaskType::Decompress => "decompress",
            TaskType::Checksum => "checksum",
            TaskType::ValidateChecksum => "validate_checksum",
            TaskType::S3Upload => "s3_upload",
            TaskType::S3Download => "s3_download",
            TaskType::Email => "email",
            TaskType::Slack => "slack",
        }
    }
}

impl std::fmt::Display for TaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_type_serialization() {
        let task_type = TaskType::Command;
        let serialized = serde_yaml::to_string(&task_type).unwrap();
        assert!(serialized.contains("command"));
    }

    #[test]
    fn test_retry_config_defaults() {
        let retry = RetryConfig::default();
        assert_eq!(retry.max_attempts, 1);
        assert_eq!(retry.backoff_multiplier, 2.0);
        assert_eq!(retry.initial_delay, Duration::from_secs(1));
    }
}
