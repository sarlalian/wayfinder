// ABOUTME: Slack task implementation for sending notifications
// ABOUTME: Handles Slack webhook and API-based message sending with formatting options

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::info;

use super::TaskImplementation;
use crate::engine::error::{ExecutionError, Result};
use crate::engine::{ExecutionContext, TaskResult, TaskStatus};

pub struct SlackTask;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    pub webhook_url: String,
    pub channel: Option<String>,
    pub username: Option<String>,
    pub icon_emoji: Option<String>,
    pub icon_url: Option<String>,
    pub text: String,
    #[serde(default)]
    pub attachments: Vec<SlackAttachment>,
    #[serde(default)]
    pub blocks: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackAttachment {
    pub color: Option<String>,
    pub title: Option<String>,
    pub title_link: Option<String>,
    pub text: Option<String>,
    pub fields: Vec<SlackField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackField {
    pub title: String,
    pub value: String,
    #[serde(default)]
    pub short: bool,
}

#[async_trait]
impl TaskImplementation for SlackTask {
    async fn execute(
        &self,
        task_id: String,
        config: serde_yaml::Value,
        _context: ExecutionContext,
    ) -> Result<TaskResult> {
        let start_time = chrono::Utc::now();

        let config: SlackConfig =
            serde_yaml::from_value(config).map_err(|e| ExecutionError::ConfigError {
                task_id: task_id.clone(),
                message: format!("Invalid Slack configuration: {}", e),
            })?;

        info!(
            "Executing Slack task: {} - sending message to {}",
            task_id,
            config.channel.as_deref().unwrap_or("default")
        );

        let mut task_result = TaskResult::new(task_id.clone(), "slack".to_string());
        task_result.start_time = start_time;
        task_result.mark_started();

        // For now, this is a placeholder implementation
        // In a real implementation, this would use an HTTP client to send to Slack
        let output = format!(
            "Would send Slack message to {} with text: '{}'",
            config.channel.as_deref().unwrap_or("webhook default"),
            config.text
        );

        info!("{}", output);

        // Add metadata
        if let Some(channel) = &config.channel {
            task_result.add_metadata("channel".to_string(), channel.clone());
        }
        if let Some(username) = &config.username {
            task_result.add_metadata("username".to_string(), username.clone());
        }
        task_result.add_metadata("text".to_string(), config.text.clone());
        task_result.add_metadata(
            "attachments_count".to_string(),
            config.attachments.len().to_string(),
        );

        task_result.mark_completed(TaskStatus::Success, Some(output), None);

        Ok(task_result)
    }

    fn task_type(&self) -> &'static str {
        "slack"
    }

    fn validate_config(&self, config: &serde_yaml::Value) -> Result<()> {
        let config: SlackConfig =
            serde_yaml::from_value(config.clone()).map_err(|e| ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: format!("Invalid Slack configuration: {}", e),
            })?;

        if config.webhook_url.is_empty() {
            return Err(ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: "Slack webhook URL cannot be empty".to_string(),
            });
        }

        if !config.webhook_url.starts_with("https://hooks.slack.com/") {
            return Err(ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: "Invalid Slack webhook URL format".to_string(),
            });
        }

        if config.text.is_empty() && config.attachments.is_empty() && config.blocks.is_empty() {
            return Err(ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: "At least one of text, attachments, or blocks must be provided"
                    .to_string(),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slack_config_validation() {
        let task = SlackTask;

        // Test valid configuration
        let valid_config = serde_yaml::to_value(SlackConfig {
            webhook_url:
                "https://hooks.slack.com/services/T00000000/B00000000/XXXXXXXXXXXXXXXXXXXXXXXX"
                    .to_string(),
            channel: Some("#general".to_string()),
            username: Some("Wayfinder Bot".to_string()),
            icon_emoji: Some(":robot_face:".to_string()),
            icon_url: None,
            text: "Hello, World!".to_string(),
            attachments: vec![],
            blocks: vec![],
        })
        .unwrap();

        assert!(task.validate_config(&valid_config).is_ok());

        // Test invalid webhook URL
        let invalid_url_config = serde_yaml::to_value(SlackConfig {
            webhook_url: "https://invalid.url/webhook".to_string(),
            channel: Some("#general".to_string()),
            username: Some("Wayfinder Bot".to_string()),
            icon_emoji: Some(":robot_face:".to_string()),
            icon_url: None,
            text: "Hello, World!".to_string(),
            attachments: vec![],
            blocks: vec![],
        })
        .unwrap();

        assert!(task.validate_config(&invalid_url_config).is_err());

        // Test empty content
        let empty_content_config = serde_yaml::to_value(SlackConfig {
            webhook_url:
                "https://hooks.slack.com/services/T00000000/B00000000/XXXXXXXXXXXXXXXXXXXXXXXX"
                    .to_string(),
            channel: Some("#general".to_string()),
            username: Some("Wayfinder Bot".to_string()),
            icon_emoji: Some(":robot_face:".to_string()),
            icon_url: None,
            text: "".to_string(),
            attachments: vec![],
            blocks: vec![],
        })
        .unwrap();

        assert!(task.validate_config(&empty_content_config).is_err());
    }
}
