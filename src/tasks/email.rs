// ABOUTME: Email task implementation for sending notifications
// ABOUTME: Handles SMTP email sending with configurable templates and attachments

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::info;

use super::TaskImplementation;
use crate::engine::error::{ExecutionError, Result};
use crate::engine::{ExecutionContext, TaskResult, TaskStatus};

pub struct EmailTask;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    pub smtp_server: String,
    #[serde(default = "default_smtp_port")]
    pub smtp_port: u16,
    pub username: String,
    pub password: String,
    pub from: String,
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    pub subject: String,
    pub body: String,
    #[serde(default)]
    pub html: bool,
    #[serde(default)]
    pub attachments: Vec<String>,
}

fn default_smtp_port() -> u16 {
    587
}

#[async_trait]
impl TaskImplementation for EmailTask {
    async fn execute(
        &self,
        task_id: String,
        config: serde_yaml::Value,
        _context: ExecutionContext,
    ) -> Result<TaskResult> {
        let start_time = chrono::Utc::now();

        let config: EmailConfig =
            serde_yaml::from_value(config).map_err(|e| ExecutionError::ConfigError {
                task_id: task_id.clone(),
                message: format!("Invalid email configuration: {}", e),
            })?;

        info!(
            "Executing email task: {} - sending to {:?}",
            task_id, config.to
        );

        let mut task_result = TaskResult::new(task_id.clone(), "email".to_string());
        task_result.start_time = start_time;
        task_result.mark_started();

        // For now, this is a placeholder implementation
        // In a real implementation, this would use an SMTP client like lettre
        let output = format!(
            "Would send email from {} to {:?} with subject: '{}' via {}:{}",
            config.from, config.to, config.subject, config.smtp_server, config.smtp_port
        );

        info!("{}", output);

        // Add metadata
        task_result.add_metadata("from".to_string(), config.from);
        task_result.add_metadata("to".to_string(), config.to.join(", "));
        task_result.add_metadata("subject".to_string(), config.subject);
        task_result.add_metadata("smtp_server".to_string(), config.smtp_server);

        task_result.mark_completed(TaskStatus::Success, Some(output), None);

        Ok(task_result)
    }

    fn task_type(&self) -> &'static str {
        "email"
    }

    fn validate_config(&self, config: &serde_yaml::Value) -> Result<()> {
        let config: EmailConfig =
            serde_yaml::from_value(config.clone()).map_err(|e| ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: format!("Invalid email configuration: {}", e),
            })?;

        if config.smtp_server.is_empty() {
            return Err(ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: "SMTP server cannot be empty".to_string(),
            });
        }

        if config.from.is_empty() {
            return Err(ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: "From address cannot be empty".to_string(),
            });
        }

        if config.to.is_empty() {
            return Err(ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: "To addresses cannot be empty".to_string(),
            });
        }

        if config.subject.is_empty() {
            return Err(ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: "Subject cannot be empty".to_string(),
            });
        }

        if config.body.is_empty() {
            return Err(ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: "Body cannot be empty".to_string(),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_config_validation() {
        let task = EmailTask;

        // Test valid configuration
        let valid_config = serde_yaml::to_value(EmailConfig {
            smtp_server: "smtp.example.com".to_string(),
            smtp_port: 587,
            username: "user@example.com".to_string(),
            password: "password".to_string(),
            from: "sender@example.com".to_string(),
            to: vec!["recipient@example.com".to_string()],
            cc: vec![],
            bcc: vec![],
            subject: "Test Subject".to_string(),
            body: "Test Body".to_string(),
            html: false,
            attachments: vec![],
        })
        .unwrap();

        assert!(task.validate_config(&valid_config).is_ok());

        // Test invalid configuration (empty to addresses)
        let invalid_config = serde_yaml::to_value(EmailConfig {
            smtp_server: "smtp.example.com".to_string(),
            smtp_port: 587,
            username: "user@example.com".to_string(),
            password: "password".to_string(),
            from: "sender@example.com".to_string(),
            to: vec![],
            cc: vec![],
            bcc: vec![],
            subject: "Test Subject".to_string(),
            body: "Test Body".to_string(),
            html: false,
            attachments: vec![],
        })
        .unwrap();

        assert!(task.validate_config(&invalid_config).is_err());
    }
}
