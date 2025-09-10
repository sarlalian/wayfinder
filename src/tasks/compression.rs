// ABOUTME: Compression task implementation for creating tar.gz archives
// ABOUTME: Handles file and directory compression with configurable exclusion patterns

use async_trait::async_trait;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self};
use std::path::Path;
use tar::Builder;
use tracing::{debug, error, info};
use walkdir::WalkDir;

use super::TaskImplementation;
use crate::engine::error::{ExecutionError, Result};
use crate::engine::{ExecutionContext, TaskResult, TaskStatus};

pub struct CompressionTask;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    pub source_path: String,
    pub output_path: String,
    #[serde(default)]
    pub compression_level: u32,
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
    #[serde(default = "default_follow_symlinks")]
    pub follow_symlinks: bool,
    #[serde(default = "default_preserve_permissions")]
    pub preserve_permissions: bool,
}

fn default_follow_symlinks() -> bool {
    false
}

fn default_preserve_permissions() -> bool {
    true
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            source_path: String::new(),
            output_path: String::new(),
            compression_level: 6, // Default gzip compression level
            exclude_patterns: Vec::new(),
            follow_symlinks: false,
            preserve_permissions: true,
        }
    }
}

#[async_trait]
impl TaskImplementation for CompressionTask {
    async fn execute(
        &self,
        task_id: String,
        config: serde_yaml::Value,
        _context: ExecutionContext,
    ) -> Result<TaskResult> {
        let start_time = chrono::Utc::now();

        let config: CompressionConfig =
            serde_yaml::from_value(config).map_err(|e| ExecutionError::ConfigError {
                task_id: task_id.clone(),
                message: format!("Invalid compression configuration: {}", e),
            })?;

        info!(
            "Executing compression task: {} - {} -> {}",
            task_id, config.source_path, config.output_path
        );

        let mut task_result = TaskResult::new(task_id.clone(), "compression".to_string());
        task_result.start_time = start_time;
        task_result.mark_started();

        // Clone values needed after the move
        let source_path = config.source_path.clone();
        let output_path = config.output_path.clone();

        // Execute compression in a blocking task to avoid blocking async runtime
        let task_id_clone = task_id.clone();
        let result =
            tokio::task::spawn_blocking(move || Self::compress_files(&config, &task_id_clone))
                .await;

        match result {
            Ok(Ok(stats)) => {
                info!(
                    "Compression completed: {} files, {} bytes",
                    stats.files_compressed, stats.bytes_processed
                );

                // Add metadata
                task_result.add_metadata(
                    "files_compressed".to_string(),
                    stats.files_compressed.to_string(),
                );
                task_result.add_metadata(
                    "bytes_processed".to_string(),
                    stats.bytes_processed.to_string(),
                );
                task_result.add_metadata("source_path".to_string(), source_path);
                task_result.add_metadata("output_path".to_string(), output_path);

                let output_message = format!(
                    "Successfully compressed {} files ({} bytes) to {}",
                    stats.files_compressed, stats.bytes_processed, stats.output_path
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
        "archive"
    }

    fn validate_config(&self, config: &serde_yaml::Value) -> Result<()> {
        let config: CompressionConfig =
            serde_yaml::from_value(config.clone()).map_err(|e| ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: format!("Invalid compression configuration: {}", e),
            })?;

        if config.source_path.is_empty() {
            return Err(ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: "Source path cannot be empty".to_string(),
            });
        }

        if config.output_path.is_empty() {
            return Err(ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: "Output path cannot be empty".to_string(),
            });
        }

        if config.compression_level > 9 {
            return Err(ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: "Compression level must be between 0 and 9".to_string(),
            });
        }

        let source_path = Path::new(&config.source_path);
        if !source_path.exists() {
            return Err(ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: format!("Source path does not exist: {}", config.source_path),
            });
        }

        Ok(())
    }
}

#[derive(Debug)]
struct CompressionStats {
    files_compressed: u64,
    bytes_processed: u64,
    output_path: String,
}

impl CompressionTask {
    fn compress_files(config: &CompressionConfig, task_id: &str) -> io::Result<CompressionStats> {
        let source_path = Path::new(&config.source_path);
        let output_path = Path::new(&config.output_path);

        // Create output directory if it doesn't exist
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Create the output file
        let output_file = File::create(output_path)?;
        let encoder = GzEncoder::new(output_file, Compression::new(config.compression_level));
        let mut tar_builder = Builder::new(encoder);

        let mut stats = CompressionStats {
            files_compressed: 0,
            bytes_processed: 0,
            output_path: config.output_path.clone(),
        };

        if source_path.is_file() {
            // Compress a single file
            Self::add_file_to_archive(
                &mut tar_builder,
                source_path,
                source_path.file_name().unwrap().to_str().unwrap(),
                &mut stats,
            )?;
        } else if source_path.is_dir() {
            // Compress a directory
            for entry in WalkDir::new(source_path).follow_links(config.follow_symlinks) {
                let entry = entry?;
                let path = entry.path();

                // Skip if matches exclusion patterns
                if Self::should_exclude(path, &config.exclude_patterns) {
                    debug!("Excluding file: {}", path.display());
                    continue;
                }

                if path.is_file() {
                    // Calculate relative path for archive
                    let relative_path = path
                        .strip_prefix(source_path)
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

                    Self::add_file_to_archive(
                        &mut tar_builder,
                        path,
                        relative_path.to_str().unwrap(),
                        &mut stats,
                    )?;
                }
            }
        }

        // Finalize the archive
        tar_builder.finish()?;

        info!(
            "Compression completed for task {}: {} files, {} bytes",
            task_id, stats.files_compressed, stats.bytes_processed
        );

        Ok(stats)
    }

    fn add_file_to_archive(
        tar_builder: &mut Builder<GzEncoder<File>>,
        file_path: &Path,
        archive_path: &str,
        stats: &mut CompressionStats,
    ) -> io::Result<()> {
        let metadata = std::fs::metadata(file_path)?;
        let file_size = metadata.len();

        tar_builder.append_path_with_name(file_path, archive_path)?;

        stats.files_compressed += 1;
        stats.bytes_processed += file_size;

        debug!("Added to archive: {} ({} bytes)", archive_path, file_size);

        Ok(())
    }

    fn should_exclude(path: &Path, patterns: &[String]) -> bool {
        if patterns.is_empty() {
            return false;
        }

        let path_str = path.to_string_lossy();

        for pattern in patterns {
            if glob_match::glob_match(pattern, &path_str) {
                return true;
            }

            // Also check just the file name
            if let Some(filename) = path.file_name() {
                if glob_match::glob_match(pattern, &filename.to_string_lossy()) {
                    return true;
                }
            }
        }

        false
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
    async fn test_compression_validation() {
        let task = CompressionTask;

        // Test empty source path
        let empty_source_config = serde_yaml::to_value(CompressionConfig {
            source_path: "".to_string(),
            output_path: "/tmp/test.tar.gz".to_string(),
            ..Default::default()
        })
        .unwrap();

        let result = task.validate_config(&empty_source_config);
        assert!(result.is_err());

        // Test empty output path
        let empty_output_config = serde_yaml::to_value(CompressionConfig {
            source_path: "/tmp".to_string(),
            output_path: "".to_string(),
            ..Default::default()
        })
        .unwrap();

        let result = task.validate_config(&empty_output_config);
        assert!(result.is_err());

        // Test invalid compression level
        let invalid_level_config = serde_yaml::to_value(CompressionConfig {
            source_path: "/tmp".to_string(),
            output_path: "/tmp/test.tar.gz".to_string(),
            compression_level: 10,
            ..Default::default()
        })
        .unwrap();

        let result = task.validate_config(&invalid_level_config);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_file_compression() {
        let task = CompressionTask;
        let variables = HashMap::new();
        let template_context = TemplateContext::new(&variables).unwrap();
        let context = ExecutionContext::new(
            "test_workflow".to_string(),
            "run_123".to_string(),
            "compress_task".to_string(),
            template_context,
        );

        // Create a temporary directory and file
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        let output_file = temp_dir.path().join("output.tar.gz");

        fs::write(&test_file, "Hello, World!").unwrap();

        let config = serde_yaml::to_value(CompressionConfig {
            source_path: test_file.to_string_lossy().to_string(),
            output_path: output_file.to_string_lossy().to_string(),
            compression_level: 6,
            ..Default::default()
        })
        .unwrap();

        let result = task
            .execute("test_compress".to_string(), config, context)
            .await
            .unwrap();

        assert_eq!(result.status, TaskStatus::Success);
        assert!(output_file.exists());
        assert!(result.metadata.contains_key("files_compressed"));
        assert_eq!(
            result.metadata.get("files_compressed"),
            Some(&"1".to_string())
        );
    }
}
