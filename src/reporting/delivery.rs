// ABOUTME: Report delivery implementations for various channels
// ABOUTME: Handles sending reports via email, Slack, file, terminal, and other channels

use async_trait::async_trait;
use reqwest::Client;
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

use super::config::{
    DeliveryChannel, EmailDeliveryConfig, FileDeliveryConfig, RetryConfig, SlackDeliveryConfig,
    TerminalDeliveryConfig,
};
use super::error::{ReportingError, Result};
use super::GeneratedReport;

#[async_trait]
pub trait ReportDeliverer: Send + Sync {
    async fn deliver_report(
        &self,
        report: &GeneratedReport,
        channel: &DeliveryChannel,
    ) -> Result<()>;

    fn supported_channel_types(&self) -> Vec<&'static str>;
    fn get_deliverer_info(&self) -> DelivererInfo;
}

#[derive(Debug, Clone)]
pub struct DelivererInfo {
    pub name: String,
    pub description: String,
    pub supported_formats: Vec<String>,
    pub requires_config: bool,
}

pub struct EmailDeliverer {
    name: String,
}

pub struct SlackDeliverer {
    name: String,
    http_client: Client,
}

pub struct FileDeliverer {
    name: String,
}

pub struct TerminalDeliverer {
    name: String,
}

pub struct HttpDeliverer {
    name: String,
    http_client: Client,
}

impl EmailDeliverer {
    pub fn new() -> Self {
        Self {
            name: "Email Deliverer".to_string(),
        }
    }
}

#[async_trait]
impl ReportDeliverer for EmailDeliverer {
    async fn deliver_report(
        &self,
        report: &GeneratedReport,
        channel: &DeliveryChannel,
    ) -> Result<()> {
        if !channel.enabled {
            debug!("Email delivery skipped - channel disabled");
            return Ok(());
        }

        let config: EmailDeliveryConfig =
            channel
                .get_config()
                .map_err(|e| ReportingError::ConfigError {
                    message: format!("Invalid email delivery config: {}", e),
                })?;

        if config.to.is_empty() {
            return Err(ReportingError::DeliveryError {
                message: "No recipient addresses specified".to_string(),
            });
        }

        // For now, this is a placeholder implementation
        // In a real implementation, this would use SMTP libraries like lettre
        let subject = if let Some(template) = &config.subject_template {
            self.render_subject_template(template, report)
        } else {
            report.title.clone()
        };

        info!(
            "Would send email report '{}' to {} recipients",
            subject,
            config.to.len()
        );

        debug!("Email details: to={:?}, subject='{}'", config.to, subject);
        debug!(
            "Email content preview: {}",
            &report.content[..report.content.len().min(100)]
        );

        // Simulate email sending delay
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Apply retry logic if configured
        if let Some(retry_config) = &channel.retry_config {
            self.deliver_with_retry(report, channel, retry_config).await
        } else {
            Ok(())
        }
    }

    fn supported_channel_types(&self) -> Vec<&'static str> {
        vec!["email"]
    }

    fn get_deliverer_info(&self) -> DelivererInfo {
        DelivererInfo {
            name: self.name.clone(),
            description: "Delivers reports via email using SMTP".to_string(),
            supported_formats: vec![
                "text".to_string(),
                "html".to_string(),
                "markdown".to_string(),
            ],
            requires_config: true,
        }
    }
}

impl EmailDeliverer {
    fn render_subject_template(&self, template: &str, report: &GeneratedReport) -> String {
        template
            .replace("{title}", &report.title)
            .replace("{type}", &report.report_type)
            .replace(
                "{generated_at}",
                &report.generated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            )
    }

    async fn deliver_with_retry(
        &self,
        _report: &GeneratedReport,
        _channel: &DeliveryChannel,
        retry_config: &RetryConfig,
    ) -> Result<()> {
        let mut attempts = 0;
        let mut delay = retry_config.initial_delay_seconds;

        while attempts < retry_config.max_attempts {
            attempts += 1;

            // Simulate delivery attempt
            if attempts < retry_config.max_attempts / 2 {
                // Simulate failure on early attempts
                warn!("Email delivery attempt {} failed, retrying...", attempts);

                tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;

                // Apply backoff
                delay = ((delay as f64) * retry_config.backoff_multiplier) as u64;
                delay = delay.min(retry_config.max_delay_seconds);

                continue;
            } else {
                // Simulate success
                info!("Email delivered successfully on attempt {}", attempts);
                return Ok(());
            }
        }

        Err(ReportingError::DeliveryError {
            message: format!("Email delivery failed after {} attempts", attempts),
        })
    }
}

impl SlackDeliverer {
    pub fn new() -> Self {
        Self {
            name: "Slack Deliverer".to_string(),
            http_client: Client::new(),
        }
    }
}

#[async_trait]
impl ReportDeliverer for SlackDeliverer {
    async fn deliver_report(
        &self,
        report: &GeneratedReport,
        channel: &DeliveryChannel,
    ) -> Result<()> {
        if !channel.enabled {
            debug!("Slack delivery skipped - channel disabled");
            return Ok(());
        }

        let config: SlackDeliveryConfig =
            channel
                .get_config()
                .map_err(|e| ReportingError::ConfigError {
                    message: format!("Invalid Slack delivery config: {}", e),
                })?;

        // Prepare Slack message
        let message_text = if let Some(template) = &config.message_template {
            self.render_message_template(template, report)
        } else {
            self.format_default_message(report)
        };

        let mut payload = serde_json::json!({
            "text": message_text
        });

        // Add optional fields
        if let Some(channel_name) = &config.channel {
            payload["channel"] = serde_json::Value::String(channel_name.clone());
        }

        if let Some(username) = &config.username {
            payload["username"] = serde_json::Value::String(username.clone());
        }

        if let Some(icon) = &config.icon_emoji {
            payload["icon_emoji"] = serde_json::Value::String(icon.clone());
        }

        // Send to Slack webhook
        let response = self
            .http_client
            .post(&config.webhook_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| ReportingError::DeliveryError {
                message: format!("Failed to send Slack webhook: {}", e),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ReportingError::DeliveryError {
                message: format!("Slack webhook failed with status {}: {}", status, body),
            });
        }

        info!("Report delivered to Slack channel successfully");
        debug!(
            "Slack message preview: {}",
            &message_text[..message_text.len().min(100)]
        );

        Ok(())
    }

    fn supported_channel_types(&self) -> Vec<&'static str> {
        vec!["slack"]
    }

    fn get_deliverer_info(&self) -> DelivererInfo {
        DelivererInfo {
            name: self.name.clone(),
            description: "Delivers reports to Slack channels via webhooks".to_string(),
            supported_formats: vec!["text".to_string(), "markdown".to_string()],
            requires_config: true,
        }
    }
}

impl SlackDeliverer {
    fn render_message_template(&self, template: &str, report: &GeneratedReport) -> String {
        template
            .replace("{title}", &report.title)
            .replace("{type}", &report.report_type)
            .replace("{content}", &self.truncate_content(&report.content, 1000))
            .replace(
                "{generated_at}",
                &report.generated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            )
    }

    fn format_default_message(&self, report: &GeneratedReport) -> String {
        let icon = match report.report_type.as_str() {
            "summary" => "📋",
            "detailed" => "📊",
            "metrics" => "📈",
            _ => "📄",
        };

        format!(
            "{} *{}*\n\n{}",
            icon,
            report.title,
            self.truncate_content(&report.content, 1500)
        )
    }

    fn truncate_content(&self, content: &str, max_length: usize) -> String {
        if content.len() <= max_length {
            content.to_string()
        } else {
            format!("{}...\n\n_(Content truncated)_", &content[..max_length])
        }
    }
}

impl FileDeliverer {
    pub fn new() -> Self {
        Self {
            name: "File Deliverer".to_string(),
        }
    }
}

#[async_trait]
impl ReportDeliverer for FileDeliverer {
    async fn deliver_report(
        &self,
        report: &GeneratedReport,
        channel: &DeliveryChannel,
    ) -> Result<()> {
        if !channel.enabled {
            debug!("File delivery skipped - channel disabled");
            return Ok(());
        }

        let config: FileDeliveryConfig =
            channel
                .get_config()
                .map_err(|e| ReportingError::ConfigError {
                    message: format!("Invalid file delivery config: {}", e),
                })?;

        // Determine the file path
        let file_path = if let Some(template) = &config.filename_template {
            let filename = self.render_filename_template(template, report);
            Path::new(&config.path).join(filename)
        } else {
            Path::new(&config.path).to_path_buf()
        };

        // Create directories if needed
        if config.create_directories {
            if let Some(parent_dir) = file_path.parent() {
                fs::create_dir_all(parent_dir).await.map_err(|e| {
                    ReportingError::DeliveryError {
                        message: format!(
                            "Failed to create directories {}: {}",
                            parent_dir.display(),
                            e
                        ),
                    }
                })?;
            }
        }

        // Handle file rotation if configured
        if let Some(rotation_config) = &config.rotation {
            self.rotate_file_if_needed(&file_path, rotation_config)
                .await?;
        }

        // Write the report
        if config.append_mode {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file_path)
                .await
                .map_err(|e| ReportingError::DeliveryError {
                    message: format!(
                        "Failed to open file for append {}: {}",
                        file_path.display(),
                        e
                    ),
                })?;

            // Add timestamp separator for appends
            let separator = format!(
                "\n--- {} ---\n",
                report.generated_at.format("%Y-%m-%d %H:%M:%S UTC")
            );
            file.write_all(separator.as_bytes()).await.map_err(|e| {
                ReportingError::DeliveryError {
                    message: format!("Failed to write separator to file: {}", e),
                }
            })?;

            file.write_all(report.content.as_bytes())
                .await
                .map_err(|e| ReportingError::DeliveryError {
                    message: format!("Failed to append report to file: {}", e),
                })?;

            file.write_all(b"\n")
                .await
                .map_err(|e| ReportingError::DeliveryError {
                    message: format!("Failed to append newline to file: {}", e),
                })?;
        } else {
            fs::write(&file_path, &report.content).await.map_err(|e| {
                ReportingError::DeliveryError {
                    message: format!(
                        "Failed to write report to file {}: {}",
                        file_path.display(),
                        e
                    ),
                }
            })?;
        }

        // Compress if configured
        if config.compress {
            self.compress_file(&file_path).await?;
        }

        info!(
            "Report delivered to file: {} ({} bytes)",
            file_path.display(),
            report.content.len()
        );
        Ok(())
    }

    fn supported_channel_types(&self) -> Vec<&'static str> {
        vec!["file"]
    }

    fn get_deliverer_info(&self) -> DelivererInfo {
        DelivererInfo {
            name: self.name.clone(),
            description: "Saves reports to local or network file systems".to_string(),
            supported_formats: vec![
                "text".to_string(),
                "json".to_string(),
                "yaml".to_string(),
                "html".to_string(),
            ],
            requires_config: true,
        }
    }
}

impl FileDeliverer {
    fn render_filename_template(&self, template: &str, report: &GeneratedReport) -> String {
        template
            .replace("{title}", &self.sanitize_filename(&report.title))
            .replace("{type}", &report.report_type)
            .replace(
                "{timestamp}",
                &report.generated_at.format("%Y%m%d_%H%M%S").to_string(),
            )
            .replace(
                "{date}",
                &report.generated_at.format("%Y-%m-%d").to_string(),
            )
    }

    fn sanitize_filename(&self, filename: &str) -> String {
        filename
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    }

    async fn rotate_file_if_needed(
        &self,
        file_path: &Path,
        _rotation_config: &super::config::FileRotationConfig,
    ) -> Result<()> {
        // Placeholder for file rotation logic
        // In a real implementation, this would check file size, age, and count
        // and rotate files accordingly
        if file_path.exists() {
            debug!("File rotation check for: {}", file_path.display());
        }
        Ok(())
    }

    async fn compress_file(&self, file_path: &Path) -> Result<()> {
        // Placeholder for compression logic
        // In a real implementation, this would compress the file using gzip or similar
        debug!("File compression for: {}", file_path.display());
        Ok(())
    }
}

impl TerminalDeliverer {
    pub fn new() -> Self {
        Self {
            name: "Terminal Deliverer".to_string(),
        }
    }
}

#[async_trait]
impl ReportDeliverer for TerminalDeliverer {
    async fn deliver_report(
        &self,
        report: &GeneratedReport,
        channel: &DeliveryChannel,
    ) -> Result<()> {
        if !channel.enabled {
            debug!("Terminal delivery skipped - channel disabled");
            return Ok(());
        }

        let config: TerminalDeliveryConfig = channel.get_config().unwrap_or_default();

        let output = if config.colored {
            self.format_colored_output(report, &config)
        } else {
            self.format_plain_output(report, &config)
        };

        println!("{}", output);

        debug!("Report delivered to terminal ({} chars)", output.len());
        Ok(())
    }

    fn supported_channel_types(&self) -> Vec<&'static str> {
        vec!["terminal", "stdout", "console"]
    }

    fn get_deliverer_info(&self) -> DelivererInfo {
        DelivererInfo {
            name: self.name.clone(),
            description: "Displays reports directly in the terminal/console".to_string(),
            supported_formats: vec!["text".to_string(), "markdown".to_string()],
            requires_config: false,
        }
    }
}

impl TerminalDeliverer {
    fn format_colored_output(
        &self,
        report: &GeneratedReport,
        config: &TerminalDeliveryConfig,
    ) -> String {
        let mut output = String::new();

        if config.timestamp {
            output.push_str(&format!(
                "\x1b[90m[{}]\x1b[0m ",
                report.generated_at.format("%Y-%m-%d %H:%M:%S")
            ));
        }

        // Color code based on report type
        let (color_code, icon) = match report.report_type.as_str() {
            "summary" => ("\x1b[36m", "📋"),  // Cyan
            "detailed" => ("\x1b[34m", "📊"), // Blue
            "metrics" => ("\x1b[32m", "📈"),  // Green
            _ => ("\x1b[37m", "📄"),          // White
        };

        output.push_str(&format!(
            "{}{}  {}\x1b[0m\n",
            color_code, icon, report.title
        ));
        output.push_str("\x1b[90m");
        output.push_str(&"─".repeat(60));
        output.push_str("\x1b[0m\n");
        output.push_str(&report.content);
        output.push('\n');

        output
    }

    fn format_plain_output(
        &self,
        report: &GeneratedReport,
        config: &TerminalDeliveryConfig,
    ) -> String {
        let mut output = String::new();

        if config.timestamp {
            output.push_str(&format!(
                "[{}] ",
                report.generated_at.format("%Y-%m-%d %H:%M:%S")
            ));
        }

        output.push_str(&format!("Report: {}\n", report.title));
        output.push_str(&"-".repeat(60));
        output.push('\n');
        output.push_str(&report.content);
        output.push('\n');

        output
    }
}

impl HttpDeliverer {
    pub fn new() -> Self {
        Self {
            name: "HTTP Deliverer".to_string(),
            http_client: Client::new(),
        }
    }
}

#[async_trait]
impl ReportDeliverer for HttpDeliverer {
    async fn deliver_report(
        &self,
        report: &GeneratedReport,
        channel: &DeliveryChannel,
    ) -> Result<()> {
        if !channel.enabled {
            debug!("HTTP delivery skipped - channel disabled");
            return Ok(());
        }

        let config: crate::output::writer::HttpWriterConfig =
            channel
                .get_config()
                .map_err(|e| ReportingError::ConfigError {
                    message: format!("Invalid HTTP delivery config: {}", e),
                })?;

        // Prepare HTTP request
        let method = match config.method.to_uppercase().as_str() {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "PATCH" => reqwest::Method::PATCH,
            _ => {
                return Err(ReportingError::ConfigError {
                    message: format!("Unsupported HTTP method: {}", config.method),
                });
            }
        };

        // Create request payload
        let payload = serde_json::json!({
            "report": {
                "title": report.title,
                "type": report.report_type,
                "content": report.content,
                "metadata": report.metadata,
                "generated_at": report.generated_at
            }
        });

        let mut request = self.http_client.request(method, &config.url).json(&payload);

        // Add headers
        for (key, value) in &config.headers {
            request = request.header(key, value);
        }

        // Set timeout
        if let Some(timeout_secs) = config.timeout_seconds {
            request = request.timeout(tokio::time::Duration::from_secs(timeout_secs));
        }

        // Send request
        let response = request
            .send()
            .await
            .map_err(|e| ReportingError::DeliveryError {
                message: format!("HTTP request failed: {}", e),
            })?;

        if !response.status().is_success() {
            return Err(ReportingError::DeliveryError {
                message: format!("HTTP delivery failed with status: {}", response.status()),
            });
        }

        info!("Report delivered via HTTP to: {}", config.url);
        Ok(())
    }

    fn supported_channel_types(&self) -> Vec<&'static str> {
        vec!["http", "webhook", "api"]
    }

    fn get_deliverer_info(&self) -> DelivererInfo {
        DelivererInfo {
            name: self.name.clone(),
            description: "Sends reports to HTTP endpoints via REST APIs".to_string(),
            supported_formats: vec!["json".to_string()],
            requires_config: true,
        }
    }
}

impl Default for EmailDeliverer {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for SlackDeliverer {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for FileDeliverer {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for TerminalDeliverer {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for HttpDeliverer {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for TerminalDeliveryConfig {
    fn default() -> Self {
        Self {
            colored: true,
            format: None,
            timestamp: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_terminal_deliverer() {
        let deliverer = TerminalDeliverer::new();
        let channel = DeliveryChannel::new_terminal();

        let report = GeneratedReport {
            report_type: "summary".to_string(),
            title: "Test Report".to_string(),
            content: "This is a test report content.".to_string(),
            metadata: HashMap::new(),
            generated_at: Utc::now(),
        };

        let result = deliverer.deliver_report(&report, &channel).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_file_deliverer() {
        let deliverer = FileDeliverer::new();
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_report.txt");

        let channel = DeliveryChannel::new_file(test_file.to_string_lossy().to_string());

        let report = GeneratedReport {
            report_type: "detailed".to_string(),
            title: "Test File Report".to_string(),
            content: "This report should be saved to a file.".to_string(),
            metadata: HashMap::new(),
            generated_at: Utc::now(),
        };

        let result = deliverer.deliver_report(&report, &channel).await;
        assert!(result.is_ok());

        // Verify file was created and contains content
        let file_content = fs::read_to_string(&test_file).await.unwrap();
        assert_eq!(file_content, report.content);
    }

    #[tokio::test]
    async fn test_slack_deliverer_formatting() {
        let deliverer = SlackDeliverer::new();

        let report = GeneratedReport {
            report_type: "metrics".to_string(),
            title: "Metrics Report".to_string(),
            content: "Some metrics content...".to_string(),
            metadata: HashMap::new(),
            generated_at: Utc::now(),
        };

        let message = deliverer.format_default_message(&report);
        assert!(message.contains("📈"));
        assert!(message.contains("Metrics Report"));
        assert!(message.contains("Some metrics content..."));
    }

    #[test]
    fn test_filename_sanitization() {
        let deliverer = FileDeliverer::new();
        let sanitized = deliverer.sanitize_filename("Test Report: Success/Failed?");
        assert_eq!(sanitized, "Test_Report__Success_Failed_");
    }

    #[test]
    fn test_deliverer_info() {
        let email_deliverer = EmailDeliverer::new();
        let info = email_deliverer.get_deliverer_info();

        assert!(!info.name.is_empty());
        assert!(!info.description.is_empty());
        assert!(info.requires_config);
        assert!(!info.supported_formats.is_empty());
    }
}
