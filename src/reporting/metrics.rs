// ABOUTME: Metrics collection and analysis for workflow execution
// ABOUTME: Tracks performance statistics, success rates, and execution patterns

use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{ReportPeriod, WorkflowContext};
use crate::engine::{TaskResult, WorkflowResult};

#[derive(Debug, Clone)]
pub struct MetricsCollector {
    workflow_metrics: Arc<RwLock<VecDeque<WorkflowMetrics>>>,
    task_metrics: Arc<RwLock<VecDeque<TaskMetrics>>>,
    execution_metrics: Arc<RwLock<ExecutionMetrics>>,
    retention_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowMetrics {
    pub workflow_name: String,
    pub run_id: String,
    pub status: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration: Option<Duration>,
    pub total_tasks: u32,
    pub successful_tasks: u32,
    pub failed_tasks: u32,
    pub skipped_tasks: u32,
    pub success_rate: f64,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMetrics {
    pub task_id: String,
    pub task_type: String,
    pub workflow_name: String,
    pub run_id: String,
    pub status: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration: Option<Duration>,
    pub retry_count: u32,
    pub error_message: Option<String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    pub total_workflows: u64,
    pub successful_workflows: u64,
    pub failed_workflows: u64,
    pub total_tasks: u64,
    pub successful_tasks: u64,
    pub failed_tasks: u64,
    pub skipped_tasks: u64,
    pub cancelled_tasks: u64,
    pub avg_workflow_duration: Option<Duration>,
    pub avg_task_duration: Option<Duration>,
    pub workflow_success_rate: f64,
    pub task_success_rate: f64,
    pub most_common_failures: Vec<FailurePattern>,
    pub performance_trends: PerformanceTrends,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailurePattern {
    pub error_type: String,
    pub count: u64,
    pub percentage: f64,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceTrends {
    pub workflow_duration_trend: TrendDirection,
    pub task_duration_trend: TrendDirection,
    pub success_rate_trend: TrendDirection,
    pub throughput_trend: TrendDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    Improving,
    Stable,
    Degrading,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodMetrics {
    pub period: ReportPeriod,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub execution_metrics: ExecutionMetrics,
    pub top_workflows: Vec<WorkflowSummary>,
    pub performance_breakdown: PerformanceBreakdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSummary {
    pub name: String,
    pub runs: u64,
    pub success_rate: f64,
    pub avg_duration: Option<Duration>,
    pub total_tasks: u64,
    pub last_run: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBreakdown {
    pub by_workflow: HashMap<String, WorkflowPerformance>,
    pub by_task_type: HashMap<String, TaskTypePerformance>,
    pub by_hour: Vec<HourlyMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPerformance {
    pub total_runs: u64,
    pub successful_runs: u64,
    pub failed_runs: u64,
    pub avg_duration: Option<Duration>,
    pub min_duration: Option<Duration>,
    pub max_duration: Option<Duration>,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTypePerformance {
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub avg_duration: Option<Duration>,
    pub avg_retry_count: f64,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HourlyMetrics {
    pub hour: DateTime<Utc>,
    pub workflows_started: u64,
    pub workflows_completed: u64,
    pub tasks_executed: u64,
    pub avg_workflow_duration: Option<Duration>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            workflow_metrics: Arc::new(RwLock::new(VecDeque::new())),
            task_metrics: Arc::new(RwLock::new(VecDeque::new())),
            execution_metrics: Arc::new(RwLock::new(ExecutionMetrics::new())),
            retention_limit: 10000, // Keep last 10k records
        }
    }

    pub fn with_retention_limit(mut self, limit: usize) -> Self {
        self.retention_limit = limit;
        self
    }

    pub async fn record_workflow_completion(&self, result: &WorkflowResult) {
        let metrics = WorkflowMetrics::from_workflow_result(result);

        // Add to workflow metrics history
        {
            let mut workflow_metrics = self.workflow_metrics.write().await;
            workflow_metrics.push_back(metrics.clone());

            // Maintain retention limit
            while workflow_metrics.len() > self.retention_limit {
                workflow_metrics.pop_front();
            }
        }

        // Update execution metrics
        {
            let mut exec_metrics = self.execution_metrics.write().await;
            exec_metrics.record_workflow_completion(&metrics);
        }
    }

    pub async fn record_task_completion(&self, result: &TaskResult, context: &WorkflowContext) {
        let metrics = TaskMetrics::from_task_result(result, context);

        // Add to task metrics history
        {
            let mut task_metrics = self.task_metrics.write().await;
            task_metrics.push_back(metrics.clone());

            // Maintain retention limit
            while task_metrics.len() > self.retention_limit {
                task_metrics.pop_front();
            }
        }

        // Update execution metrics
        {
            let mut exec_metrics = self.execution_metrics.write().await;
            exec_metrics.record_task_completion(&metrics);
        }
    }

    pub async fn get_period_metrics(&self, period: &ReportPeriod) -> PeriodMetrics {
        let (start_time, end_time) = self.calculate_period_bounds(period);

        let workflow_metrics = self.workflow_metrics.read().await;
        let task_metrics = self.task_metrics.read().await;

        // Filter metrics for the period
        let period_workflows: Vec<_> = workflow_metrics
            .iter()
            .filter(|m| m.start_time >= start_time && m.start_time < end_time)
            .cloned()
            .collect();

        let period_tasks: Vec<_> = task_metrics
            .iter()
            .filter(|m| m.start_time >= start_time && m.start_time < end_time)
            .cloned()
            .collect();

        // Calculate execution metrics for period
        let execution_metrics = self.calculate_execution_metrics(
            &period_workflows,
            &period_tasks,
            start_time,
            end_time,
        );

        // Generate top workflows
        let top_workflows = self.calculate_top_workflows(&period_workflows);

        // Generate performance breakdown
        let performance_breakdown =
            self.calculate_performance_breakdown(&period_workflows, &period_tasks);

        PeriodMetrics {
            period: period.clone(),
            start_time,
            end_time,
            execution_metrics,
            top_workflows,
            performance_breakdown,
        }
    }

    pub async fn get_workflow_metrics(
        &self,
        workflow_name: &str,
        limit: Option<usize>,
    ) -> Vec<WorkflowMetrics> {
        let workflow_metrics = self.workflow_metrics.read().await;
        let mut filtered: Vec<_> = workflow_metrics
            .iter()
            .filter(|m| m.workflow_name == workflow_name)
            .cloned()
            .collect();

        filtered.sort_by(|a, b| b.start_time.cmp(&a.start_time));

        if let Some(limit) = limit {
            filtered.truncate(limit);
        }

        filtered
    }

    pub async fn get_task_metrics(
        &self,
        task_type: Option<&str>,
        limit: Option<usize>,
    ) -> Vec<TaskMetrics> {
        let task_metrics = self.task_metrics.read().await;
        let mut filtered: Vec<_> = if let Some(task_type) = task_type {
            task_metrics
                .iter()
                .filter(|m| m.task_type == task_type)
                .cloned()
                .collect()
        } else {
            task_metrics.iter().cloned().collect()
        };

        filtered.sort_by(|a, b| b.start_time.cmp(&a.start_time));

        if let Some(limit) = limit {
            filtered.truncate(limit);
        }

        filtered
    }

    pub async fn get_execution_summary(&self) -> ExecutionMetrics {
        self.execution_metrics.read().await.clone()
    }

    pub async fn reset_metrics(&self) {
        let mut workflow_metrics = self.workflow_metrics.write().await;
        let mut task_metrics = self.task_metrics.write().await;
        let mut exec_metrics = self.execution_metrics.write().await;

        workflow_metrics.clear();
        task_metrics.clear();
        *exec_metrics = ExecutionMetrics::new();
    }

    fn calculate_period_bounds(&self, period: &ReportPeriod) -> (DateTime<Utc>, DateTime<Utc>) {
        let now = Utc::now();

        match period {
            ReportPeriod::Hourly => {
                let start = now
                    .with_minute(0)
                    .unwrap()
                    .with_second(0)
                    .unwrap()
                    .with_nanosecond(0)
                    .unwrap()
                    - Duration::hours(1);
                let end = start + Duration::hours(1);
                (start, end)
            }
            ReportPeriod::Daily => {
                let start =
                    now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc() - Duration::days(1);
                let end = start + Duration::days(1);
                (start, end)
            }
            ReportPeriod::Weekly => {
                let days_since_monday = now.weekday().num_days_from_monday() as i64;
                let start = (now.date_naive() - Duration::days(days_since_monday + 7))
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_utc();
                let end = start + Duration::days(7);
                (start, end)
            }
            ReportPeriod::Monthly => {
                let start = now
                    .date_naive()
                    .with_day(1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_utc()
                    - Duration::days(30); // Approximate previous month
                let end = now
                    .date_naive()
                    .with_day(1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_utc();
                (start, end)
            }
        }
    }

    fn calculate_execution_metrics(
        &self,
        workflows: &[WorkflowMetrics],
        tasks: &[TaskMetrics],
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> ExecutionMetrics {
        let mut exec_metrics = ExecutionMetrics::new();
        exec_metrics.period_start = start_time;
        exec_metrics.period_end = end_time;

        // Workflow statistics
        exec_metrics.total_workflows = workflows.len() as u64;
        exec_metrics.successful_workflows =
            workflows.iter().filter(|w| w.status == "Success").count() as u64;
        exec_metrics.failed_workflows =
            workflows.iter().filter(|w| w.status == "Failed").count() as u64;

        if exec_metrics.total_workflows > 0 {
            exec_metrics.workflow_success_rate = exec_metrics.successful_workflows as f64
                / exec_metrics.total_workflows as f64
                * 100.0;
        }

        // Task statistics
        exec_metrics.total_tasks = tasks.len() as u64;
        exec_metrics.successful_tasks =
            tasks.iter().filter(|t| t.status == "Success").count() as u64;
        exec_metrics.failed_tasks = tasks.iter().filter(|t| t.status == "Failed").count() as u64;
        exec_metrics.skipped_tasks = tasks.iter().filter(|t| t.status == "Skipped").count() as u64;
        exec_metrics.cancelled_tasks =
            tasks.iter().filter(|t| t.status == "Cancelled").count() as u64;

        if exec_metrics.total_tasks > 0 {
            exec_metrics.task_success_rate =
                exec_metrics.successful_tasks as f64 / exec_metrics.total_tasks as f64 * 100.0;
        }

        // Duration calculations
        let workflow_durations: Vec<_> = workflows.iter().filter_map(|w| w.duration).collect();
        if !workflow_durations.is_empty() {
            let total_duration: Duration = workflow_durations.iter().sum();
            exec_metrics.avg_workflow_duration =
                Some(total_duration / workflow_durations.len() as i32);
        }

        let task_durations: Vec<_> = tasks.iter().filter_map(|t| t.duration).collect();
        if !task_durations.is_empty() {
            let total_duration: Duration = task_durations.iter().sum();
            exec_metrics.avg_task_duration = Some(total_duration / task_durations.len() as i32);
        }

        exec_metrics
    }

    fn calculate_top_workflows(&self, workflows: &[WorkflowMetrics]) -> Vec<WorkflowSummary> {
        let mut workflow_map: HashMap<String, WorkflowSummary> = HashMap::new();

        for workflow in workflows {
            let summary = workflow_map
                .entry(workflow.workflow_name.clone())
                .or_insert_with(|| WorkflowSummary {
                    name: workflow.workflow_name.clone(),
                    runs: 0,
                    success_rate: 0.0,
                    avg_duration: None,
                    total_tasks: 0,
                    last_run: workflow.start_time,
                });

            summary.runs += 1;
            summary.total_tasks += workflow.total_tasks as u64;

            if workflow.start_time > summary.last_run {
                summary.last_run = workflow.start_time;
            }

            // Calculate success rate
            let successful = workflows
                .iter()
                .filter(|w| w.workflow_name == workflow.workflow_name && w.status == "Success")
                .count() as f64;
            summary.success_rate = successful / summary.runs as f64 * 100.0;
        }

        let mut summaries: Vec<_> = workflow_map.into_values().collect();
        summaries.sort_by(|a, b| b.runs.cmp(&a.runs));
        summaries.truncate(10); // Top 10
        summaries
    }

    fn calculate_performance_breakdown(
        &self,
        workflows: &[WorkflowMetrics],
        tasks: &[TaskMetrics],
    ) -> PerformanceBreakdown {
        let by_workflow = self.calculate_workflow_performance(workflows);
        let by_task_type = self.calculate_task_type_performance(tasks);
        let by_hour = self.calculate_hourly_metrics(workflows, tasks);

        PerformanceBreakdown {
            by_workflow,
            by_task_type,
            by_hour,
        }
    }

    fn calculate_workflow_performance(
        &self,
        workflows: &[WorkflowMetrics],
    ) -> HashMap<String, WorkflowPerformance> {
        let mut performance_map = HashMap::new();

        for workflow in workflows {
            let perf = performance_map
                .entry(workflow.workflow_name.clone())
                .or_insert_with(|| WorkflowPerformance {
                    total_runs: 0,
                    successful_runs: 0,
                    failed_runs: 0,
                    avg_duration: None,
                    min_duration: None,
                    max_duration: None,
                    success_rate: 0.0,
                });

            perf.total_runs += 1;
            if workflow.status == "Success" {
                perf.successful_runs += 1;
            } else if workflow.status == "Failed" {
                perf.failed_runs += 1;
            }

            if let Some(duration) = workflow.duration {
                if perf.min_duration.is_none() || duration < perf.min_duration.unwrap() {
                    perf.min_duration = Some(duration);
                }
                if perf.max_duration.is_none() || duration > perf.max_duration.unwrap() {
                    perf.max_duration = Some(duration);
                }
            }
        }

        // Calculate success rates and average durations
        for (name, perf) in performance_map.iter_mut() {
            if perf.total_runs > 0 {
                perf.success_rate = perf.successful_runs as f64 / perf.total_runs as f64 * 100.0;

                let durations: Vec<_> = workflows
                    .iter()
                    .filter(|w| w.workflow_name == *name)
                    .filter_map(|w| w.duration)
                    .collect();

                if !durations.is_empty() {
                    let total: Duration = durations.iter().sum();
                    perf.avg_duration = Some(total / durations.len() as i32);
                }
            }
        }

        performance_map
    }

    fn calculate_task_type_performance(
        &self,
        tasks: &[TaskMetrics],
    ) -> HashMap<String, TaskTypePerformance> {
        let mut performance_map = HashMap::new();

        for task in tasks {
            let perf = performance_map
                .entry(task.task_type.clone())
                .or_insert_with(|| TaskTypePerformance {
                    total_executions: 0,
                    successful_executions: 0,
                    failed_executions: 0,
                    avg_duration: None,
                    avg_retry_count: 0.0,
                    success_rate: 0.0,
                });

            perf.total_executions += 1;
            if task.status == "Success" {
                perf.successful_executions += 1;
            } else if task.status == "Failed" {
                perf.failed_executions += 1;
            }
        }

        // Calculate success rates, average durations and retry counts
        for (task_type, perf) in performance_map.iter_mut() {
            if perf.total_executions > 0 {
                perf.success_rate =
                    perf.successful_executions as f64 / perf.total_executions as f64 * 100.0;

                let type_tasks: Vec<_> =
                    tasks.iter().filter(|t| t.task_type == *task_type).collect();

                let durations: Vec<_> = type_tasks.iter().filter_map(|t| t.duration).collect();
                if !durations.is_empty() {
                    let total: Duration = durations.iter().sum();
                    perf.avg_duration = Some(total / durations.len() as i32);
                }

                let total_retries: u32 = type_tasks.iter().map(|t| t.retry_count).sum();
                perf.avg_retry_count = total_retries as f64 / type_tasks.len() as f64;
            }
        }

        performance_map
    }

    fn calculate_hourly_metrics(
        &self,
        workflows: &[WorkflowMetrics],
        _tasks: &[TaskMetrics],
    ) -> Vec<HourlyMetrics> {
        let mut hourly_map: HashMap<DateTime<Utc>, HourlyMetrics> = HashMap::new();

        for workflow in workflows {
            let hour = workflow
                .start_time
                .with_minute(0)
                .unwrap()
                .with_second(0)
                .unwrap()
                .with_nanosecond(0)
                .unwrap();

            let metrics = hourly_map.entry(hour).or_insert_with(|| HourlyMetrics {
                hour,
                workflows_started: 0,
                workflows_completed: 0,
                tasks_executed: 0,
                avg_workflow_duration: None,
            });

            metrics.workflows_started += 1;
            if workflow.end_time.is_some() {
                metrics.workflows_completed += 1;
            }
            metrics.tasks_executed += workflow.total_tasks as u64;
        }

        let mut hourly: Vec<_> = hourly_map.into_values().collect();
        hourly.sort_by(|a, b| a.hour.cmp(&b.hour));
        hourly
    }
}

impl WorkflowMetrics {
    pub fn from_workflow_result(result: &WorkflowResult) -> Self {
        Self {
            workflow_name: result.workflow_name.clone(),
            run_id: result.run_id.clone(),
            status: result.status.to_string(),
            start_time: result.start_time,
            end_time: result.end_time,
            duration: result
                .duration
                .map(|d| Duration::from_std(d).unwrap_or(Duration::zero())),
            total_tasks: result.summary.total_tasks as u32,
            successful_tasks: result.summary.successful_tasks as u32,
            failed_tasks: result.summary.failed_tasks as u32,
            skipped_tasks: result.summary.skipped_tasks as u32,
            success_rate: result.summary.success_rate,
            metadata: result.metadata.clone(),
        }
    }
}

impl TaskMetrics {
    pub fn from_task_result(result: &TaskResult, context: &WorkflowContext) -> Self {
        Self {
            task_id: result.task_id.clone(),
            task_type: result.task_type.clone(),
            workflow_name: context.workflow_name.clone(),
            run_id: context.run_id.clone(),
            status: result.status.to_string(),
            start_time: result.start_time,
            end_time: result.end_time,
            duration: result
                .duration
                .map(|d| Duration::from_std(d).unwrap_or(Duration::zero())),
            retry_count: result.retry_count,
            error_message: result.error.clone(),
            metadata: result.metadata.clone(),
        }
    }
}

impl Default for ExecutionMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionMetrics {
    pub fn new() -> Self {
        Self {
            total_workflows: 0,
            successful_workflows: 0,
            failed_workflows: 0,
            total_tasks: 0,
            successful_tasks: 0,
            failed_tasks: 0,
            skipped_tasks: 0,
            cancelled_tasks: 0,
            avg_workflow_duration: None,
            avg_task_duration: None,
            workflow_success_rate: 0.0,
            task_success_rate: 0.0,
            most_common_failures: Vec::new(),
            performance_trends: PerformanceTrends::default(),
            period_start: Utc::now(),
            period_end: Utc::now(),
        }
    }

    pub fn record_workflow_completion(&mut self, metrics: &WorkflowMetrics) {
        self.total_workflows += 1;
        if metrics.status == "success" {
            self.successful_workflows += 1;
        } else if metrics.status == "failed" {
            self.failed_workflows += 1;
        }

        if self.total_workflows > 0 {
            self.workflow_success_rate =
                self.successful_workflows as f64 / self.total_workflows as f64 * 100.0;
        }
    }

    pub fn record_task_completion(&mut self, metrics: &TaskMetrics) {
        self.total_tasks += 1;
        match metrics.status.as_str() {
            "Success" => self.successful_tasks += 1,
            "Failed" => self.failed_tasks += 1,
            "Skipped" => self.skipped_tasks += 1,
            "Cancelled" => self.cancelled_tasks += 1,
            _ => {}
        }

        if self.total_tasks > 0 {
            self.task_success_rate = self.successful_tasks as f64 / self.total_tasks as f64 * 100.0;
        }
    }
}

impl Default for PerformanceTrends {
    fn default() -> Self {
        Self {
            workflow_duration_trend: TrendDirection::Unknown,
            task_duration_trend: TrendDirection::Unknown,
            success_rate_trend: TrendDirection::Unknown,
            throughput_trend: TrendDirection::Unknown,
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_collection() {
        let collector = MetricsCollector::new();

        // Create test workflow result
        let mut workflow_result =
            WorkflowResult::new("test_workflow".to_string(), "run_1".to_string());
        workflow_result.mark_completed();

        collector.record_workflow_completion(&workflow_result).await;

        let summary = collector.get_execution_summary().await;
        assert_eq!(summary.total_workflows, 1);
        assert_eq!(summary.successful_workflows, 1);
    }

    #[tokio::test]
    async fn test_period_metrics() {
        let collector = MetricsCollector::new();
        let period = ReportPeriod::Hourly;

        let period_metrics = collector.get_period_metrics(&period).await;
        assert!(matches!(period_metrics.period, ReportPeriod::Hourly));
    }

    #[tokio::test]
    async fn test_workflow_filtering() {
        let collector = MetricsCollector::new();

        let mut workflow1 = WorkflowResult::new("workflow1".to_string(), "run_1".to_string());
        workflow1.mark_completed();

        let mut workflow2 = WorkflowResult::new("workflow2".to_string(), "run_2".to_string());
        workflow2.mark_completed();

        collector.record_workflow_completion(&workflow1).await;
        collector.record_workflow_completion(&workflow2).await;

        let workflow1_metrics = collector.get_workflow_metrics("workflow1", None).await;
        assert_eq!(workflow1_metrics.len(), 1);
        assert_eq!(workflow1_metrics[0].workflow_name, "workflow1");

        let workflow2_metrics = collector.get_workflow_metrics("workflow2", None).await;
        assert_eq!(workflow2_metrics.len(), 1);
        assert_eq!(workflow2_metrics[0].workflow_name, "workflow2");
    }

    #[test]
    fn test_metrics_serialization() {
        let metrics = ExecutionMetrics::new();
        let json = serde_json::to_string(&metrics).unwrap();
        let parsed: ExecutionMetrics = serde_json::from_str(&json).unwrap();

        assert_eq!(metrics.total_workflows, parsed.total_workflows);
        assert_eq!(metrics.workflow_success_rate, parsed.workflow_success_rate);
    }
}
