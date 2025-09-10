// ABOUTME: Report generators for different report types and formats
// ABOUTME: Handles creation of summary, detailed, and metrics reports from workflow data

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

use super::config::{ReportFilters, ReportTemplate};
use super::error::{ReportingError, Result};
use super::metrics::{ExecutionMetrics, WorkflowMetrics};
use super::{GeneratedReport, ReportData, ReportPeriod};
use crate::engine::{TaskResult, TaskStatus, WorkflowResult};

#[async_trait]
pub trait ReportGenerator: Send + Sync {
    async fn generate_report(
        &self,
        template: &ReportTemplate,
        data: &ReportData,
    ) -> Result<GeneratedReport>;

    fn supported_data_types(&self) -> Vec<&'static str>;
    fn get_generator_info(&self) -> GeneratorInfo;
}

#[derive(Debug, Clone)]
pub struct GeneratorInfo {
    pub name: String,
    pub description: String,
    pub supported_formats: Vec<String>,
    pub template_required: bool,
}

pub struct SummaryReportGenerator {
    name: String,
}

pub struct DetailedReportGenerator {
    name: String,
}

pub struct MetricsReportGenerator {
    name: String,
}

pub struct CustomReportGenerator {
    _name: String,
    _template_engine: crate::template::TemplateEngine,
}

impl SummaryReportGenerator {
    pub fn new() -> Self {
        Self {
            name: "Summary Report Generator".to_string(),
        }
    }
}

#[async_trait]
impl ReportGenerator for SummaryReportGenerator {
    async fn generate_report(
        &self,
        template: &ReportTemplate,
        data: &ReportData,
    ) -> Result<GeneratedReport> {
        let content = match data {
            ReportData::WorkflowCompletion { result, metrics } => {
                self.generate_workflow_summary(result, metrics, template)
                    .await?
            }
            ReportData::TaskCompletion { result, context } => {
                self.generate_task_summary(result, context, template)
                    .await?
            }
            ReportData::PeriodicMetrics {
                metrics,
                period,
                generated_at,
            } => {
                self.generate_periodic_summary(metrics, period, *generated_at, template)
                    .await?
            }
        };

        Ok(GeneratedReport {
            report_type: "summary".to_string(),
            title: self.generate_title(data, template),
            content,
            metadata: self.generate_metadata(data, template),
            generated_at: Utc::now(),
        })
    }

    fn supported_data_types(&self) -> Vec<&'static str> {
        vec!["workflow_completion", "task_completion", "periodic_metrics"]
    }

    fn get_generator_info(&self) -> GeneratorInfo {
        GeneratorInfo {
            name: self.name.clone(),
            description: "Generates concise summary reports".to_string(),
            supported_formats: vec!["text".to_string(), "markdown".to_string()],
            template_required: false,
        }
    }
}

impl SummaryReportGenerator {
    async fn generate_workflow_summary(
        &self,
        result: &WorkflowResult,
        _metrics: &WorkflowMetrics,
        _template: &ReportTemplate,
    ) -> Result<String> {
        let status_icon = match result.status.to_string().as_str() {
            "Success" => "✅",
            "Failed" => "❌",
            "Cancelled" => "⚠️",
            _ => "ℹ️",
        };

        let duration_text = if let Some(duration) = result.duration {
            format!(" (took {:.1}s)", duration.as_secs_f64())
        } else {
            String::new()
        };

        let mut content = format!(
            "{} **Workflow Summary**\n\n\
            **Workflow:** {}\n\
            **Run ID:** {}\n\
            **Status:** {}{}\n\
            **Started:** {}\n",
            status_icon,
            result.workflow_name,
            result.run_id,
            result.status,
            duration_text,
            result.start_time.format("%Y-%m-%d %H:%M:%S UTC")
        );

        if let Some(end_time) = result.end_time {
            content.push_str(&format!(
                "**Completed:** {}\n",
                end_time.format("%Y-%m-%d %H:%M:%S UTC")
            ));
        }

        content.push_str(&format!(
            "\n**Task Summary:**\n\
            - Total: {}\n\
            - Successful: {} ({:.1}%)\n\
            - Failed: {}\n\
            - Skipped: {}\n",
            result.summary.total_tasks,
            result.summary.successful_tasks,
            result.summary.success_rate,
            result.summary.failed_tasks,
            result.summary.skipped_tasks
        ));

        // Add failed tasks if any
        let failed_tasks: Vec<_> = result
            .tasks
            .iter()
            .filter(|t| {
                matches!(
                    t.status,
                    TaskStatus::Failed | TaskStatus::Timeout | TaskStatus::Cancelled
                )
            })
            .collect();

        if !failed_tasks.is_empty() {
            content.push_str("\n**Failed Tasks:**\n");
            for task in failed_tasks.iter().take(5) {
                // Show first 5 failed tasks
                content.push_str(&format!(
                    "- {} ({}): {}\n",
                    task.task_id,
                    task.task_type,
                    task.error.as_deref().unwrap_or("Unknown error")
                ));
            }
            if failed_tasks.len() > 5 {
                content.push_str(&format!("- ... and {} more\n", failed_tasks.len() - 5));
            }
        }

        Ok(content)
    }

    async fn generate_task_summary(
        &self,
        result: &TaskResult,
        context: &super::WorkflowContext,
        _template: &ReportTemplate,
    ) -> Result<String> {
        let status_icon = match result.status {
            TaskStatus::Success => "✅",
            TaskStatus::Failed | TaskStatus::Timeout | TaskStatus::Cancelled => "❌",
            TaskStatus::Skipped => "⚠️",
            _ => "ℹ️",
        };

        let duration_text = if let Some(duration) = result.duration {
            format!(" (took {:.1}s)", duration.as_secs_f64())
        } else {
            String::new()
        };

        let mut content = format!(
            "{} **Task Summary**\n\n\
            **Task:** {}\n\
            **Type:** {}\n\
            **Workflow:** {} ({})\n\
            **Status:** {}{}\n\
            **Started:** {}\n",
            status_icon,
            result.task_id,
            result.task_type,
            context.workflow_name,
            context.run_id,
            result.status,
            duration_text,
            result.start_time.format("%Y-%m-%d %H:%M:%S UTC")
        );

        if let Some(end_time) = result.end_time {
            content.push_str(&format!(
                "**Completed:** {}\n",
                end_time.format("%Y-%m-%d %H:%M:%S UTC")
            ));
        }

        if result.retry_count > 0 {
            content.push_str(&format!("**Retries:** {}\n", result.retry_count));
        }

        if let Some(ref output) = result.output {
            let display_output = if output.len() > 200 {
                format!("{}...", &output[..200])
            } else {
                output.clone()
            };
            content.push_str(&format!("\n**Output:**\n```\n{}\n```\n", display_output));
        }

        if let Some(ref error) = result.error {
            content.push_str(&format!("\n**Error:**\n```\n{}\n```\n", error));
        }

        Ok(content)
    }

    async fn generate_periodic_summary(
        &self,
        metrics: &ExecutionMetrics,
        period: &ReportPeriod,
        generated_at: DateTime<Utc>,
        _template: &ReportTemplate,
    ) -> Result<String> {
        let period_name = match period {
            ReportPeriod::Hourly => "Hourly",
            ReportPeriod::Daily => "Daily",
            ReportPeriod::Weekly => "Weekly",
            ReportPeriod::Monthly => "Monthly",
        };

        let mut content = format!(
            "📊 **{} Metrics Summary**\n\n\
            **Period:** {} to {}\n\
            **Generated:** {}\n\n",
            period_name,
            metrics.period_start.format("%Y-%m-%d %H:%M:%S UTC"),
            metrics.period_end.format("%Y-%m-%d %H:%M:%S UTC"),
            generated_at.format("%Y-%m-%d %H:%M:%S UTC")
        );

        content.push_str(&format!(
            "**Workflow Statistics:**\n\
            - Total workflows: {}\n\
            - Successful: {} ({:.1}%)\n\
            - Failed: {}\n",
            metrics.total_workflows,
            metrics.successful_workflows,
            metrics.workflow_success_rate,
            metrics.failed_workflows
        ));

        if let Some(avg_duration) = metrics.avg_workflow_duration {
            content.push_str(&format!(
                "- Average duration: {:.1}s\n",
                avg_duration.num_seconds() as f64
            ));
        }

        content.push_str(&format!(
            "\n**Task Statistics:**\n\
            - Total tasks: {}\n\
            - Successful: {} ({:.1}%)\n\
            - Failed: {}\n\
            - Skipped: {}\n\
            - Cancelled: {}\n",
            metrics.total_tasks,
            metrics.successful_tasks,
            metrics.task_success_rate,
            metrics.failed_tasks,
            metrics.skipped_tasks,
            metrics.cancelled_tasks
        ));

        if let Some(avg_duration) = metrics.avg_task_duration {
            content.push_str(&format!(
                "- Average duration: {:.1}s\n",
                avg_duration.num_seconds() as f64
            ));
        }

        Ok(content)
    }

    fn generate_title(&self, data: &ReportData, template: &ReportTemplate) -> String {
        match data {
            ReportData::WorkflowCompletion { result, .. } => {
                format!(
                    "Workflow Summary: {} - {}",
                    result.workflow_name, result.status
                )
            }
            ReportData::TaskCompletion { result, context } => {
                format!(
                    "Task Summary: {} in {} - {}",
                    result.task_id, context.workflow_name, result.status
                )
            }
            ReportData::PeriodicMetrics { period, .. } => {
                let period_name = match period {
                    ReportPeriod::Hourly => "Hourly",
                    ReportPeriod::Daily => "Daily",
                    ReportPeriod::Weekly => "Weekly",
                    ReportPeriod::Monthly => "Monthly",
                };
                format!("{} {} Report", period_name, template.name)
            }
        }
    }

    fn generate_metadata(
        &self,
        data: &ReportData,
        template: &ReportTemplate,
    ) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        metadata.insert("generator".to_string(), "summary".to_string());
        metadata.insert("template".to_string(), template.name.clone());

        match data {
            ReportData::WorkflowCompletion { result, .. } => {
                metadata.insert("workflow_name".to_string(), result.workflow_name.clone());
                metadata.insert("run_id".to_string(), result.run_id.clone());
                metadata.insert("status".to_string(), result.status.to_string());
            }
            ReportData::TaskCompletion { result, context } => {
                metadata.insert("task_id".to_string(), result.task_id.clone());
                metadata.insert("task_type".to_string(), result.task_type.clone());
                metadata.insert("workflow_name".to_string(), context.workflow_name.clone());
                metadata.insert("status".to_string(), result.status.to_string());
            }
            ReportData::PeriodicMetrics { period, .. } => {
                metadata.insert("period".to_string(), format!("{:?}", period));
            }
        }

        metadata
    }
}

impl DetailedReportGenerator {
    pub fn new() -> Self {
        Self {
            name: "Detailed Report Generator".to_string(),
        }
    }
}

#[async_trait]
impl ReportGenerator for DetailedReportGenerator {
    async fn generate_report(
        &self,
        template: &ReportTemplate,
        data: &ReportData,
    ) -> Result<GeneratedReport> {
        let content = match data {
            ReportData::WorkflowCompletion { result, metrics } => {
                self.generate_detailed_workflow_report(result, metrics, template)
                    .await?
            }
            ReportData::TaskCompletion { result, context } => {
                self.generate_detailed_task_report(result, context, template)
                    .await?
            }
            ReportData::PeriodicMetrics {
                metrics,
                period,
                generated_at,
            } => {
                self.generate_detailed_periodic_report(metrics, period, *generated_at, template)
                    .await?
            }
        };

        Ok(GeneratedReport {
            report_type: "detailed".to_string(),
            title: self.generate_title(data, template),
            content,
            metadata: self.generate_metadata(data, template),
            generated_at: Utc::now(),
        })
    }

    fn supported_data_types(&self) -> Vec<&'static str> {
        vec!["workflow_completion", "task_completion", "periodic_metrics"]
    }

    fn get_generator_info(&self) -> GeneratorInfo {
        GeneratorInfo {
            name: self.name.clone(),
            description: "Generates comprehensive detailed reports with full task information"
                .to_string(),
            supported_formats: vec![
                "text".to_string(),
                "markdown".to_string(),
                "html".to_string(),
            ],
            template_required: false,
        }
    }
}

impl DetailedReportGenerator {
    async fn generate_detailed_workflow_report(
        &self,
        result: &WorkflowResult,
        _metrics: &WorkflowMetrics,
        template: &ReportTemplate,
    ) -> Result<String> {
        let mut content = String::new();

        // Header
        content.push_str(&format!(
            "# Workflow Execution Report\n\n\
            ## Overview\n\
            - **Workflow Name:** {}\n\
            - **Run ID:** {}\n\
            - **Status:** {}\n\
            - **Started:** {}\n",
            result.workflow_name,
            result.run_id,
            result.status,
            result.start_time.format("%Y-%m-%d %H:%M:%S UTC")
        ));

        if let Some(end_time) = result.end_time {
            content.push_str(&format!(
                "- **Completed:** {}\n",
                end_time.format("%Y-%m-%d %H:%M:%S UTC")
            ));
        }

        if let Some(duration) = result.duration {
            content.push_str(&format!(
                "- **Duration:** {:.2} seconds\n",
                duration.as_secs_f64()
            ));
        }

        // Summary statistics
        content.push_str(&format!(
            "\n## Task Statistics\n\
            - **Total Tasks:** {}\n\
            - **Successful:** {} ({:.1}%)\n\
            - **Failed:** {}\n\
            - **Skipped:** {}\n\
            - **Success Rate:** {:.1}%\n",
            result.summary.total_tasks,
            result.summary.successful_tasks,
            result.summary.success_rate,
            result.summary.failed_tasks,
            result.summary.skipped_tasks,
            result.summary.success_rate
        ));

        // Task details
        if !result.tasks.is_empty() {
            content.push_str("\n## Task Details\n\n");

            for task in &result.tasks {
                // Apply filters if configured
                if !self.should_include_task(task, &template.filters) {
                    continue;
                }

                let status_icon = match task.status {
                    TaskStatus::Success => "✅",
                    TaskStatus::Failed | TaskStatus::Timeout | TaskStatus::Cancelled => "❌",
                    TaskStatus::Skipped => "⚠️",
                    TaskStatus::Pending => "⏳",
                    TaskStatus::Running => "🔄",
                };

                content.push_str(&format!(
                    "### {} {} ({})\n",
                    status_icon, task.task_id, task.task_type
                ));
                content.push_str(&format!(
                    "- **Status:** {}\n\
                    - **Started:** {}\n",
                    task.status,
                    task.start_time.format("%Y-%m-%d %H:%M:%S UTC")
                ));

                if let Some(end_time) = task.end_time {
                    content.push_str(&format!(
                        "- **Completed:** {}\n",
                        end_time.format("%Y-%m-%d %H:%M:%S UTC")
                    ));
                }

                if let Some(duration) = task.duration {
                    content.push_str(&format!(
                        "- **Duration:** {:.2} seconds\n",
                        duration.as_secs_f64()
                    ));
                }

                if task.retry_count > 0 {
                    content.push_str(&format!("- **Retries:** {}\n", task.retry_count));
                }

                if let Some(ref output) = task.output {
                    content.push_str(&format!("\n**Output:**\n```\n{}\n```\n", output));
                }

                if let Some(ref error) = task.error {
                    content.push_str(&format!("\n**Error:**\n```\n{}\n```\n", error));
                }

                if !task.metadata.is_empty() {
                    content.push_str("\n**Metadata:**\n");
                    for (key, value) in &task.metadata {
                        content.push_str(&format!("- {}: {}\n", key, value));
                    }
                }

                content.push('\n');
            }
        }

        // Workflow metadata
        if !result.metadata.is_empty() {
            content.push_str("## Workflow Metadata\n");
            for (key, value) in &result.metadata {
                content.push_str(&format!("- {}: {}\n", key, value));
            }
        }

        Ok(content)
    }

    async fn generate_detailed_task_report(
        &self,
        result: &TaskResult,
        context: &super::WorkflowContext,
        _template: &ReportTemplate,
    ) -> Result<String> {
        let status_icon = match result.status {
            TaskStatus::Success => "✅",
            TaskStatus::Failed | TaskStatus::Timeout | TaskStatus::Cancelled => "❌",
            TaskStatus::Skipped => "⚠️",
            TaskStatus::Pending => "⏳",
            TaskStatus::Running => "🔄",
        };

        let mut content = format!(
            "# Task Execution Report\n\n\
            ## {} Task: {}\n\n\
            ### Overview\n\
            - **Task ID:** {}\n\
            - **Task Type:** {}\n\
            - **Status:** {}\n\
            - **Workflow:** {} (Run: {})\n\
            - **Started:** {}\n",
            status_icon,
            result.task_id,
            result.task_id,
            result.task_type,
            result.status,
            context.workflow_name,
            context.run_id,
            result.start_time.format("%Y-%m-%d %H:%M:%S UTC")
        );

        if let Some(end_time) = result.end_time {
            content.push_str(&format!(
                "- **Completed:** {}\n",
                end_time.format("%Y-%m-%d %H:%M:%S UTC")
            ));
        }

        if let Some(duration) = result.duration {
            content.push_str(&format!(
                "- **Duration:** {:.2} seconds\n",
                duration.as_secs_f64()
            ));
        }

        if result.retry_count > 0 {
            content.push_str(&format!("- **Retry Count:** {}\n", result.retry_count));
        }

        // Execution details
        content.push_str("\n### Execution Details\n");

        if let Some(ref output) = result.output {
            content.push_str(&format!("\n**Output:**\n```\n{}\n```\n", output));
        }

        if let Some(ref error) = result.error {
            content.push_str(&format!("\n**Error Details:**\n```\n{}\n```\n", error));
        }

        // Task metadata
        if !result.metadata.is_empty() {
            content.push_str("\n### Task Metadata\n");
            for (key, value) in &result.metadata {
                content.push_str(&format!("- **{}:** {}\n", key, value));
            }
        }

        // Context metadata
        if !context.metadata.is_empty() {
            content.push_str("\n### Workflow Context\n");
            for (key, value) in &context.metadata {
                content.push_str(&format!("- **{}:** {}\n", key, value));
            }
        }

        Ok(content)
    }

    async fn generate_detailed_periodic_report(
        &self,
        _metrics: &ExecutionMetrics,
        _period: &ReportPeriod,
        _generated_at: DateTime<Utc>,
        _template: &ReportTemplate,
    ) -> Result<String> {
        // For now, delegate to summary generator for periodic reports
        // This could be expanded to include more detailed breakdowns
        let summary_generator = SummaryReportGenerator::new();
        let _data = ReportData::PeriodicMetrics {
            metrics: _metrics.clone(),
            period: _period.clone(),
            generated_at: _generated_at,
        };
        summary_generator
            .generate_periodic_summary(_metrics, _period, _generated_at, _template)
            .await
    }

    fn should_include_task(&self, task: &TaskResult, filters: &ReportFilters) -> bool {
        // Check status filter
        if let Some(ref statuses) = filters.task_status {
            if !statuses.contains(&task.status.to_string()) {
                return false;
            }
        }

        // Check task type filter
        if let Some(ref types) = filters.task_types {
            if !types.contains(&task.task_type) {
                return false;
            }
        }

        // Check duration filters
        if let Some(duration) = task.duration {
            let duration_secs = duration.as_secs();

            if let Some(min_duration) = filters.min_duration_seconds {
                if duration_secs < min_duration {
                    return false;
                }
            }

            if let Some(max_duration) = filters.max_duration_seconds {
                if duration_secs > max_duration {
                    return false;
                }
            }
        }

        // Check tag filters
        if let Some(ref required_tags) = filters.tags {
            for (key, value) in required_tags {
                if let Some(task_value) = task.metadata.get(key) {
                    if task_value != value {
                        return false;
                    }
                } else {
                    return false;
                }
            }
        }

        true
    }

    fn generate_title(&self, data: &ReportData, template: &ReportTemplate) -> String {
        match data {
            ReportData::WorkflowCompletion { result, .. } => {
                format!(
                    "Detailed Workflow Report: {} - {}",
                    result.workflow_name, result.status
                )
            }
            ReportData::TaskCompletion { result, context } => {
                format!(
                    "Detailed Task Report: {} in {} - {}",
                    result.task_id, context.workflow_name, result.status
                )
            }
            ReportData::PeriodicMetrics { period, .. } => {
                let period_name = match period {
                    ReportPeriod::Hourly => "Hourly",
                    ReportPeriod::Daily => "Daily",
                    ReportPeriod::Weekly => "Weekly",
                    ReportPeriod::Monthly => "Monthly",
                };
                format!("Detailed {} {} Report", period_name, template.name)
            }
        }
    }

    fn generate_metadata(
        &self,
        data: &ReportData,
        template: &ReportTemplate,
    ) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        metadata.insert("generator".to_string(), "detailed".to_string());
        metadata.insert("template".to_string(), template.name.clone());

        match data {
            ReportData::WorkflowCompletion { result, .. } => {
                metadata.insert("workflow_name".to_string(), result.workflow_name.clone());
                metadata.insert("run_id".to_string(), result.run_id.clone());
                metadata.insert("task_count".to_string(), result.tasks.len().to_string());
            }
            ReportData::TaskCompletion { result, context } => {
                metadata.insert("task_id".to_string(), result.task_id.clone());
                metadata.insert("task_type".to_string(), result.task_type.clone());
                metadata.insert("workflow_name".to_string(), context.workflow_name.clone());
            }
            ReportData::PeriodicMetrics { period, .. } => {
                metadata.insert("period".to_string(), format!("{:?}", period));
            }
        }

        metadata
    }
}

impl MetricsReportGenerator {
    pub fn new() -> Self {
        Self {
            name: "Metrics Report Generator".to_string(),
        }
    }
}

#[async_trait]
impl ReportGenerator for MetricsReportGenerator {
    async fn generate_report(
        &self,
        template: &ReportTemplate,
        data: &ReportData,
    ) -> Result<GeneratedReport> {
        let content = match data {
            ReportData::PeriodicMetrics {
                metrics,
                period,
                generated_at,
            } => {
                self.generate_metrics_report(metrics, period, *generated_at, template)
                    .await?
            }
            _ => {
                return Err(ReportingError::GenerationError {
                    message: "Metrics generator only supports periodic metrics data".to_string(),
                });
            }
        };

        Ok(GeneratedReport {
            report_type: "metrics".to_string(),
            title: format!("Metrics Report - {:?}", data),
            content,
            metadata: HashMap::new(),
            generated_at: Utc::now(),
        })
    }

    fn supported_data_types(&self) -> Vec<&'static str> {
        vec!["periodic_metrics"]
    }

    fn get_generator_info(&self) -> GeneratorInfo {
        GeneratorInfo {
            name: self.name.clone(),
            description: "Generates statistical analysis and metrics reports".to_string(),
            supported_formats: vec!["json".to_string(), "yaml".to_string(), "csv".to_string()],
            template_required: false,
        }
    }
}

impl MetricsReportGenerator {
    async fn generate_metrics_report(
        &self,
        metrics: &ExecutionMetrics,
        _period: &ReportPeriod,
        _generated_at: DateTime<Utc>,
        _template: &ReportTemplate,
    ) -> Result<String> {
        // Generate JSON format metrics report
        let metrics_json =
            serde_json::to_value(metrics).map_err(ReportingError::SerializationError)?;

        serde_json::to_string_pretty(&metrics_json)
            .map_err(ReportingError::SerializationError)
    }
}

impl Default for SummaryReportGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for DetailedReportGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for MetricsReportGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handlebars::JsonValue;

    #[tokio::test]
    async fn test_summary_generator_workflow() {
        let generator = SummaryReportGenerator::new();
        let template = ReportTemplate::new("test".to_string(), "summary".to_string());

        let mut result = WorkflowResult::new("test_workflow".to_string(), "run_1".to_string());
        result.mark_completed();

        let metrics = WorkflowMetrics::from_workflow_result(&result);
        let data = ReportData::WorkflowCompletion { result, metrics };

        let report = generator.generate_report(&template, &data).await.unwrap();

        assert_eq!(report.report_type, "summary");
        assert!(report.content.contains("Workflow Summary"));
        assert!(report.content.contains("test_workflow"));
    }

    #[tokio::test]
    async fn test_detailed_generator_workflow() {
        let generator = DetailedReportGenerator::new();
        let template = ReportTemplate::new("test".to_string(), "detailed".to_string());

        let mut result = WorkflowResult::new("test_workflow".to_string(), "run_1".to_string());

        // Add a task
        let mut task = TaskResult::new("task_1".to_string(), "command".to_string());
        task.mark_completed(TaskStatus::Success, Some("Task output".to_string()), None);
        result.add_task_result(task);
        result.mark_completed();

        let metrics = WorkflowMetrics::from_workflow_result(&result);
        let data = ReportData::WorkflowCompletion { result, metrics };

        let report = generator.generate_report(&template, &data).await.unwrap();

        assert_eq!(report.report_type, "detailed");
        assert!(report.content.contains("# Workflow Execution Report"));
        assert!(report.content.contains("## Task Details"));
        assert!(report.content.contains("task_1"));
    }

    #[tokio::test]
    async fn test_metrics_generator() {
        let generator = MetricsReportGenerator::new();
        let template = ReportTemplate::new("test".to_string(), "metrics".to_string());

        let metrics = ExecutionMetrics::new();
        let data = ReportData::PeriodicMetrics {
            metrics,
            period: ReportPeriod::Daily,
            generated_at: Utc::now(),
        };

        let report = generator.generate_report(&template, &data).await.unwrap();

        assert_eq!(report.report_type, "metrics");
        assert!(report.content.contains("total_workflows"));

        // Should be valid JSON
        let _: JsonValue = serde_json::from_str(&report.content).unwrap();
    }

    #[test]
    fn test_generator_info() {
        let summary_gen = SummaryReportGenerator::new();
        let info = summary_gen.get_generator_info();

        assert!(!info.name.is_empty());
        assert!(!info.description.is_empty());
        assert!(!info.supported_formats.is_empty());
    }
}
