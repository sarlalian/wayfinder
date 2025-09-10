// ABOUTME: Output formatters for different result formats (JSON, YAML, text)
// ABOUTME: Handles serialization and presentation of workflow and task results

use async_trait::async_trait;
use chrono::Utc;
use serde_json::{self, Value as JsonValue};

use super::config::{OutputConfig, OutputOptions};
use super::error::{OutputError, Result};
use crate::engine::{TaskResult, TaskStatus, WorkflowResult};

#[async_trait]
pub trait OutputFormatter: Send + Sync {
    async fn format_workflow_result(
        &self,
        result: &WorkflowResult,
        config: &OutputConfig,
    ) -> Result<String>;

    async fn format_task_result(
        &self,
        result: &TaskResult,
        config: &OutputConfig,
    ) -> Result<String>;
}

pub struct JsonFormatter {
    pretty: bool,
}

pub struct YamlFormatter;

pub struct TextFormatter;

impl Default for JsonFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonFormatter {
    pub fn new() -> Self {
        Self { pretty: false }
    }

    pub fn new_pretty() -> Self {
        Self { pretty: true }
    }
}

#[async_trait]
impl OutputFormatter for JsonFormatter {
    async fn format_workflow_result(
        &self,
        result: &WorkflowResult,
        config: &OutputConfig,
    ) -> Result<String> {
        let formatted_result = self
            .prepare_workflow_output(result, &config.options)
            .await?;

        if self.pretty || config.options.pretty_print {
            serde_json::to_string_pretty(&formatted_result).map_err(OutputError::SerializationError)
        } else {
            serde_json::to_string(&formatted_result).map_err(OutputError::SerializationError)
        }
    }

    async fn format_task_result(
        &self,
        result: &TaskResult,
        config: &OutputConfig,
    ) -> Result<String> {
        let formatted_result = self.prepare_task_output(result, &config.options).await?;

        if self.pretty || config.options.pretty_print {
            serde_json::to_string_pretty(&formatted_result).map_err(OutputError::SerializationError)
        } else {
            serde_json::to_string(&formatted_result).map_err(OutputError::SerializationError)
        }
    }
}

impl JsonFormatter {
    async fn prepare_workflow_output(
        &self,
        result: &WorkflowResult,
        options: &OutputOptions,
    ) -> Result<JsonValue> {
        let mut output = serde_json::Map::new();

        // Basic workflow information
        output.insert(
            "workflow_name".to_string(),
            JsonValue::String(result.workflow_name.clone()),
        );
        output.insert(
            "run_id".to_string(),
            JsonValue::String(result.run_id.clone()),
        );
        output.insert(
            "status".to_string(),
            JsonValue::String(result.status.to_string()),
        );

        // Timestamps
        if options.include_timestamps {
            output.insert(
                "start_time".to_string(),
                JsonValue::String(result.start_time.to_rfc3339()),
            );
            if let Some(end_time) = result.end_time {
                output.insert(
                    "end_time".to_string(),
                    JsonValue::String(end_time.to_rfc3339()),
                );
            }
        }

        // Duration
        if options.include_duration {
            if let Some(duration) = result.duration {
                output.insert(
                    "duration_seconds".to_string(),
                    JsonValue::Number(
                        serde_json::Number::from_f64(duration.as_secs_f64()).unwrap(),
                    ),
                );
            }
        }

        // Summary
        output.insert(
            "summary".to_string(),
            serde_json::to_value(&result.summary)?,
        );

        // Task results
        if options.include_task_results {
            let mut filtered_tasks = Vec::new();

            for task in &result.tasks {
                // Apply status filters - skip tasks that don't match the filter criteria
                let include_task = if options.filter_successful && options.filter_failed {
                    // Include both successful and failed tasks
                    task.is_successful() || task.is_failed()
                } else if options.filter_successful {
                    // Include only successful tasks
                    task.is_successful()
                } else if options.filter_failed {
                    // Include only failed tasks
                    task.is_failed()
                } else {
                    // Include all tasks (no filtering)
                    true
                };

                if !include_task {
                    continue;
                }

                let task_json = self.prepare_task_output(task, options).await?;
                filtered_tasks.push(task_json);
            }

            output.insert("tasks".to_string(), JsonValue::Array(filtered_tasks));
        }

        // Metadata
        if options.include_metadata && !result.metadata.is_empty() {
            output.insert(
                "metadata".to_string(),
                serde_json::to_value(&result.metadata)?,
            );
        }

        // System information
        if options.include_system_info {
            let mut system_info = serde_json::Map::new();
            system_info.insert(
                "hostname".to_string(),
                JsonValue::String(
                    hostname::get()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                ),
            );
            system_info.insert(
                "timestamp".to_string(),
                JsonValue::String(Utc::now().to_rfc3339()),
            );
            output.insert("system_info".to_string(), JsonValue::Object(system_info));
        }

        Ok(JsonValue::Object(output))
    }

    async fn prepare_task_output(
        &self,
        result: &TaskResult,
        options: &OutputOptions,
    ) -> Result<JsonValue> {
        let mut output = serde_json::Map::new();

        // Basic task information
        output.insert(
            "task_id".to_string(),
            JsonValue::String(result.task_id.clone()),
        );
        output.insert(
            "task_type".to_string(),
            JsonValue::String(result.task_type.clone()),
        );
        output.insert(
            "status".to_string(),
            JsonValue::String(result.status.to_string()),
        );

        // Timestamps
        if options.include_timestamps {
            output.insert(
                "start_time".to_string(),
                JsonValue::String(result.start_time.to_rfc3339()),
            );
            if let Some(end_time) = result.end_time {
                output.insert(
                    "end_time".to_string(),
                    JsonValue::String(end_time.to_rfc3339()),
                );
            }
        }

        // Duration
        if options.include_duration {
            if let Some(duration) = result.duration {
                output.insert(
                    "duration_seconds".to_string(),
                    JsonValue::Number(
                        serde_json::Number::from_f64(duration.as_secs_f64()).unwrap(),
                    ),
                );
            }
        }

        // Output and error
        if let Some(ref task_output) = result.output {
            let output_text = if let Some(max_len) = options.max_output_length {
                if task_output.len() > max_len {
                    format!("{}... [truncated]", &task_output[..max_len])
                } else {
                    task_output.clone()
                }
            } else {
                task_output.clone()
            };
            output.insert("output".to_string(), JsonValue::String(output_text));
        }

        if let Some(ref error) = result.error {
            output.insert("error".to_string(), JsonValue::String(error.clone()));
        }

        // Retry information
        if result.retry_count > 0 {
            output.insert(
                "retry_count".to_string(),
                JsonValue::Number((result.retry_count as u64).into()),
            );
        }

        // Metadata
        if options.include_metadata && !result.metadata.is_empty() {
            output.insert(
                "metadata".to_string(),
                serde_json::to_value(&result.metadata)?,
            );
        }

        Ok(JsonValue::Object(output))
    }
}

#[async_trait]
impl OutputFormatter for YamlFormatter {
    async fn format_workflow_result(
        &self,
        result: &WorkflowResult,
        config: &OutputConfig,
    ) -> Result<String> {
        let json_formatter = JsonFormatter::new();
        let json_value = json_formatter
            .prepare_workflow_output(result, &config.options)
            .await?;
        serde_yaml::to_string(&json_value).map_err(OutputError::YamlSerializationError)
    }

    async fn format_task_result(
        &self,
        result: &TaskResult,
        config: &OutputConfig,
    ) -> Result<String> {
        let json_formatter = JsonFormatter::new();
        let json_value = json_formatter
            .prepare_task_output(result, &config.options)
            .await?;
        serde_yaml::to_string(&json_value).map_err(OutputError::YamlSerializationError)
    }
}

impl Default for YamlFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl YamlFormatter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl OutputFormatter for TextFormatter {
    async fn format_workflow_result(
        &self,
        result: &WorkflowResult,
        config: &OutputConfig,
    ) -> Result<String> {
        let mut output = String::new();

        // Header
        output.push_str(&format!(
            "Workflow: {} ({})\n",
            result.workflow_name, result.run_id
        ));
        output.push_str(&format!("Status: {}\n", result.status));

        if config.options.include_timestamps {
            output.push_str(&format!(
                "Started: {}\n",
                result.start_time.format("%Y-%m-%d %H:%M:%S UTC")
            ));
            if let Some(end_time) = result.end_time {
                output.push_str(&format!(
                    "Completed: {}\n",
                    end_time.format("%Y-%m-%d %H:%M:%S UTC")
                ));
            }
        }

        if config.options.include_duration {
            if let Some(duration) = result.duration {
                output.push_str(&format!("Duration: {:.2}s\n", duration.as_secs_f64()));
            }
        }

        // Summary
        output.push_str("\nSummary:\n");
        output.push_str(&format!("  Total tasks: {}\n", result.summary.total_tasks));
        output.push_str(&format!(
            "  Successful: {}\n",
            result.summary.successful_tasks
        ));
        output.push_str(&format!("  Failed: {}\n", result.summary.failed_tasks));
        output.push_str(&format!("  Skipped: {}\n", result.summary.skipped_tasks));
        output.push_str(&format!(
            "  Success rate: {:.1}%\n",
            result.summary.success_rate
        ));

        // Task results
        if config.options.include_task_results && !result.tasks.is_empty() {
            output.push_str("\nTasks:\n");

            for task in &result.tasks {
                // Apply status filters
                if config.options.filter_successful && !task.is_successful() {
                    continue;
                }
                if config.options.filter_failed && !task.is_failed() {
                    continue;
                }

                let task_text = self.format_task_result(task, config).await?;
                // Indent task output
                for line in task_text.lines() {
                    output.push_str(&format!("  {}\n", line));
                }
                output.push('\n');
            }
        }

        Ok(output)
    }

    async fn format_task_result(
        &self,
        result: &TaskResult,
        config: &OutputConfig,
    ) -> Result<String> {
        let mut output = String::new();

        // Status icon
        let status_icon = match result.status {
            TaskStatus::Success => "✓",
            TaskStatus::Failed | TaskStatus::Timeout | TaskStatus::Cancelled => "✗",
            TaskStatus::Skipped => "⊘",
            TaskStatus::Pending => "⧖",
            TaskStatus::Running => "⟳",
        };

        output.push_str(&format!(
            "{} {} ({})",
            status_icon, result.task_id, result.task_type
        ));

        if config.options.include_duration {
            if let Some(duration) = result.duration {
                output.push_str(&format!(" [{:.2}s]", duration.as_secs_f64()));
            }
        }

        if result.retry_count > 0 {
            output.push_str(&format!(" (retried {} times)", result.retry_count));
        }

        // Output or error on next line
        if let Some(ref task_output) = result.output {
            let display_output = if let Some(max_len) = config.options.max_output_length {
                if task_output.len() > max_len {
                    format!("{}... [truncated]", &task_output[..max_len])
                } else {
                    task_output.clone()
                }
            } else {
                task_output.clone()
            };

            if !display_output.is_empty() {
                output.push_str(&format!(
                    "\n    Output: {}",
                    display_output.replace('\n', "\n    ")
                ));
            }
        }

        if let Some(ref error) = result.error {
            output.push_str(&format!("\n    Error: {}", error));
        }

        Ok(output)
    }
}

impl Default for TextFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl TextFormatter {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_workflow_result() -> WorkflowResult {
        let mut result = WorkflowResult::new("test_workflow".to_string(), "run_123".to_string());

        let mut task_result = TaskResult::new("task_1".to_string(), "command".to_string());
        task_result.mark_completed(TaskStatus::Success, Some("Hello World".to_string()), None);

        result.add_task_result(task_result);
        result.mark_completed();

        result
    }

    #[tokio::test]
    async fn test_json_formatter() {
        let formatter = JsonFormatter::new();
        let result = create_test_workflow_result();
        let config = OutputConfig::default();

        let output = formatter
            .format_workflow_result(&result, &config)
            .await
            .unwrap();

        // Should be valid JSON
        let parsed: JsonValue = serde_json::from_str(&output).unwrap();
        assert!(parsed.get("workflow_name").is_some());
        assert!(parsed.get("status").is_some());
    }

    #[tokio::test]
    async fn test_yaml_formatter() {
        let formatter = YamlFormatter::new();
        let result = create_test_workflow_result();
        let config = OutputConfig::default();

        let output = formatter
            .format_workflow_result(&result, &config)
            .await
            .unwrap();

        // Should be valid YAML
        let parsed: serde_yaml::Value = serde_yaml::from_str(&output).unwrap();
        assert!(parsed.get("workflow_name").is_some());
        assert!(parsed.get("status").is_some());
    }

    #[tokio::test]
    async fn test_text_formatter() {
        let formatter = TextFormatter::new();
        let result = create_test_workflow_result();
        let config = OutputConfig::default();

        let output = formatter
            .format_workflow_result(&result, &config)
            .await
            .unwrap();

        assert!(output.contains("Workflow: test_workflow"));
        assert!(output.contains("Status:"));
        assert!(output.contains("Summary:"));
    }

    #[tokio::test]
    async fn test_output_filtering() {
        let formatter = JsonFormatter::new();
        let result = create_test_workflow_result();

        let mut config = OutputConfig::default();
        config.options.include_task_results = true; // Explicitly enable task results
        config.options.filter_successful = true;
        config.options.filter_failed = false;

        let output = formatter
            .format_workflow_result(&result, &config)
            .await
            .unwrap();
        let parsed: JsonValue = serde_json::from_str(&output).unwrap();

        // Should include successful tasks
        let tasks = parsed
            .get("tasks")
            .expect("tasks field should exist")
            .as_array()
            .unwrap();
        assert_eq!(tasks.len(), 1);
    }
}
