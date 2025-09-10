// ABOUTME: Output writers for different destinations (stdout, files, S3)
// ABOUTME: Handles writing formatted results to various output destinations

use async_trait::async_trait;
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info};

use super::config::{FileWriterConfig, OutputDestination, S3WriterConfig, StdoutWriterConfig};
use super::error::{OutputError, Result};

#[async_trait]
pub trait OutputWriter: Send + Sync {
    async fn write(&self, content: &str, destination: &OutputDestination) -> Result<()>;
}

pub struct StdoutWriter;

pub struct FileWriter;

pub struct S3Writer;

impl Default for StdoutWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl StdoutWriter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl OutputWriter for StdoutWriter {
    async fn write(&self, content: &str, destination: &OutputDestination) -> Result<()> {
        let config: StdoutWriterConfig =
            destination
                .get_config()
                .unwrap_or(StdoutWriterConfig {
                    colored: true,
                    quiet: false,
                });

        if !config.quiet {
            if config.colored {
                // For now, just print without color - can add colored output later
                println!("{}", content);
            } else {
                println!("{}", content);
            }
        }

        debug!("Output written to stdout ({} chars)", content.len());
        Ok(())
    }
}

impl Default for FileWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl FileWriter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl OutputWriter for FileWriter {
    async fn write(&self, content: &str, destination: &OutputDestination) -> Result<()> {
        let config: FileWriterConfig =
            destination
                .get_config()
                .map_err(|e| OutputError::ConfigError {
                    message: format!("Invalid file writer config: {}", e),
                })?;

        let output_path = Path::new(&config.path);

        // Create parent directories if needed
        if config.create_dirs {
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)
                    .await
                    .map_err(|e| OutputError::WriteError {
                        message: format!("Failed to create directory {}: {}", parent.display(), e),
                    })?;
            }
        }

        // Backup existing file if requested
        if config.backup_existing && output_path.exists() {
            let backup_path = format!("{}.bak", config.path);
            fs::copy(&config.path, &backup_path)
                .await
                .map_err(|e| OutputError::WriteError {
                    message: format!("Failed to backup existing file: {}", e),
                })?;
            debug!("Backed up existing file to {}", backup_path);
        }

        // Write content to file
        if config.append {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&config.path)
                .await
                .map_err(|e| OutputError::WriteError {
                    message: format!("Failed to open file for append {}: {}", config.path, e),
                })?;

            file.write_all(content.as_bytes())
                .await
                .map_err(|e| OutputError::WriteError {
                    message: format!("Failed to append to file {}: {}", config.path, e),
                })?;

            // Add newline for appending
            file.write_all(b"\n")
                .await
                .map_err(|e| OutputError::WriteError {
                    message: format!("Failed to append newline to file {}: {}", config.path, e),
                })?;
        } else {
            fs::write(&config.path, content)
                .await
                .map_err(|e| OutputError::WriteError {
                    message: format!("Failed to write file {}: {}", config.path, e),
                })?;
        }

        info!(
            "Output written to file: {} ({} bytes)",
            config.path,
            content.len()
        );
        Ok(())
    }
}

impl Default for S3Writer {
    fn default() -> Self {
        Self::new()
    }
}

impl S3Writer {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl OutputWriter for S3Writer {
    async fn write(&self, content: &str, destination: &OutputDestination) -> Result<()> {
        let config: S3WriterConfig =
            destination
                .get_config()
                .map_err(|e| OutputError::ConfigError {
                    message: format!("Invalid S3 writer config: {}", e),
                })?;

        // For now, this is a placeholder implementation
        // In a real implementation, this would use the AWS SDK to upload to S3
        info!(
            "Would upload {} bytes to s3://{}/{}",
            content.len(),
            config.bucket,
            config.key
        );

        debug!(
            "S3 upload details: bucket={}, key={}, content_type={:?}",
            config.bucket, config.key, config.content_type
        );

        // Simulate successful upload
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        Ok(())
    }
}

// Additional writer implementations can be added here

pub struct HttpWriter {
    client: reqwest::Client,
}

impl Default for HttpWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpWriter {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[derive(serde::Deserialize)]
pub struct HttpWriterConfig {
    pub url: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

impl Default for HttpWriterConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            method: "POST".to_string(),
            headers: std::collections::HashMap::new(),
            timeout_seconds: Some(30),
        }
    }
}

#[async_trait]
impl OutputWriter for HttpWriter {
    async fn write(&self, content: &str, destination: &OutputDestination) -> Result<()> {
        let config: HttpWriterConfig =
            destination
                .get_config()
                .map_err(|e| OutputError::ConfigError {
                    message: format!("Invalid HTTP writer config: {}", e),
                })?;

        let method = match config.method.to_uppercase().as_str() {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "PATCH" => reqwest::Method::PATCH,
            _ => {
                return Err(OutputError::ConfigError {
                    message: format!("Unsupported HTTP method: {}", config.method),
                });
            }
        };

        let mut request = self
            .client
            .request(method, &config.url)
            .body(content.to_string());

        // Add headers
        for (key, value) in &config.headers {
            request = request.header(key, value);
        }

        // Set timeout
        if let Some(timeout_secs) = config.timeout_seconds {
            request = request.timeout(tokio::time::Duration::from_secs(timeout_secs));
        }

        let response = request.send().await.map_err(|e| OutputError::WriteError {
            message: format!("HTTP request failed: {}", e),
        })?;

        if !response.status().is_success() {
            return Err(OutputError::WriteError {
                message: format!("HTTP request failed with status: {}", response.status()),
            });
        }

        info!(
            "Output sent to HTTP endpoint: {} ({} bytes)",
            config.url,
            content.len()
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_stdout_writer() {
        let writer = StdoutWriter::new();
        let destination = OutputDestination::new_stdout();

        // This should not fail
        let result = writer.write("Test output", &destination).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_file_writer() {
        let writer = FileWriter::new();
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_output.txt");

        let destination = OutputDestination::new_file(test_file.to_string_lossy());

        let result = writer.write("Test file content", &destination).await;
        assert!(result.is_ok());

        // Verify file was written
        let content = fs::read_to_string(&test_file).await.unwrap();
        assert_eq!(content, "Test file content");
    }

    #[tokio::test]
    async fn test_file_writer_with_directories() {
        let writer = FileWriter::new();
        let temp_dir = TempDir::new().unwrap();
        let nested_file = temp_dir.path().join("nested").join("dir").join("test.txt");

        let mut config = HashMap::new();
        config.insert(
            "path".to_string(),
            serde_yaml::Value::String(nested_file.to_string_lossy().to_string()),
        );
        config.insert("create_dirs".to_string(), serde_yaml::Value::Bool(true));

        let destination = OutputDestination {
            writer_type: "file".to_string(),
            config,
        };

        let result = writer.write("Test nested content", &destination).await;
        assert!(result.is_ok());

        // Verify file was written
        let content = fs::read_to_string(&nested_file).await.unwrap();
        assert_eq!(content, "Test nested content");
    }

    #[tokio::test]
    async fn test_s3_writer_placeholder() {
        let writer = S3Writer::new();
        let destination = OutputDestination::new_s3("test-bucket", "test/output.json");

        // Placeholder implementation should not fail
        let result = writer.write("Test S3 content", &destination).await;
        assert!(result.is_ok());
    }
}
