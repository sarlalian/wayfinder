// ABOUTME: Reporting engine for workflow execution analytics and notifications
// ABOUTME: Handles report generation, metrics collection, and multi-channel delivery

pub mod config;
pub mod delivery;
pub mod error;
pub mod generator;
pub mod metrics;
pub mod templates;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use self::config::{ReportTemplate, ReportingConfig};
use self::delivery::{
    EmailDeliverer, FileDeliverer, ReportDeliverer, SlackDeliverer, TerminalDeliverer,
};
use self::error::{ReportingError, Result};
use self::generator::{
    DetailedReportGenerator, MetricsReportGenerator, ReportGenerator, SummaryReportGenerator,
};
use self::metrics::{ExecutionMetrics, MetricsCollector, WorkflowMetrics};
use crate::engine::{TaskResult, WorkflowResult};

pub struct ReportingEngine {
    generators: HashMap<String, Box<dyn ReportGenerator>>,
    deliverers: HashMap<String, Box<dyn ReportDeliverer>>,
    metrics_collector: MetricsCollector,
}

#[async_trait]
pub trait ReportProcessor: Send + Sync {
    async fn process_workflow_completion(
        &self,
        result: &WorkflowResult,
        config: &ReportingConfig,
    ) -> Result<()>;

    async fn process_task_completion(
        &self,
        result: &TaskResult,
        workflow_context: &WorkflowContext,
        config: &ReportingConfig,
    ) -> Result<()>;

    async fn generate_periodic_report(
        &self,
        period: ReportPeriod,
        config: &ReportingConfig,
    ) -> Result<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowContext {
    pub workflow_name: String,
    pub run_id: String,
    pub start_time: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ReportPeriod {
    Hourly,
    Daily,
    Weekly,
    Monthly,
}

#[derive(Debug, Clone)]
pub struct GeneratedReport {
    pub report_type: String,
    pub title: String,
    pub content: String,
    pub metadata: HashMap<String, String>,
    pub generated_at: DateTime<Utc>,
}

impl ReportingEngine {
    pub fn new() -> Self {
        let mut engine = Self {
            generators: HashMap::new(),
            deliverers: HashMap::new(),
            metrics_collector: MetricsCollector::new(),
        };

        // Register built-in report generators
        engine.register_generator("summary", Box::new(SummaryReportGenerator::new()));
        engine.register_generator("detailed", Box::new(DetailedReportGenerator::new()));
        engine.register_generator("metrics", Box::new(MetricsReportGenerator::new()));

        // Register built-in deliverers
        engine.register_deliverer("email", Box::new(EmailDeliverer::new()));
        engine.register_deliverer("slack", Box::new(SlackDeliverer::new()));
        engine.register_deliverer("file", Box::new(FileDeliverer::new()));
        engine.register_deliverer("terminal", Box::new(TerminalDeliverer::new()));

        engine
    }

    pub fn register_generator(&mut self, name: &str, generator: Box<dyn ReportGenerator>) {
        self.generators.insert(name.to_string(), generator);
    }

    pub fn register_deliverer(&mut self, name: &str, deliverer: Box<dyn ReportDeliverer>) {
        self.deliverers.insert(name.to_string(), deliverer);
    }

    pub async fn generate_and_deliver_report(
        &self,
        template: &ReportTemplate,
        data: &ReportData,
    ) -> Result<()> {
        // Get report generator
        let generator = self
            .generators
            .get(&template.generator_type)
            .ok_or_else(|| ReportingError::GeneratorNotFound {
                generator_type: template.generator_type.clone(),
            })?;

        // Generate the report
        let report = generator.generate_report(template, data).await?;

        // Deliver to all configured channels
        for channel in &template.delivery_channels {
            if let Some(deliverer) = self.deliverers.get(&channel.channel_type) {
                deliverer.deliver_report(&report, channel).await?;
            } else {
                return Err(ReportingError::DelivererNotFound {
                    deliverer_type: channel.channel_type.clone(),
                });
            }
        }

        Ok(())
    }

    pub fn get_metrics(&self) -> &MetricsCollector {
        &self.metrics_collector
    }

    pub fn list_generators(&self) -> Vec<&str> {
        self.generators.keys().map(|k| k.as_str()).collect()
    }

    pub fn list_deliverers(&self) -> Vec<&str> {
        self.deliverers.keys().map(|k| k.as_str()).collect()
    }
}

#[async_trait]
impl ReportProcessor for ReportingEngine {
    async fn process_workflow_completion(
        &self,
        result: &WorkflowResult,
        config: &ReportingConfig,
    ) -> Result<()> {
        // Collect metrics
        self.metrics_collector
            .record_workflow_completion(result)
            .await;

        // Check if workflow completion reports are enabled
        if !config.workflow_completion_reports {
            return Ok(());
        }

        // Generate reports for workflow completion
        let data = ReportData::from_workflow_result(result);

        for template in &config.templates {
            if template.trigger_on_workflow_completion {
                self.generate_and_deliver_report(template, &data).await?;
            }
        }

        Ok(())
    }

    async fn process_task_completion(
        &self,
        result: &TaskResult,
        workflow_context: &WorkflowContext,
        config: &ReportingConfig,
    ) -> Result<()> {
        // Collect metrics
        self.metrics_collector
            .record_task_completion(result, workflow_context)
            .await;

        // Check if task completion reports are enabled
        if !config.task_completion_reports {
            return Ok(());
        }

        // Only generate reports for failed tasks or if explicitly configured
        let should_report = match result.status {
            crate::engine::TaskStatus::Failed
            | crate::engine::TaskStatus::Timeout
            | crate::engine::TaskStatus::Cancelled => true,
            _ => config.report_successful_tasks,
        };

        if !should_report {
            return Ok(());
        }

        // Generate reports for task completion
        let data = ReportData::from_task_result(result, workflow_context);

        for template in &config.templates {
            if template.trigger_on_task_failure && result.is_failed() {
                self.generate_and_deliver_report(template, &data).await?;
            }
        }

        Ok(())
    }

    async fn generate_periodic_report(
        &self,
        period: ReportPeriod,
        config: &ReportingConfig,
    ) -> Result<()> {
        // Get metrics for the specified period
        let metrics = self.metrics_collector.get_period_metrics(&period).await;
        let data = ReportData::from_metrics(&metrics.execution_metrics, &period);

        // Generate reports for the period
        for template in &config.templates {
            if template.matches_period(&period) {
                self.generate_and_deliver_report(template, &data).await?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ReportData {
    WorkflowCompletion {
        result: WorkflowResult,
        metrics: WorkflowMetrics,
    },
    TaskCompletion {
        result: TaskResult,
        context: WorkflowContext,
    },
    PeriodicMetrics {
        metrics: ExecutionMetrics,
        period: ReportPeriod,
        generated_at: DateTime<Utc>,
    },
}

impl ReportData {
    pub fn from_workflow_result(result: &WorkflowResult) -> Self {
        Self::WorkflowCompletion {
            result: result.clone(),
            metrics: WorkflowMetrics::from_workflow_result(result),
        }
    }

    pub fn from_task_result(result: &TaskResult, context: &WorkflowContext) -> Self {
        Self::TaskCompletion {
            result: result.clone(),
            context: context.clone(),
        }
    }

    pub fn from_metrics(metrics: &ExecutionMetrics, period: &ReportPeriod) -> Self {
        Self::PeriodicMetrics {
            metrics: metrics.clone(),
            period: period.clone(),
            generated_at: Utc::now(),
        }
    }
}

impl Default for ReportingEngine {
    fn default() -> Self {
        Self::new()
    }
}
