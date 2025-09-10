// ABOUTME: Output handler module for workflow result formatting and persistence
// ABOUTME: Handles JSON/YAML formatting and output to files, S3, stdout, etc.

pub mod config;
pub mod error;
pub mod formatter;
pub mod writer;

use async_trait::async_trait;
use std::collections::HashMap;

use self::config::OutputConfig;
use self::error::{OutputError, Result};
use self::formatter::{JsonFormatter, OutputFormatter, TextFormatter, YamlFormatter};
use self::writer::{FileWriter, OutputWriter, S3Writer, StdoutWriter};
use crate::engine::{TaskResult, WorkflowResult};

pub struct OutputHandler {
    formatters: HashMap<String, Box<dyn OutputFormatter>>,
    writers: HashMap<String, Box<dyn OutputWriter>>,
}

#[async_trait]
pub trait OutputProcessor: Send + Sync {
    async fn process_workflow_result(
        &self,
        result: &WorkflowResult,
        config: &OutputConfig,
    ) -> Result<()>;

    async fn process_task_result(&self, result: &TaskResult, config: &OutputConfig) -> Result<()>;
}

impl OutputHandler {
    pub fn new() -> Self {
        let mut handler = Self {
            formatters: HashMap::new(),
            writers: HashMap::new(),
        };

        // Register built-in formatters
        handler.register_formatter("json", Box::new(JsonFormatter::new()));
        handler.register_formatter("yaml", Box::new(YamlFormatter::new()));
        handler.register_formatter("text", Box::new(TextFormatter::new()));
        handler.register_formatter("pretty", Box::new(JsonFormatter::new_pretty()));

        // Register built-in writers
        handler.register_writer("stdout", Box::new(StdoutWriter::new()));
        handler.register_writer("file", Box::new(FileWriter::new()));
        handler.register_writer("s3", Box::new(S3Writer::new()));

        handler
    }

    pub fn register_formatter(&mut self, name: &str, formatter: Box<dyn OutputFormatter>) {
        self.formatters.insert(name.to_string(), formatter);
    }

    pub fn register_writer(&mut self, name: &str, writer: Box<dyn OutputWriter>) {
        self.writers.insert(name.to_string(), writer);
    }

    pub async fn output_workflow_result(
        &self,
        result: &WorkflowResult,
        config: &OutputConfig,
    ) -> Result<()> {
        // Get formatter
        let formatter =
            self.formatters
                .get(&config.format)
                .ok_or_else(|| OutputError::FormatterNotFound {
                    format: config.format.clone(),
                })?;

        // Format the result
        let formatted_output = formatter.format_workflow_result(result, config).await?;

        // Write to all configured destinations
        for destination in &config.destinations {
            if let Some(writer) = self.writers.get(&destination.writer_type) {
                writer.write(&formatted_output, destination).await?;
            } else {
                return Err(OutputError::WriterNotFound {
                    writer_type: destination.writer_type.clone(),
                });
            }
        }

        Ok(())
    }

    pub async fn output_task_result(
        &self,
        result: &TaskResult,
        config: &OutputConfig,
    ) -> Result<()> {
        // Get formatter
        let formatter =
            self.formatters
                .get(&config.format)
                .ok_or_else(|| OutputError::FormatterNotFound {
                    format: config.format.clone(),
                })?;

        // Format the result
        let formatted_output = formatter.format_task_result(result, config).await?;

        // Write to all configured destinations
        for destination in &config.destinations {
            if let Some(writer) = self.writers.get(&destination.writer_type) {
                writer.write(&formatted_output, destination).await?;
            } else {
                return Err(OutputError::WriterNotFound {
                    writer_type: destination.writer_type.clone(),
                });
            }
        }

        Ok(())
    }

    pub fn list_formatters(&self) -> Vec<&str> {
        self.formatters.keys().map(|k| k.as_str()).collect()
    }

    pub fn list_writers(&self) -> Vec<&str> {
        self.writers.keys().map(|k| k.as_str()).collect()
    }
}

#[async_trait]
impl OutputProcessor for OutputHandler {
    async fn process_workflow_result(
        &self,
        result: &WorkflowResult,
        config: &OutputConfig,
    ) -> Result<()> {
        self.output_workflow_result(result, config).await
    }

    async fn process_task_result(&self, result: &TaskResult, config: &OutputConfig) -> Result<()> {
        self.output_task_result(result, config).await
    }
}

impl Default for OutputHandler {
    fn default() -> Self {
        Self::new()
    }
}
