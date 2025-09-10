// ABOUTME: Report template management and rendering
// ABOUTME: Handles loading, parsing, and rendering report templates with variable substitution

use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;

use super::config::ReportTemplate;
use super::error::{ReportingError, Result};
use super::{GeneratedReport, ReportData};
use crate::template::TemplateEngine;

pub struct ReportTemplateManager {
    template_engine: TemplateEngine,
    template_cache: HashMap<String, CachedTemplate>,
    global_variables: HashMap<String, String>,
}

#[derive(Debug, Clone)]
struct CachedTemplate {
    content: String,
    last_modified: DateTime<Utc>,
    file_path: Option<String>,
}

impl ReportTemplateManager {
    pub fn new() -> Self {
        Self {
            template_engine: TemplateEngine::new().expect("Failed to initialize template engine"),
            template_cache: HashMap::new(),
            global_variables: HashMap::new(),
        }
    }

    pub fn with_global_variables(mut self, variables: HashMap<String, String>) -> Self {
        self.global_variables = variables;
        self
    }

    /// Load a template from a file and cache it
    pub async fn load_template(&mut self, template_name: &str, file_path: &str) -> Result<()> {
        let path = Path::new(file_path);

        if !path.exists() {
            return Err(ReportingError::TemplateError {
                message: format!("Template file not found: {}", file_path),
            });
        }

        let content = fs::read_to_string(path)
            .await
            .map_err(ReportingError::IoError)?;

        let metadata = fs::metadata(path)
            .await
            .map_err(ReportingError::IoError)?;

        let last_modified = metadata
            .modified()
            .map_err(ReportingError::IoError)?;

        self.template_cache.insert(
            template_name.to_string(),
            CachedTemplate {
                content,
                last_modified: last_modified.into(),
                file_path: Some(file_path.to_string()),
            },
        );

        Ok(())
    }

    /// Add a template from string content
    pub fn add_template(&mut self, template_name: &str, content: &str) {
        self.template_cache.insert(
            template_name.to_string(),
            CachedTemplate {
                content: content.to_string(),
                last_modified: Utc::now(),
                file_path: None,
            },
        );
    }

    /// Render a report using a template
    pub async fn render_report(
        &self,
        template: &ReportTemplate,
        data: &ReportData,
    ) -> Result<GeneratedReport> {
        let template_content = self.get_template_content(template).await?;
        let context = self.build_template_context(template, data)?;

        let context_value = serde_json::to_value(&context)?;
        let rendered_content = self
            .template_engine
            .render_template(&template_content, &context_value)
            .map_err(ReportingError::TemplateEngineError)?;

        Ok(GeneratedReport {
            report_type: template.generator_type.clone(),
            title: self.render_title(template, data, &context)?,
            content: rendered_content,
            metadata: self.build_metadata(template, data),
            generated_at: Utc::now(),
        })
    }

    /// Check if templates need to be reloaded from disk
    pub async fn refresh_templates(&mut self) -> Result<()> {
        let mut templates_to_reload = Vec::new();

        for (name, cached) in &self.template_cache {
            if let Some(file_path) = &cached.file_path {
                let path = Path::new(file_path);
                if path.exists() {
                    let metadata = fs::metadata(path)
                        .await
                        .map_err(ReportingError::IoError)?;

                    let file_modified: DateTime<Utc> = metadata
                        .modified()
                        .map_err(ReportingError::IoError)?
                        .into();

                    if file_modified > cached.last_modified {
                        templates_to_reload.push((name.clone(), file_path.clone()));
                    }
                }
            }
        }

        for (name, file_path) in templates_to_reload {
            self.load_template(&name, &file_path).await?;
        }

        Ok(())
    }

    /// Get available template names
    pub fn list_templates(&self) -> Vec<&str> {
        self.template_cache.keys().map(|k| k.as_str()).collect()
    }

    /// Remove a template from cache
    pub fn remove_template(&mut self, template_name: &str) -> bool {
        self.template_cache.remove(template_name).is_some()
    }

    /// Clear all cached templates
    pub fn clear_cache(&mut self) {
        self.template_cache.clear();
    }

    /// Set global variables that are available to all templates
    pub fn set_global_variables(&mut self, variables: HashMap<String, String>) {
        self.global_variables = variables;
    }

    /// Add a global variable
    pub fn add_global_variable(&mut self, key: String, value: String) {
        self.global_variables.insert(key, value);
    }

    async fn get_template_content(&self, template: &ReportTemplate) -> Result<String> {
        // Priority: inline content > template name lookup > template file
        if let Some(ref content) = template.template_content {
            return Ok(content.clone());
        }

        // Check cache for template by name
        if let Some(cached) = self.template_cache.get(&template.name) {
            return Ok(cached.content.clone());
        }

        // Try to load from file if specified
        if let Some(ref file_path) = template.template_file {
            let content =
                fs::read_to_string(file_path)
                    .await
                    .map_err(|e| ReportingError::TemplateError {
                        message: format!("Failed to read template file {}: {}", file_path, e),
                    })?;
            return Ok(content);
        }

        // No template found, return error
        Err(ReportingError::TemplateError {
            message: format!("No template content found for template '{}'", template.name),
        })
    }

    fn build_template_context(
        &self,
        template: &ReportTemplate,
        data: &ReportData,
    ) -> Result<HashMap<String, JsonValue>> {
        let mut context = HashMap::new();

        // Add global variables
        for (key, value) in &self.global_variables {
            context.insert(key.clone(), JsonValue::String(value.clone()));
        }

        // Add template-specific variables
        for (key, value) in &template.variables {
            context.insert(key.clone(), JsonValue::String(value.clone()));
        }

        // Add data-specific variables
        match data {
            ReportData::WorkflowCompletion { result, metrics } => {
                context.insert("workflow".to_string(), serde_json::to_value(result)?);
                context.insert("metrics".to_string(), serde_json::to_value(metrics)?);
                context.insert(
                    "workflow_name".to_string(),
                    JsonValue::String(result.workflow_name.clone()),
                );
                context.insert(
                    "run_id".to_string(),
                    JsonValue::String(result.run_id.clone()),
                );
                context.insert(
                    "status".to_string(),
                    JsonValue::String(result.status.to_string()),
                );
                context.insert(
                    "success_rate".to_string(),
                    JsonValue::Number(
                        serde_json::Number::from_f64(result.summary.success_rate)
                            .unwrap_or_else(|| serde_json::Number::from(0)),
                    ),
                );
                context.insert(
                    "total_tasks".to_string(),
                    JsonValue::Number(result.summary.total_tasks.into()),
                );
                context.insert(
                    "failed_tasks".to_string(),
                    JsonValue::Number(result.summary.failed_tasks.into()),
                );
            }
            ReportData::TaskCompletion {
                result,
                context: task_context,
            } => {
                context.insert("task".to_string(), serde_json::to_value(result)?);
                context.insert(
                    "workflow_context".to_string(),
                    serde_json::to_value(task_context)?,
                );
                context.insert(
                    "task_id".to_string(),
                    JsonValue::String(result.task_id.clone()),
                );
                context.insert(
                    "task_type".to_string(),
                    JsonValue::String(result.task_type.clone()),
                );
                context.insert(
                    "status".to_string(),
                    JsonValue::String(result.status.to_string()),
                );
                context.insert(
                    "workflow_name".to_string(),
                    JsonValue::String(task_context.workflow_name.clone()),
                );
                context.insert(
                    "run_id".to_string(),
                    JsonValue::String(task_context.run_id.clone()),
                );

                if let Some(ref error) = result.error {
                    context.insert("error".to_string(), JsonValue::String(error.clone()));
                }
                if let Some(ref output) = result.output {
                    context.insert("output".to_string(), JsonValue::String(output.clone()));
                }
            }
            ReportData::PeriodicMetrics {
                metrics,
                period,
                generated_at,
            } => {
                context.insert("metrics".to_string(), serde_json::to_value(metrics)?);
                context.insert(
                    "period".to_string(),
                    JsonValue::String(format!("{:?}", period)),
                );
                context.insert(
                    "generated_at".to_string(),
                    JsonValue::String(generated_at.to_rfc3339()),
                );
                context.insert(
                    "total_workflows".to_string(),
                    JsonValue::Number(metrics.total_workflows.into()),
                );
                context.insert(
                    "successful_workflows".to_string(),
                    JsonValue::Number(metrics.successful_workflows.into()),
                );
                context.insert(
                    "failed_workflows".to_string(),
                    JsonValue::Number(metrics.failed_workflows.into()),
                );
                context.insert(
                    "workflow_success_rate".to_string(),
                    JsonValue::Number(
                        serde_json::Number::from_f64(metrics.workflow_success_rate)
                            .unwrap_or_else(|| serde_json::Number::from(0)),
                    ),
                );
                context.insert(
                    "total_tasks".to_string(),
                    JsonValue::Number(metrics.total_tasks.into()),
                );
                context.insert(
                    "task_success_rate".to_string(),
                    JsonValue::Number(
                        serde_json::Number::from_f64(metrics.task_success_rate)
                            .unwrap_or_else(|| serde_json::Number::from(0)),
                    ),
                );
            }
        }

        // Add common variables
        context.insert(
            "timestamp".to_string(),
            JsonValue::String(Utc::now().to_rfc3339()),
        );
        context.insert(
            "date".to_string(),
            JsonValue::String(Utc::now().format("%Y-%m-%d").to_string()),
        );
        context.insert(
            "time".to_string(),
            JsonValue::String(Utc::now().format("%H:%M:%S").to_string()),
        );
        context.insert(
            "template_name".to_string(),
            JsonValue::String(template.name.clone()),
        );
        context.insert(
            "generator_type".to_string(),
            JsonValue::String(template.generator_type.clone()),
        );

        Ok(context)
    }

    fn render_title(
        &self,
        template: &ReportTemplate,
        data: &ReportData,
        context: &HashMap<String, JsonValue>,
    ) -> Result<String> {
        // Simple title template rendering using string replacement
        let default_title = match data {
            ReportData::WorkflowCompletion { result, .. } => {
                format!(
                    "{} - Workflow {} Report",
                    result.workflow_name, result.status
                )
            }
            ReportData::TaskCompletion {
                result,
                context: task_context,
            } => {
                format!(
                    "{} - Task {} Report",
                    task_context.workflow_name, result.status
                )
            }
            ReportData::PeriodicMetrics { period, .. } => {
                format!("{:?} Metrics Report", period)
            }
        };

        // Check if there's a custom title template
        if let Some(title_var) = context.get("title") {
            if let Some(title_str) = title_var.as_str() {
                return Ok(title_str.to_string());
            }
        }

        // Use template name as prefix if it's descriptive
        if !template.name.is_empty() && template.name != "default" {
            Ok(format!("{} - {}", template.name, default_title))
        } else {
            Ok(default_title)
        }
    }

    fn build_metadata(
        &self,
        template: &ReportTemplate,
        data: &ReportData,
    ) -> HashMap<String, String> {
        let mut metadata = HashMap::new();

        metadata.insert("template_name".to_string(), template.name.clone());
        metadata.insert(
            "generator_type".to_string(),
            template.generator_type.clone(),
        );
        metadata.insert("generated_at".to_string(), Utc::now().to_rfc3339());

        match data {
            ReportData::WorkflowCompletion { result, .. } => {
                metadata.insert("data_type".to_string(), "workflow_completion".to_string());
                metadata.insert("workflow_name".to_string(), result.workflow_name.clone());
                metadata.insert("run_id".to_string(), result.run_id.clone());
            }
            ReportData::TaskCompletion { result, context } => {
                metadata.insert("data_type".to_string(), "task_completion".to_string());
                metadata.insert("task_id".to_string(), result.task_id.clone());
                metadata.insert("workflow_name".to_string(), context.workflow_name.clone());
            }
            ReportData::PeriodicMetrics { period, .. } => {
                metadata.insert("data_type".to_string(), "periodic_metrics".to_string());
                metadata.insert("period".to_string(), format!("{:?}", period));
            }
        }

        // Add template variables as metadata
        for (key, value) in &template.variables {
            metadata.insert(format!("template_var_{}", key), value.clone());
        }

        metadata
    }
}

impl Default for ReportTemplateManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Pre-defined report templates for common use cases
pub struct DefaultTemplates;

impl DefaultTemplates {
    /// Get a simple workflow summary template
    pub fn workflow_summary_template() -> String {
        r#"
# Workflow Summary: {{ workflow_name }}

**Status:** {{ status }}
**Run ID:** {{ run_id }}
**Started:** {{ workflow.start_time }}
{% if workflow.end_time %}**Completed:** {{ workflow.end_time }}{% endif %}
{% if workflow.duration %}**Duration:** {{ workflow.duration.as_secs_f64() | round(2) }} seconds{% endif %}

## Task Summary
- **Total Tasks:** {{ total_tasks }}
- **Successful:** {{ workflow.summary.successful_tasks }} ({{ success_rate | round(1) }}%)
- **Failed:** {{ failed_tasks }}
- **Skipped:** {{ workflow.summary.skipped_tasks }}

{% if workflow.tasks | selectattr("status", "equalto", "Failed") | list | length > 0 %}
## Failed Tasks
{% for task in workflow.tasks %}
{% if task.status == "Failed" %}
- **{{ task.task_id }}** ({{ task.task_type }}): {{ task.error | default("Unknown error") }}
{% endif %}
{% endfor %}
{% endif %}
        "#.trim().to_string()
    }

    /// Get a detailed workflow report template  
    pub fn workflow_detailed_template() -> String {
        r#"
# Detailed Workflow Report: {{ workflow_name }}

## Overview
- **Workflow:** {{ workflow_name }}
- **Run ID:** {{ run_id }}
- **Status:** {{ status }}
- **Started:** {{ workflow.start_time }}
{% if workflow.end_time %}
- **Completed:** {{ workflow.end_time }}
{% endif %}
{% if workflow.duration %}
- **Duration:** {{ workflow.duration.as_secs_f64() | round(2) }} seconds
{% endif %}

## Statistics
- **Total Tasks:** {{ total_tasks }}
- **Success Rate:** {{ success_rate | round(1) }}%
- **Failed Tasks:** {{ failed_tasks }}
- **Skipped Tasks:** {{ workflow.summary.skipped_tasks }}

## Task Details
{% for task in workflow.tasks %}
### {{ task.task_id }} ({{ task.task_type }})
- **Status:** {{ task.status }}
- **Duration:** {% if task.duration %}{{ task.duration.as_secs_f64() | round(2) }}s{% else %}N/A{% endif %}
{% if task.retry_count > 0 %}
- **Retries:** {{ task.retry_count }}
{% endif %}
{% if task.output %}
- **Output:** 
```
{{ task.output }}
```
{% endif %}
{% if task.error %}
- **Error:** 
```
{{ task.error }}
```
{% endif %}

{% endfor %}
        "#.trim().to_string()
    }

    /// Get a task failure notification template
    pub fn task_failure_template() -> String {
        r#"
# Task Failed: {{ task_id }}

**Task:** {{ task_id }}
**Type:** {{ task_type }}
**Workflow:** {{ workflow_name }} ({{ run_id }})
**Status:** {{ status }}
**Failed at:** {{ task.end_time | default(task.start_time) }}

{% if task.retry_count > 0 %}
**Retries:** {{ task.retry_count }}
{% endif %}

## Error Details
```
{{ error | default("No error message available") }}
```

{% if output %}
## Task Output
```
{{ output }}
```
{% endif %}
        "#
        .trim()
        .to_string()
    }

    /// Get a periodic metrics report template
    pub fn metrics_summary_template() -> String {
        r#"
# {{ period }} Metrics Report

**Period:** {{ metrics.period_start }} to {{ metrics.period_end }}
**Generated:** {{ timestamp }}

## Workflow Statistics
- **Total Workflows:** {{ total_workflows }}
- **Successful:** {{ successful_workflows }} ({{ workflow_success_rate | round(1) }}%)
- **Failed:** {{ failed_workflows }}
{% if metrics.avg_workflow_duration %}
- **Average Duration:** {{ metrics.avg_workflow_duration.as_secs_f64() | round(1) }}s
{% endif %}

## Task Statistics  
- **Total Tasks:** {{ total_tasks }}
- **Successful:** {{ metrics.successful_tasks }} ({{ task_success_rate | round(1) }}%)
- **Failed:** {{ metrics.failed_tasks }}
- **Skipped:** {{ metrics.skipped_tasks }}
- **Cancelled:** {{ metrics.cancelled_tasks }}
{% if metrics.avg_task_duration %}
- **Average Duration:** {{ metrics.avg_task_duration.as_secs_f64() | round(1) }}s
{% endif %}
        "#
        .trim()
        .to_string()
    }

    /// Initialize a template manager with default templates
    pub async fn setup_default_templates(manager: &mut ReportTemplateManager) {
        manager.add_template("workflow_summary", &Self::workflow_summary_template());
        manager.add_template("workflow_detailed", &Self::workflow_detailed_template());
        manager.add_template("task_failure", &Self::task_failure_template());
        manager.add_template("metrics_summary", &Self::metrics_summary_template());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::result::WorkflowResult;

    use tempfile::TempDir;

    #[tokio::test]
    async fn test_template_manager_basic() {
        let mut manager = ReportTemplateManager::new();

        let template_content = "Hello {{ name }}!";
        manager.add_template("greeting", template_content);

        assert_eq!(manager.list_templates(), vec!["greeting"]);
        assert!(manager.remove_template("greeting"));
        assert!(manager.list_templates().is_empty());
    }

    #[tokio::test]
    async fn test_template_file_loading() {
        let temp_dir = TempDir::new().unwrap();
        let template_file = temp_dir.path().join("test_template.txt");

        fs::write(&template_file, "Workflow: {{ workflow_name }}")
            .await
            .unwrap();

        let mut manager = ReportTemplateManager::new();
        manager
            .load_template("test", template_file.to_str().unwrap())
            .await
            .unwrap();

        assert!(manager.list_templates().contains(&"test"));
    }

    #[tokio::test]
    async fn test_template_context_building() {
        let manager = ReportTemplateManager::new();
        let template = ReportTemplate::new("test".to_string(), "summary".to_string());

        let mut workflow_result =
            WorkflowResult::new("test_workflow".to_string(), "run_1".to_string());
        workflow_result.mark_completed();

        let metrics =
            super::super::metrics::WorkflowMetrics::from_workflow_result(&workflow_result);
        let data = ReportData::WorkflowCompletion {
            result: workflow_result,
            metrics,
        };

        let context = manager.build_template_context(&template, &data).unwrap();

        assert!(context.contains_key("workflow_name"));
        assert!(context.contains_key("status"));
        assert!(context.contains_key("timestamp"));
    }

    #[tokio::test]
    async fn test_default_templates() {
        let mut manager = ReportTemplateManager::new();
        DefaultTemplates::setup_default_templates(&mut manager).await;

        let templates = manager.list_templates();
        assert!(templates.contains(&"workflow_summary"));
        assert!(templates.contains(&"workflow_detailed"));
        assert!(templates.contains(&"task_failure"));
        assert!(templates.contains(&"metrics_summary"));
    }

    #[test]
    fn test_template_content() {
        let summary_template = DefaultTemplates::workflow_summary_template();
        assert!(summary_template.contains("{{ workflow_name }}"));
        assert!(summary_template.contains("{{ status }}"));
        assert!(summary_template.contains("{{ total_tasks }}"));

        let detailed_template = DefaultTemplates::workflow_detailed_template();
        assert!(detailed_template.contains("## Task Details"));
        assert!(detailed_template.contains("{% for task in workflow.tasks %}"));
    }
}
