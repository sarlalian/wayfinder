// ABOUTME: Task execution result types and workflow result aggregation
// ABOUTME: Defines result structures for individual tasks and complete workflow execution

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
    Timeout,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub task_type: String,
    pub status: TaskStatus,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration: Option<Duration>,
    pub output: Option<String>,
    pub error: Option<String>,
    pub metadata: HashMap<String, String>,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResult {
    pub workflow_name: String,
    pub run_id: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration: Option<Duration>,
    pub status: WorkflowStatus,
    pub tasks: Vec<TaskResult>,
    pub summary: WorkflowSummary,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowStatus {
    Running,
    Success,
    Failed,
    PartialSuccess,
    Cancelled,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSummary {
    pub total_tasks: usize,
    pub successful_tasks: usize,
    pub failed_tasks: usize,
    pub skipped_tasks: usize,
    pub cancelled_tasks: usize,
    pub success_rate: f64,
}

impl TaskResult {
    pub fn new(task_id: String, task_type: String) -> Self {
        Self {
            task_id,
            task_type,
            status: TaskStatus::Pending,
            start_time: Utc::now(),
            end_time: None,
            duration: None,
            output: None,
            error: None,
            metadata: HashMap::new(),
            retry_count: 0,
        }
    }

    pub fn mark_started(&mut self) {
        self.status = TaskStatus::Running;
        self.start_time = Utc::now();
    }

    pub fn mark_completed(
        &mut self,
        status: TaskStatus,
        output: Option<String>,
        error: Option<String>,
    ) {
        self.status = status;
        self.end_time = Some(Utc::now());
        self.duration = Some(
            (Utc::now() - self.start_time)
                .to_std()
                .unwrap_or(Duration::ZERO),
        );
        self.output = output;
        self.error = error;
    }

    pub fn add_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }

    pub fn is_successful(&self) -> bool {
        self.status == TaskStatus::Success
    }

    pub fn is_failed(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Failed | TaskStatus::Timeout | TaskStatus::Cancelled
        )
    }

    pub fn is_finished(&self) -> bool {
        !matches!(self.status, TaskStatus::Pending | TaskStatus::Running)
    }
}

impl WorkflowResult {
    pub fn new(workflow_name: String, run_id: String) -> Self {
        Self {
            workflow_name,
            run_id,
            start_time: Utc::now(),
            end_time: None,
            duration: None,
            status: WorkflowStatus::Running,
            tasks: Vec::new(),
            summary: WorkflowSummary::default(),
            metadata: HashMap::new(),
        }
    }

    pub fn add_task_result(&mut self, result: TaskResult) {
        self.tasks.push(result);
        self.update_summary();
    }

    pub fn update_task_result(&mut self, task_id: &str, result: TaskResult) {
        if let Some(existing) = self.tasks.iter_mut().find(|t| t.task_id == task_id) {
            *existing = result;
        } else {
            self.tasks.push(result);
        }
        self.update_summary();
    }

    pub fn mark_completed(&mut self) {
        self.end_time = Some(Utc::now());
        self.duration = Some(
            (Utc::now() - self.start_time)
                .to_std()
                .unwrap_or(Duration::ZERO),
        );
        self.update_status();
        self.update_summary();
    }

    pub fn get_task_result(&self, task_id: &str) -> Option<&TaskResult> {
        self.tasks.iter().find(|t| t.task_id == task_id)
    }

    pub fn has_failures(&self) -> bool {
        self.tasks.iter().any(|t| t.is_failed())
    }

    pub fn has_critical_failure(&self) -> bool {
        self.tasks
            .iter()
            .any(|t| t.is_failed() && self.is_task_required(t))
    }

    fn is_task_required(&self, _task: &TaskResult) -> bool {
        // TODO: This would check the original task configuration
        // For now, assume all failed tasks are critical
        true
    }

    fn update_status(&mut self) {
        if self.tasks.is_empty() {
            self.status = WorkflowStatus::Success;
            return;
        }

        let has_running = self.tasks.iter().any(|t| t.status == TaskStatus::Running);
        if has_running {
            self.status = WorkflowStatus::Running;
            return;
        }

        let has_failed = self.tasks.iter().any(|t| t.is_failed());
        let has_success = self.tasks.iter().any(|t| t.is_successful());

        match (has_failed, has_success) {
            (false, true) => self.status = WorkflowStatus::Success,
            (true, false) => self.status = WorkflowStatus::Failed,
            (true, true) => self.status = WorkflowStatus::PartialSuccess,
            (false, false) => self.status = WorkflowStatus::Success, // All skipped
        }
    }

    fn update_summary(&mut self) {
        let total = self.tasks.len();
        let successful = self.tasks.iter().filter(|t| t.is_successful()).count();
        let failed = self
            .tasks
            .iter()
            .filter(|t| matches!(t.status, TaskStatus::Failed | TaskStatus::Timeout))
            .count();
        let skipped = self
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Skipped)
            .count();
        let cancelled = self
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Cancelled)
            .count();

        let success_rate = if total > 0 {
            (successful as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        self.summary = WorkflowSummary {
            total_tasks: total,
            successful_tasks: successful,
            failed_tasks: failed,
            skipped_tasks: skipped,
            cancelled_tasks: cancelled,
            success_rate,
        };
    }
}

impl Default for WorkflowSummary {
    fn default() -> Self {
        Self {
            total_tasks: 0,
            successful_tasks: 0,
            failed_tasks: 0,
            skipped_tasks: 0,
            cancelled_tasks: 0,
            success_rate: 0.0,
        }
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::Running => write!(f, "running"),
            TaskStatus::Success => write!(f, "success"),
            TaskStatus::Failed => write!(f, "failed"),
            TaskStatus::Skipped => write!(f, "skipped"),
            TaskStatus::Timeout => write!(f, "timeout"),
            TaskStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::fmt::Display for WorkflowStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkflowStatus::Running => write!(f, "running"),
            WorkflowStatus::Success => write!(f, "success"),
            WorkflowStatus::Failed => write!(f, "failed"),
            WorkflowStatus::PartialSuccess => write!(f, "partial_success"),
            WorkflowStatus::Cancelled => write!(f, "cancelled"),
            WorkflowStatus::Timeout => write!(f, "timeout"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_result_lifecycle() {
        let mut result = TaskResult::new("test_task".to_string(), "command".to_string());

        assert_eq!(result.status, TaskStatus::Pending);
        assert!(!result.is_finished());

        result.mark_started();
        assert_eq!(result.status, TaskStatus::Running);
        assert!(!result.is_finished());

        result.mark_completed(TaskStatus::Success, Some("output".to_string()), None);
        assert_eq!(result.status, TaskStatus::Success);
        assert!(result.is_finished());
        assert!(result.is_successful());
        assert!(!result.is_failed());
    }

    #[test]
    fn test_workflow_result_aggregation() {
        let mut workflow = WorkflowResult::new("test_workflow".to_string(), "run_123".to_string());

        let mut task1 = TaskResult::new("task1".to_string(), "command".to_string());
        task1.mark_completed(TaskStatus::Success, Some("output1".to_string()), None);

        let mut task2 = TaskResult::new("task2".to_string(), "command".to_string());
        task2.mark_completed(TaskStatus::Failed, None, Some("error".to_string()));

        workflow.add_task_result(task1);
        workflow.add_task_result(task2);
        workflow.mark_completed();

        assert_eq!(workflow.summary.total_tasks, 2);
        assert_eq!(workflow.summary.successful_tasks, 1);
        assert_eq!(workflow.summary.failed_tasks, 1);
        assert_eq!(workflow.summary.success_rate, 50.0);
        assert_eq!(workflow.status, WorkflowStatus::PartialSuccess);
    }
}
