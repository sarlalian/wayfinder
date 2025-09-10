// ABOUTME: Individual file compression task implementation
// ABOUTME: Supports bzip2, xz, and lza compression for single files

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::path::Path;
use tracing::{debug, error, info};

use super::TaskImplementation;
use crate::engine::error::{ExecutionError, Result};
use crate::engine::{ExecutionContext, TaskResult, TaskStatus};

pub struct CompressTask;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressConfig {
    pub input_path: String,
    pub output_path: String,
    pub compression_type: CompressionType,
    pub compression_level: Option<u32>,
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

impl Default for CompressConfig {
    fn default() -> Self {
        Self {
            input_path: String::new(),
            output_path: String::new(),
            compression_type: CompressionType::Bzip2,
            compression_level: None,
            preserve_original: false,
        }
    }
}

#[async_trait]
impl TaskImplementation for CompressTask {
    async fn execute(
        &self,
        task_id: String,
        config: serde_yaml::Value,
        _context: ExecutionContext,
    ) -> Result<TaskResult> {
        let start_time = chrono::Utc::now();

        let config: CompressConfig =
            serde_yaml::from_value(config).map_err(|e| ExecutionError::ConfigError {
                task_id: task_id.clone(),
                message: format!("Invalid compress configuration: {}", e),
            })?;

        info!(
            "Executing compress task: {} - {} -> {} ({:?})",
            task_id, config.input_path, config.output_path, config.compression_type
        );

        let mut task_result = TaskResult::new(task_id.clone(), "compress".to_string());
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

        // Create output directory if it doesn't exist
        let output_path = Path::new(&config.output_path);
        if let Some(parent) = output_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                let error_msg = format!("Failed to create output directory: {}", e);
                error!("{}", error_msg);
                task_result.mark_completed(TaskStatus::Failed, None, Some(error_msg));
                return Ok(task_result);
            }
        }

        // Clone values needed after the move
        let input_path_clone = config.input_path.clone();
        let output_path_clone = config.output_path.clone();
        let preserve_original = config.preserve_original;
        let compression_type = config.compression_type.clone();

        // Execute compression in a blocking task
        let task_id_clone = task_id.clone();
        let result = tokio::task::spawn_blocking(move || {
            Self::compress_file(&config, &task_id_clone)
        })
        .await;

        match result {
            Ok(Ok(compression_result)) => {
                info!(
                    "Compression completed: {} -> {} ({} bytes -> {} bytes, {:.1}% reduction)",
                    input_path_clone,
                    output_path_clone,
                    compression_result.original_size,
                    compression_result.compressed_size,
                    compression_result.compression_ratio
                );

                // Remove original file if not preserving
                if !preserve_original {
                    if let Err(e) = std::fs::remove_file(&input_path_clone) {
                        let warning = format!("Failed to remove original file: {}", e);
                        debug!("{}", warning);
                        // Don't fail the task for this, just log it
                    }
                }

                // Add metadata
                task_result.add_metadata("input_path".to_string(), input_path_clone.clone());
                task_result.add_metadata("output_path".to_string(), output_path_clone.clone());
                task_result.add_metadata("compression_type".to_string(), format!("{:?}", compression_type));
                task_result.add_metadata("original_size".to_string(), compression_result.original_size.to_string());
                task_result.add_metadata("compressed_size".to_string(), compression_result.compressed_size.to_string());
                task_result.add_metadata("compression_ratio".to_string(), format!("{:.1}", compression_result.compression_ratio));
                task_result.add_metadata("preserve_original".to_string(), preserve_original.to_string());

                let output_message = format!(
                    "Successfully compressed {} to {} ({:.1}% reduction)",
                    input_path_clone, output_path_clone, compression_result.compression_ratio
                );

                task_result.mark_completed(TaskStatus::Success, Some(output_message), None);
            }
            Ok(Err(e)) => {
                let error_msg = format!("Compression failed: {}", e);
                error!("{}", error_msg);
                task_result.mark_completed(TaskStatus::Failed, None, Some(error_msg));
            }
            Err(e) => {
                let error_msg = format!("Compression task panicked: {}", e);
                error!("{}", error_msg);
                task_result.mark_completed(TaskStatus::Failed, None, Some(error_msg));
            }
        }

        Ok(task_result)
    }

    fn task_type(&self) -> &'static str {
        "compress"
    }

    fn validate_config(&self, config: &serde_yaml::Value) -> Result<()> {
        let config: CompressConfig =
            serde_yaml::from_value(config.clone()).map_err(|e| ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: format!("Invalid compress configuration: {}", e),
            })?;

        if config.input_path.is_empty() {
            return Err(ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: "Input path cannot be empty".to_string(),
            });
        }

        if config.output_path.is_empty() {
            return Err(ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: "Output path cannot be empty".to_string(),
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

        // Validate compression level if specified
        if let Some(level) = config.compression_level {
            let max_level = match config.compression_type {
                CompressionType::Bzip2 => 9,
                CompressionType::Xz => 9,
                CompressionType::Lza => 9,
            };

            if level > max_level {
                return Err(ExecutionError::ConfigError {
                    task_id: "validation".to_string(),
                    message: format!(
                        "Compression level {} is too high for {:?} (max: {})",
                        level, config.compression_type, max_level
                    ),
                });
            }
        }

        // Validate output directory exists
        let output_path = Path::new(&config.output_path);
        if let Some(parent) = output_path.parent() {
            if !parent.exists() {
                return Err(ExecutionError::ConfigError {
                    task_id: "validation".to_string(),
                    message: format!("Output directory does not exist: {}", parent.display()),
                });
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
struct CompressionResult {
    original_size: u64,
    compressed_size: u64,
    compression_ratio: f64,
}

impl CompressTask {
    fn compress_file(config: &CompressConfig, task_id: &str) -> io::Result<CompressionResult> {
        let input_path = Path::new(&config.input_path);
        let output_path = Path::new(&config.output_path);

        // Get original file size
        let metadata = std::fs::metadata(input_path)?;
        let original_size = metadata.len();

        debug!(
            "Compressing file for task {}: {} -> {} ({} bytes)",
            task_id,
            input_path.display(),
            output_path.display(),
            original_size
        );

        let input_file = File::open(input_path)?;
        let output_file = File::create(output_path)?;
        let mut reader = BufReader::new(input_file);

        match config.compression_type {
            CompressionType::Bzip2 => {
                let level = config.compression_level.unwrap_or(6);
                let mut compressor = bzip2::write::BzEncoder::new(output_file, bzip2::Compression::new(level));
                std::io::copy(&mut reader, &mut compressor)?;
                compressor.finish()?;
            }
            CompressionType::Xz => {
                let level = config.compression_level.unwrap_or(6);
                let mut compressor = xz2::write::XzEncoder::new(output_file, level);
                std::io::copy(&mut reader, &mut compressor)?;
                compressor.finish()?;
            }
            CompressionType::Lza => {
                // For LZMA, we'll use a simple implementation
                let mut writer = BufWriter::new(output_file);
                lzma_rs::lzma_compress(&mut reader, &mut writer)?;
            }
        }

        // Get compressed file size
        let compressed_metadata = std::fs::metadata(output_path)?;
        let compressed_size = compressed_metadata.len();

        let compression_ratio = if original_size > 0 {
            ((original_size - compressed_size) as f64 / original_size as f64) * 100.0
        } else {
            0.0
        };

        debug!(
            "Compression completed for task {}: {} bytes -> {} bytes ({:.1}% reduction)",
            task_id, original_size, compressed_size, compression_ratio
        );

        Ok(CompressionResult {
            original_size,
            compressed_size,
            compression_ratio,
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
    async fn test_compress_validation() {
        let task = CompressTask;

        // Test empty input path
        let empty_input_config = serde_yaml::to_value(CompressConfig {
            input_path: "".to_string(),
            output_path: "/tmp/test.bz2".to_string(),
            ..Default::default()
        })
        .unwrap();

        let result = task.validate_config(&empty_input_config);
        assert!(result.is_err());

        // Test empty output path
        let empty_output_config = serde_yaml::to_value(CompressConfig {
            input_path: "/tmp/test.txt".to_string(),
            output_path: "".to_string(),
            ..Default::default()
        })
        .unwrap();

        let result = task.validate_config(&empty_output_config);
        assert!(result.is_err());

        // Test non-existent file
        let nonexistent_config = serde_yaml::to_value(CompressConfig {
            input_path: "/nonexistent/file.txt".to_string(),
            output_path: "/tmp/test.bz2".to_string(),
            ..Default::default()
        })
        .unwrap();

        let result = task.validate_config(&nonexistent_config);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_bzip2_compression() {
        let task = CompressTask;
        let variables = HashMap::new();
        let template_context = TemplateContext::new(&variables).unwrap();
        let context = ExecutionContext::new(
            "test_workflow".to_string(),
            "run_123".to_string(),
            "compress_task".to_string(),
            template_context,
        );

        // Create a temporary file
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        let output_file = temp_dir.path().join("test.txt.bz2");

        // Create a file with some compressible content
        let content = "Hello, World! ".repeat(100);
        fs::write(&test_file, &content).unwrap();

        let config = serde_yaml::to_value(CompressConfig {
            input_path: test_file.to_string_lossy().to_string(),
            output_path: output_file.to_string_lossy().to_string(),
            compression_type: CompressionType::Bzip2,
            compression_level: Some(9),
            preserve_original: true,
        })
        .unwrap();

        let result = task
            .execute("test_compress".to_string(), config, context)
            .await
            .unwrap();

        assert_eq!(result.status, TaskStatus::Success);
        assert!(output_file.exists());
        assert!(test_file.exists()); // Should be preserved
        assert!(result.metadata.contains_key("compressed_size"));
        assert!(result.metadata.contains_key("compression_ratio"));

        // Check that compression actually reduced file size
        let original_size: u64 = result.metadata.get("original_size").unwrap().parse().unwrap();
        let compressed_size: u64 = result.metadata.get("compressed_size").unwrap().parse().unwrap();
        assert!(compressed_size < original_size);
    }

    #[tokio::test]
    async fn test_missing_input_file() {
        let task = CompressTask;
        let variables = HashMap::new();
        let template_context = TemplateContext::new(&variables).unwrap();
        let context = ExecutionContext::new(
            "test_workflow".to_string(),
            "run_123".to_string(),
            "compress_task".to_string(),
            template_context,
        );

        let config = serde_yaml::to_value(CompressConfig {
            input_path: "/nonexistent/file.txt".to_string(),
            output_path: "/tmp/test.bz2".to_string(),
            compression_type: CompressionType::Bzip2,
            compression_level: None,
            preserve_original: false,
        })
        .unwrap();

        let result = task
            .execute("test_compress".to_string(), config, context)
            .await
            .unwrap();

        assert_eq!(result.status, TaskStatus::Failed);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_preserve_original_false() {
        let task = CompressTask;
        let variables = HashMap::new();
        let template_context = TemplateContext::new(&variables).unwrap();
        let context = ExecutionContext::new(
            "test_workflow".to_string(),
            "run_123".to_string(),
            "compress_task".to_string(),
            template_context,
        );

        // Create a temporary file
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        let output_file = temp_dir.path().join("test.txt.bz2");

        let content = "Hello, World!";
        fs::write(&test_file, content).unwrap();

        let config = serde_yaml::to_value(CompressConfig {
            input_path: test_file.to_string_lossy().to_string(),
            output_path: output_file.to_string_lossy().to_string(),
            compression_type: CompressionType::Bzip2,
            compression_level: None,
            preserve_original: false, // Should remove original
        })
        .unwrap();

        let result = task
            .execute("test_compress".to_string(), config, context)
            .await
            .unwrap();

        assert_eq!(result.status, TaskStatus::Success);
        assert!(output_file.exists());
        assert!(!test_file.exists()); // Should be removed
    }
}