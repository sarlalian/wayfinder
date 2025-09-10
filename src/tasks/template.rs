// ABOUTME: Template task implementation for generating files from templates
// ABOUTME: Handles Handlebars template processing and file generation

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, error, info};

use super::TaskImplementation;
use crate::engine::error::{ExecutionError, Result};
use crate::engine::{ExecutionContext, TaskResult, TaskStatus};
use crate::template::TemplateEngine;

pub struct TemplateTask;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateConfig {
    pub template_path: Option<String>,
    pub template_content: Option<String>,
    pub output_path: String,
    #[serde(default)]
    pub variables: std::collections::HashMap<String, serde_yaml::Value>,
    #[serde(default = "default_create_dirs")]
    pub create_output_dirs: bool,
}

fn default_create_dirs() -> bool {
    true
}

#[async_trait]
impl TaskImplementation for TemplateTask {
    async fn execute(
        &self,
        task_id: String,
        config: serde_yaml::Value,
        context: ExecutionContext,
    ) -> Result<TaskResult> {
        let start_time = chrono::Utc::now();

        let config: TemplateConfig =
            serde_yaml::from_value(config).map_err(|e| ExecutionError::ConfigError {
                task_id: task_id.clone(),
                message: format!("Invalid template configuration: {}", e),
            })?;

        info!(
            "Executing template task: {} - generating {}",
            task_id, config.output_path
        );

        let mut task_result = TaskResult::new(task_id.clone(), "template".to_string());
        task_result.start_time = start_time;
        task_result.mark_started();

        // Get template content (either from file or inline)
        let template_content = if let Some(template_path) = &config.template_path {
            match std::fs::read_to_string(template_path) {
                Ok(content) => content,
                Err(e) => {
                    let error_msg =
                        format!("Failed to read template file {}: {}", template_path, e);
                    error!("{}", error_msg);
                    task_result.mark_completed(TaskStatus::Failed, None, Some(error_msg));
                    return Ok(task_result);
                }
            }
        } else if let Some(ref content) = config.template_content {
            content.clone()
        } else {
            let error_msg = "Either template_path or template_content must be provided";
            error!("{}", error_msg);
            task_result.mark_completed(TaskStatus::Failed, None, Some(error_msg.to_string()));
            return Ok(task_result);
        };

        // Create template engine
        let template_engine = match TemplateEngine::new() {
            Ok(engine) => engine,
            Err(e) => {
                let error_msg = format!("Failed to create template engine: {}", e);
                error!("{}", error_msg);
                task_result.mark_completed(TaskStatus::Failed, None, Some(error_msg));
                return Ok(task_result);
            }
        };

        // Create template context with additional variables
        let mut template_context_json = match context.template_context.to_json() {
            Ok(json) => json,
            Err(e) => {
                let error_msg = format!("Failed to convert template context to JSON: {}", e);
                error!("{}", error_msg);
                task_result.mark_completed(TaskStatus::Failed, None, Some(error_msg));
                return Ok(task_result);
            }
        };

        // Add additional variables from config
        if let serde_json::Value::Object(ref mut map) = template_context_json {
            for (key, value) in config.variables {
                if let Ok(json_value) = serde_json::to_value(value) {
                    map.insert(key, json_value);
                }
            }
        }

        // Render the template
        match template_engine.render_template(&template_content, &template_context_json) {
            Ok(rendered_content) => {
                debug!(
                    "Template rendered successfully, {} characters",
                    rendered_content.len()
                );

                // Create output directory if needed
                if config.create_output_dirs {
                    if let Some(parent) = Path::new(&config.output_path).parent() {
                        if let Err(e) = std::fs::create_dir_all(parent) {
                            let error_msg = format!("Failed to create output directory: {}", e);
                            error!("{}", error_msg);
                            task_result.mark_completed(TaskStatus::Failed, None, Some(error_msg));
                            return Ok(task_result);
                        }
                    }
                }

                // Write the rendered content to output file
                match std::fs::write(&config.output_path, &rendered_content) {
                    Ok(()) => {
                        let output = format!(
                            "Template rendered to {} ({} bytes)",
                            config.output_path,
                            rendered_content.len()
                        );
                        info!("{}", output);

                        // Add metadata
                        task_result.add_metadata("output_path".to_string(), config.output_path);
                        task_result.add_metadata(
                            "content_size".to_string(),
                            rendered_content.len().to_string(),
                        );
                        if let Some(template_path) = config.template_path {
                            task_result.add_metadata("template_path".to_string(), template_path);
                        }

                        task_result.mark_completed(TaskStatus::Success, Some(output), None);
                    }
                    Err(e) => {
                        let error_msg =
                            format!("Failed to write output file {}: {}", config.output_path, e);
                        error!("{}", error_msg);
                        task_result.mark_completed(TaskStatus::Failed, None, Some(error_msg));
                    }
                }
            }
            Err(e) => {
                let error_msg = format!("Template rendering failed: {}", e);
                error!("{}", error_msg);
                task_result.mark_completed(TaskStatus::Failed, None, Some(error_msg));
            }
        }

        Ok(task_result)
    }

    fn task_type(&self) -> &'static str {
        "template"
    }

    fn validate_config(&self, config: &serde_yaml::Value) -> Result<()> {
        let config: TemplateConfig =
            serde_yaml::from_value(config.clone()).map_err(|e| ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: format!("Invalid template configuration: {}", e),
            })?;

        if config.template_path.is_none() && config.template_content.is_none() {
            return Err(ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: "Either template_path or template_content must be provided".to_string(),
            });
        }

        if config.output_path.is_empty() {
            return Err(ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: "Output path cannot be empty".to_string(),
            });
        }

        // If template_path is provided, check if file exists
        if let Some(template_path) = &config.template_path {
            if !Path::new(template_path).exists() {
                return Err(ExecutionError::ConfigError {
                    task_id: "validation".to_string(),
                    message: format!("Template file does not exist: {}", template_path),
                });
            }
        }

        Ok(())
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
    async fn test_template_with_inline_content() {
        let task = TemplateTask;
        let variables = HashMap::new();
        let template_context = TemplateContext::new(&variables).unwrap();
        let context = ExecutionContext::new(
            "test_workflow".to_string(),
            "run_123".to_string(),
            "template_task".to_string(),
            template_context,
        );

        let temp_dir = TempDir::new().unwrap();
        let output_file = temp_dir.path().join("output.txt");

        let mut template_variables = HashMap::new();
        template_variables.insert(
            "name".to_string(),
            serde_yaml::Value::String("World".to_string()),
        );

        let config = serde_yaml::to_value(TemplateConfig {
            template_path: None,
            template_content: Some("Hello, {{name}}!".to_string()),
            output_path: output_file.to_string_lossy().to_string(),
            variables: template_variables,
            create_output_dirs: true,
        })
        .unwrap();

        let result = task
            .execute("test_template".to_string(), config, context)
            .await
            .unwrap();

        assert_eq!(result.status, TaskStatus::Success);
        assert!(output_file.exists());

        let content = fs::read_to_string(&output_file).unwrap();
        assert_eq!(content, "Hello, World!");
    }

    #[test]
    fn test_template_config_validation() {
        let task = TemplateTask;

        // Test missing both template_path and template_content
        let invalid_config = serde_yaml::to_value(TemplateConfig {
            template_path: None,
            template_content: None,
            output_path: "/tmp/output.txt".to_string(),
            variables: HashMap::new(),
            create_output_dirs: true,
        })
        .unwrap();

        assert!(task.validate_config(&invalid_config).is_err());

        // Test empty output path
        let empty_output_config = serde_yaml::to_value(TemplateConfig {
            template_path: None,
            template_content: Some("Hello".to_string()),
            output_path: "".to_string(),
            variables: HashMap::new(),
            create_output_dirs: true,
        })
        .unwrap();

        assert!(task.validate_config(&empty_output_config).is_err());

        // Test valid config with inline content
        let valid_config = serde_yaml::to_value(TemplateConfig {
            template_path: None,
            template_content: Some("Hello, {{name}}!".to_string()),
            output_path: "/tmp/output.txt".to_string(),
            variables: HashMap::new(),
            create_output_dirs: true,
        })
        .unwrap();

        assert!(task.validate_config(&valid_config).is_ok());
    }
}
