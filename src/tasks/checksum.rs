// ABOUTME: Checksum task implementation for generating file checksums
// ABOUTME: Supports SHA256, SHA512, and MD5 algorithms with optional base64 encoding

use async_trait::async_trait;
use base64::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use tokio::fs;
use tracing::{debug, error, info};

use super::TaskImplementation;
use crate::engine::error::{ExecutionError, Result};
use crate::engine::{ExecutionContext, TaskResult, TaskStatus};

pub struct ChecksumTask;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecksumConfig {
    pub input_path: String,
    pub output_path: Option<String>,
    #[serde(default = "default_checksum_algorithm")]
    pub algorithm: ChecksumAlgorithm,
    #[serde(default)]
    pub base64_encode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChecksumAlgorithm {
    Sha256,
    Sha512,
    Md5,
}

fn default_checksum_algorithm() -> ChecksumAlgorithm {
    ChecksumAlgorithm::Sha256
}

impl Default for ChecksumConfig {
    fn default() -> Self {
        Self {
            input_path: String::new(),
            output_path: None,
            algorithm: ChecksumAlgorithm::Sha256,
            base64_encode: false,
        }
    }
}

#[async_trait]
impl TaskImplementation for ChecksumTask {
    async fn execute(
        &self,
        task_id: String,
        config: serde_yaml::Value,
        _context: ExecutionContext,
    ) -> Result<TaskResult> {
        let start_time = chrono::Utc::now();

        let config: ChecksumConfig =
            serde_yaml::from_value(config).map_err(|e| ExecutionError::ConfigError {
                task_id: task_id.clone(),
                message: format!("Invalid checksum configuration: {}", e),
            })?;

        info!(
            "Executing checksum task: {} - {} ({:?})",
            task_id, config.input_path, config.algorithm
        );

        let mut task_result = TaskResult::new(task_id.clone(), "checksum".to_string());
        task_result.start_time = start_time;
        task_result.mark_started();

        // Validate input file exists
        let input_path = Path::new(&config.input_path);
        if !input_path.exists() {
            let error_msg = format!("Input file does not exist: {}", config.input_path);
            error!("{}", error_msg);
            task_result.mark_completed(TaskStatus::Failed, None, Some(error_msg));
            return Ok(task_result);
        }

        if !input_path.is_file() {
            let error_msg = format!("Input path is not a file: {}", config.input_path);
            error!("{}", error_msg);
            task_result.mark_completed(TaskStatus::Failed, None, Some(error_msg));
            return Ok(task_result);
        }

        // Clone values needed after the move
        let input_path_clone = config.input_path.clone();
        let output_path_clone = config.output_path.clone();
        let base64_encode = config.base64_encode;

        // Execute checksum calculation in a blocking task
        let task_id_clone = task_id.clone();
        let result = tokio::task::spawn_blocking(move || {
            Self::calculate_checksum(&config, &task_id_clone)
        })
        .await;

        match result {
            Ok(Ok(checksum_result)) => {
                info!(
                    "Checksum calculation completed: {} ({})",
                    checksum_result.checksum, checksum_result.algorithm
                );

                // Write checksum to output file if specified
                if let Some(output_path) = &output_path_clone {
                    let checksum_content = format!(
                        "{}  {}\n",
                        checksum_result.checksum, input_path_clone
                    );

                    if let Err(e) = fs::write(output_path, &checksum_content).await {
                        let error_msg = format!("Failed to write checksum file: {}", e);
                        error!("{}", error_msg);
                        task_result.mark_completed(TaskStatus::Failed, None, Some(error_msg));
                        return Ok(task_result);
                    }

                    task_result.add_metadata("output_file".to_string(), output_path.clone());
                }

                // Add metadata
                task_result.add_metadata("checksum".to_string(), checksum_result.checksum.clone());
                task_result.add_metadata("algorithm".to_string(), checksum_result.algorithm.clone());
                task_result.add_metadata("input_path".to_string(), input_path_clone.clone());
                task_result.add_metadata("file_size".to_string(), checksum_result.file_size.to_string());
                task_result.add_metadata("base64_encoded".to_string(), base64_encode.to_string());

                let output_message = format!(
                    "Checksum calculated: {} ({})",
                    checksum_result.checksum, checksum_result.algorithm
                );

                task_result.mark_completed(TaskStatus::Success, Some(output_message), None);
            }
            Ok(Err(e)) => {
                let error_msg = format!("Checksum calculation failed: {}", e);
                error!("{}", error_msg);
                task_result.mark_completed(TaskStatus::Failed, None, Some(error_msg));
            }
            Err(e) => {
                let error_msg = format!("Checksum task panicked: {}", e);
                error!("{}", error_msg);
                task_result.mark_completed(TaskStatus::Failed, None, Some(error_msg));
            }
        }

        Ok(task_result)
    }

    fn task_type(&self) -> &'static str {
        "checksum"
    }

    fn validate_config(&self, config: &serde_yaml::Value) -> Result<()> {
        let config: ChecksumConfig =
            serde_yaml::from_value(config.clone()).map_err(|e| ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: format!("Invalid checksum configuration: {}", e),
            })?;

        if config.input_path.is_empty() {
            return Err(ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: "Input path cannot be empty".to_string(),
            });
        }

        let input_path = Path::new(&config.input_path);
        if !input_path.exists() {
            return Err(ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: format!("Input file does not exist: {}", config.input_path),
            });
        }

        if !input_path.is_file() {
            return Err(ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: format!("Input path is not a file: {}", config.input_path),
            });
        }

        // Validate output directory exists if output path is specified
        if let Some(ref output_path) = config.output_path {
            let output_path = Path::new(output_path);
            if let Some(parent) = output_path.parent() {
                if !parent.exists() {
                    return Err(ExecutionError::ConfigError {
                        task_id: "validation".to_string(),
                        message: format!("Output directory does not exist: {}", parent.display()),
                    });
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
struct ChecksumResult {
    checksum: String,
    algorithm: String,
    file_size: u64,
}

impl ChecksumTask {
    fn calculate_checksum(config: &ChecksumConfig, task_id: &str) -> io::Result<ChecksumResult> {
        let input_path = Path::new(&config.input_path);
        
        // Get file size for metadata
        let metadata = std::fs::metadata(input_path)?;
        let file_size = metadata.len();

        debug!("Calculating checksum for: {} ({} bytes)", input_path.display(), file_size);

        let mut file = File::open(input_path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let (checksum_bytes, algorithm_name) = match config.algorithm {
            ChecksumAlgorithm::Sha256 => {
                let mut hasher = Sha256::new();
                hasher.update(&buffer);
                (hasher.finalize().to_vec(), "SHA256")
            }
            ChecksumAlgorithm::Sha512 => {
                let mut hasher = Sha512::new();
                hasher.update(&buffer);
                (hasher.finalize().to_vec(), "SHA512")
            }
            ChecksumAlgorithm::Md5 => {
                let digest = md5::compute(&buffer);
                (digest.0.to_vec(), "MD5")
            }
        };

        let checksum = if config.base64_encode {
            base64::prelude::BASE64_STANDARD.encode(&checksum_bytes)
        } else {
            hex::encode(&checksum_bytes)
        };

        debug!(
            "Checksum calculated for task {}: {} ({}, {} bytes)",
            task_id, checksum, algorithm_name, file_size
        );

        Ok(ChecksumResult {
            checksum,
            algorithm: algorithm_name.to_string(),
            file_size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::TemplateContext;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_checksum_validation() {
        let task = ChecksumTask;

        // Test empty input path
        let empty_input_config = serde_yaml::to_value(ChecksumConfig {
            input_path: "".to_string(),
            ..Default::default()
        })
        .unwrap();

        let result = task.validate_config(&empty_input_config);
        assert!(result.is_err());

        // Test non-existent file
        let nonexistent_config = serde_yaml::to_value(ChecksumConfig {
            input_path: "/nonexistent/file.txt".to_string(),
            ..Default::default()
        })
        .unwrap();

        let result = task.validate_config(&nonexistent_config);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_sha256_checksum() {
        let task = ChecksumTask;
        let variables = HashMap::new();
        let template_context = TemplateContext::new(&variables).unwrap();
        let context = ExecutionContext::new(
            "test_workflow".to_string(),
            "run_123".to_string(),
            "checksum_task".to_string(),
            template_context,
        );

        // Create a temporary file
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        let output_file = temp_dir.path().join("test.txt.checksum");

        fs::write(&test_file, "Hello, World!").unwrap();

        let config = serde_yaml::to_value(ChecksumConfig {
            input_path: test_file.to_string_lossy().to_string(),
            output_path: Some(output_file.to_string_lossy().to_string()),
            algorithm: ChecksumAlgorithm::Sha256,
            base64_encode: false,
        })
        .unwrap();

        let result = task
            .execute("test_checksum".to_string(), config, context)
            .await
            .unwrap();

        assert_eq!(result.status, TaskStatus::Success);
        assert!(output_file.exists());
        assert!(result.metadata.contains_key("checksum"));
        assert_eq!(
            result.metadata.get("algorithm"),
            Some(&"SHA256".to_string())
        );

        // Verify the checksum is correct for "Hello, World!"
        let expected_checksum = "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f";
        assert_eq!(
            result.metadata.get("checksum"),
            Some(&expected_checksum.to_string())
        );
    }

    #[tokio::test]
    async fn test_missing_input_file() {
        let task = ChecksumTask;
        let variables = HashMap::new();
        let template_context = TemplateContext::new(&variables).unwrap();
        let context = ExecutionContext::new(
            "test_workflow".to_string(),
            "run_123".to_string(),
            "checksum_task".to_string(),
            template_context,
        );

        let config = serde_yaml::to_value(ChecksumConfig {
            input_path: "/nonexistent/file.txt".to_string(),
            output_path: None,
            algorithm: ChecksumAlgorithm::Sha256,
            base64_encode: false,
        })
        .unwrap();

        let result = task
            .execute("test_checksum".to_string(), config, context)
            .await
            .unwrap();

        assert_eq!(result.status, TaskStatus::Failed);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("does not exist"));
    }
}