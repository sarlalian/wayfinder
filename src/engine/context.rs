// ABOUTME: Execution context and task state management
// ABOUTME: Provides runtime context for task execution and state tracking

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::result::TaskStatus;
use crate::template::TemplateContext;

#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub workflow_name: String,
    pub run_id: String,
    pub execution_id: String,
    pub task_id: String,
    pub start_time: DateTime<Utc>,
    pub template_context: TemplateContext,
    pub shared_state: Arc<RwLock<SharedExecutionState>>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct SharedExecutionState {
    pub task_states: HashMap<String, TaskState>,
    pub task_outputs: HashMap<String, String>,
    pub global_variables: HashMap<String, String>,
    pub execution_flags: HashMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskState {
    pub task_id: String,
    pub status: TaskStatus,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub retry_count: u32,
    pub last_error: Option<String>,
    pub dependencies_met: bool,
}

impl ExecutionContext {
    pub fn new(
        workflow_name: String,
        run_id: String,
        task_id: String,
        template_context: TemplateContext,
    ) -> Self {
        let execution_id = uuid::Uuid::new_v4().to_string();

        Self {
            workflow_name,
            run_id,
            execution_id,
            task_id,
            start_time: Utc::now(),
            template_context,
            shared_state: Arc::new(RwLock::new(SharedExecutionState::default())),
            metadata: HashMap::new(),
        }
    }

    pub fn for_task(&self, task_id: String) -> Self {
        Self {
            workflow_name: self.workflow_name.clone(),
            run_id: self.run_id.clone(),
            execution_id: uuid::Uuid::new_v4().to_string(),
            task_id,
            start_time: Utc::now(),
            template_context: self.template_context.clone(),
            shared_state: Arc::clone(&self.shared_state),
            metadata: HashMap::new(),
        }
    }

    pub fn add_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    pub async fn get_task_state(&self, task_id: &str) -> Option<TaskState> {
        let state = self.shared_state.read().await;
        state.task_states.get(task_id).cloned()
    }

    pub async fn set_task_state(&self, task_id: String, state: TaskState) {
        let mut shared_state = self.shared_state.write().await;
        shared_state.task_states.insert(task_id, state);
    }

    pub async fn get_task_output(&self, task_id: &str) -> Option<String> {
        let state = self.shared_state.read().await;
        state.task_outputs.get(task_id).cloned()
    }

    pub async fn set_task_output(&self, task_id: String, output: String) {
        let mut shared_state = self.shared_state.write().await;
        shared_state.task_outputs.insert(task_id, output);
    }

    pub async fn get_global_variable(&self, key: &str) -> Option<String> {
        let state = self.shared_state.read().await;
        state.global_variables.get(key).cloned()
    }

    pub async fn set_global_variable(&self, key: String, value: String) {
        let mut shared_state = self.shared_state.write().await;
        shared_state.global_variables.insert(key, value);
    }

    pub async fn set_execution_flag(&self, flag: String, value: bool) {
        let mut shared_state = self.shared_state.write().await;
        shared_state.execution_flags.insert(flag, value);
    }

    pub async fn get_execution_flag(&self, flag: &str) -> bool {
        let state = self.shared_state.read().await;
        state.execution_flags.get(flag).copied().unwrap_or(false)
    }

    pub async fn are_dependencies_met(&self, dependencies: &[String]) -> bool {
        let state = self.shared_state.read().await;

        for dep_task_id in dependencies {
            match state.task_states.get(dep_task_id) {
                Some(task_state) => {
                    if !matches!(task_state.status, TaskStatus::Success) {
                        return false;
                    }
                }
                None => {
                    return false; // Dependency not found
                }
            }
        }

        true
    }

    pub async fn get_all_task_states(&self) -> HashMap<String, TaskState> {
        let state = self.shared_state.read().await;
        state.task_states.clone()
    }

    pub async fn get_all_task_outputs(&self) -> HashMap<String, String> {
        let state = self.shared_state.read().await;
        state.task_outputs.clone()
    }
}

impl TaskState {
    pub fn new(task_id: String) -> Self {
        Self {
            task_id,
            status: TaskStatus::Pending,
            start_time: None,
            end_time: None,
            retry_count: 0,
            last_error: None,
            dependencies_met: false,
        }
    }

    pub fn mark_started(&mut self) {
        self.status = TaskStatus::Running;
        self.start_time = Some(Utc::now());
    }

    pub fn mark_completed(&mut self, status: TaskStatus, error: Option<String>) {
        self.status = status;
        self.end_time = Some(Utc::now());
        self.last_error = error;
    }

    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }

    pub fn is_finished(&self) -> bool {
        !matches!(self.status, TaskStatus::Pending | TaskStatus::Running)
    }

    pub fn is_successful(&self) -> bool {
        self.status == TaskStatus::Success
    }

    pub fn can_retry(&self, max_retries: u32) -> bool {
        self.retry_count < max_retries
            && matches!(self.status, TaskStatus::Failed | TaskStatus::Timeout)
    }
}

impl SharedExecutionState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_pending_tasks(&self) -> Vec<String> {
        self.task_states
            .iter()
            .filter_map(|(task_id, state)| {
                if state.status == TaskStatus::Pending && state.dependencies_met {
                    Some(task_id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn get_running_tasks(&self) -> Vec<String> {
        self.task_states
            .iter()
            .filter_map(|(task_id, state)| {
                if state.status == TaskStatus::Running {
                    Some(task_id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn get_completed_tasks(&self) -> Vec<String> {
        self.task_states
            .iter()
            .filter_map(|(task_id, state)| {
                if state.is_finished() {
                    Some(task_id.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::TemplateContext;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_execution_context_creation() {
        let variables = HashMap::new();
        let template_context = TemplateContext::new(&variables).unwrap();

        let context = ExecutionContext::new(
            "test_workflow".to_string(),
            "run_123".to_string(),
            "task_1".to_string(),
            template_context,
        );

        assert_eq!(context.workflow_name, "test_workflow");
        assert_eq!(context.run_id, "run_123");
        assert_eq!(context.task_id, "task_1");
        assert!(!context.execution_id.is_empty());
    }

    #[tokio::test]
    async fn test_task_state_management() {
        let variables = HashMap::new();
        let template_context = TemplateContext::new(&variables).unwrap();

        let context = ExecutionContext::new(
            "test_workflow".to_string(),
            "run_123".to_string(),
            "task_1".to_string(),
            template_context,
        );

        let mut task_state = TaskState::new("task_1".to_string());
        task_state.mark_started();

        context
            .set_task_state("task_1".to_string(), task_state.clone())
            .await;

        let retrieved_state = context.get_task_state("task_1").await.unwrap();
        assert_eq!(retrieved_state.status, TaskStatus::Running);
        assert!(retrieved_state.start_time.is_some());
    }

    #[tokio::test]
    async fn test_dependency_checking() {
        let variables = HashMap::new();
        let template_context = TemplateContext::new(&variables).unwrap();

        let context = ExecutionContext::new(
            "test_workflow".to_string(),
            "run_123".to_string(),
            "task_2".to_string(),
            template_context,
        );

        let mut task1_state = TaskState::new("task_1".to_string());
        task1_state.mark_completed(TaskStatus::Success, None);

        context
            .set_task_state("task_1".to_string(), task1_state)
            .await;

        let dependencies = vec!["task_1".to_string()];
        assert!(context.are_dependencies_met(&dependencies).await);

        let dependencies_with_missing = vec!["task_1".to_string(), "task_missing".to_string()];
        assert!(
            !context
                .are_dependencies_met(&dependencies_with_missing)
                .await
        );
    }

    #[tokio::test]
    async fn test_global_variables() {
        let variables = HashMap::new();
        let template_context = TemplateContext::new(&variables).unwrap();

        let context = ExecutionContext::new(
            "test_workflow".to_string(),
            "run_123".to_string(),
            "task_1".to_string(),
            template_context,
        );

        context
            .set_global_variable("test_var".to_string(), "test_value".to_string())
            .await;

        let value = context.get_global_variable("test_var").await;
        assert_eq!(value, Some("test_value".to_string()));
    }
}
