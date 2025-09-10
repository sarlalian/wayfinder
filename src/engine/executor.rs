// ABOUTME: Main task executor orchestrating workflow execution
// ABOUTME: Coordinates dependency resolution, task scheduling, and execution monitoring

use futures::future;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::Instant;
use tracing::{error, info, instrument, warn};

use super::context::{ExecutionContext, TaskState};
use super::dependency::DependencyGraph;
use super::error::{ExecutionError, Result};
use super::result::{TaskResult, TaskStatus, WorkflowResult};
use super::scheduler::{RetryConfig, ScheduledTask, TaskScheduler};
use crate::parser::{TaskConfig, Workflow};
use crate::template::TemplateEngine;

pub struct TaskExecutor {
    scheduler: TaskScheduler,
    template_engine: TemplateEngine,
    task_registry: crate::tasks::TaskRegistry,
}

impl TaskExecutor {
    /// Create a new task executor with specified concurrency
    pub fn new(max_concurrent: usize) -> Result<Self> {
        let scheduler =
            TaskScheduler::new(max_concurrent).with_default_timeout(Duration::from_secs(3600));

        let template_engine = TemplateEngine::new().map_err(ExecutionError::TemplateError)?;

        let task_registry = crate::tasks::TaskRegistry::new();

        Ok(Self {
            scheduler,
            template_engine,
            task_registry,
        })
    }

    /// Execute a complete workflow
    #[instrument(skip(self, workflow, variables), fields(workflow_name = %workflow.name))]
    pub async fn execute_workflow(
        &self,
        workflow: &Workflow,
        variables: &HashMap<String, String>,
    ) -> Result<WorkflowResult> {
        let start_time = Instant::now();
        let run_id = uuid::Uuid::new_v4().to_string();

        info!(
            "Starting workflow execution: {} (run_id: {})",
            workflow.name, run_id
        );

        // Clone workflow so we can resolve templates in it
        let mut resolved_workflow = workflow.clone();

        // Resolve workflow templates with CLI variables
        let template_engine =
            crate::template::TemplateEngine::new().map_err(ExecutionError::TemplateError)?;
        template_engine
            .resolve_workflow(&mut resolved_workflow, variables)
            .map_err(ExecutionError::TemplateError)?;

        // Create execution context merging resolved workflow and CLI variables (CLI takes precedence)
        let mut merged_variables = resolved_workflow.variables.clone();
        merged_variables.extend(variables.clone()); // CLI variables override workflow variables

        let template_context =
            crate::template::TemplateContext::for_workflow(&merged_variables, &workflow.name)
                .map_err(ExecutionError::TemplateError)?;

        let context = ExecutionContext::new(
            workflow.name.clone(),
            run_id.clone(),
            "workflow".to_string(),
            template_context,
        );

        // Initialize workflow result
        let mut workflow_result = WorkflowResult::new(workflow.name.clone(), run_id);

        // Build dependency graph and execution plan using resolved workflow
        let dependency_graph = DependencyGraph::from_workflow(&resolved_workflow)?;
        dependency_graph.validate()?;

        let execution_plan = dependency_graph.create_execution_plan()?;

        info!(
            "Execution plan: {} batches, {} tasks total, max parallelism: {}",
            execution_plan.execution_depth(),
            execution_plan.total_tasks,
            execution_plan.max_parallelism()
        );

        // Initialize task states
        for task_id in workflow.tasks.keys() {
            let task_state = TaskState::new(task_id.clone());
            context.set_task_state(task_id.clone(), task_state).await;
        }

        // Execute batches sequentially, tasks within batches in parallel
        let mut batch_number = 0;
        for batch in &execution_plan.batches {
            batch_number += 1;

            info!(
                "Executing batch {}/{} with {} tasks: {:?}",
                batch_number,
                execution_plan.batches.len(),
                batch.len(),
                batch
            );

            let batch_results = self
                .execute_batch(batch, workflow, &context, &dependency_graph)
                .await?;

            // Update workflow result with batch results
            for result in batch_results {
                let task_id = result.task_id.clone();
                workflow_result.update_task_result(&task_id, result);
            }

            // Check for critical failures that should stop execution
            if self.should_stop_execution(&workflow_result, workflow).await {
                warn!("Stopping workflow execution due to critical failure");
                break;
            }
        }

        // Handle tasks that were never executed due to failed dependencies
        for (task_id, task_config) in &workflow.tasks {
            if workflow_result.get_task_result(task_id).is_none() {
                // This task was never executed, likely due to failed dependencies
                let mut skipped_result =
                    TaskResult::new(task_id.clone(), task_config.task_type.to_string());
                skipped_result.mark_completed(
                    TaskStatus::Skipped,
                    None,
                    Some("Dependencies failed".to_string()),
                );
                workflow_result.update_task_result(task_id, skipped_result);
            }
        }

        // Mark workflow as completed
        workflow_result.mark_completed();

        let execution_time = start_time.elapsed();
        info!(
            "Workflow execution completed in {:?} with status: {}",
            execution_time, workflow_result.status
        );

        Ok(workflow_result)
    }

    /// Execute a batch of tasks in parallel
    async fn execute_batch(
        &self,
        task_ids: &[String],
        workflow: &Workflow,
        context: &ExecutionContext,
        _dependency_graph: &DependencyGraph,
    ) -> Result<Vec<TaskResult>> {
        if task_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Prepare scheduled tasks and collect skipped task results
        let mut scheduled_tasks = Vec::new();
        let mut skipped_results = Vec::new();

        for task_id in task_ids {
            let task_config =
                workflow
                    .tasks
                    .get(task_id)
                    .ok_or_else(|| ExecutionError::TaskNotFound {
                        task_id: task_id.clone(),
                    })?;

            // Note: Dependency checking is handled by the execution plan batching
            // If a task is in this batch, its dependencies should already be satisfied

            // Check conditional execution
            if let Some(ref when_condition) = task_config.when {
                if !self.evaluate_condition(when_condition, context).await? {
                    info!("Task {} condition not met, skipping", task_id);

                    // Create skipped result
                    let mut task_result =
                        TaskResult::new(task_id.clone(), task_config.task_type.to_string());
                    task_result.mark_completed(
                        TaskStatus::Skipped,
                        None,
                        Some("Condition not met".to_string()),
                    );

                    // Update task state
                    let mut task_state = TaskState::new(task_id.clone());
                    task_state
                        .mark_completed(TaskStatus::Skipped, Some("Condition not met".to_string()));
                    context.set_task_state(task_id.clone(), task_state).await;

                    skipped_results.push(task_result);
                    continue;
                }
            }

            let scheduled_task = ScheduledTask {
                task_id: task_id.clone(),
                task_type: task_config.task_type.to_string(),
                timeout: task_config.timeout,
                retry_config: task_config.retry_config.as_ref().map(|r| RetryConfig {
                    max_attempts: r.max_attempts,
                    initial_delay: r.initial_delay,
                    backoff_multiplier: r.backoff_multiplier,
                    max_delay: Duration::from_secs(300), // Default max delay
                }),
            };

            scheduled_tasks.push(scheduled_task);
        }

        if scheduled_tasks.is_empty() {
            return Ok(skipped_results);
        }

        // Execute the batch tasks concurrently
        let task_futures: Vec<_> = scheduled_tasks
            .into_iter()
            .map(|scheduled_task| {
                let task_config = workflow.tasks.get(&scheduled_task.task_id).unwrap().clone();
                let scheduler = &self.scheduler;
                let template_engine = &self.template_engine;
                let task_registry = &self.task_registry;
                let context = context.clone();

                async move {
                    scheduler
                        .execute_task_with_retry(
                            scheduled_task,
                            |scheduled_task, context| async {
                                Self::execute_single_task(
                                    scheduled_task,
                                    task_config.clone(),
                                    context,
                                    template_engine,
                                    task_registry,
                                )
                                .await
                            },
                            context,
                        )
                        .await
                }
            })
            .collect();

        let mut results = future::join_all(task_futures).await;

        // Combine executed results with skipped results
        results.extend(skipped_results);

        Ok(results)
    }

    /// Execute a single task
    async fn execute_single_task(
        scheduled_task: ScheduledTask,
        task_config: TaskConfig,
        context: ExecutionContext,
        template_engine: &TemplateEngine,
        task_registry: &crate::tasks::TaskRegistry,
    ) -> TaskResult {
        let start_time = chrono::Utc::now();

        // Update task state to running
        let mut task_state = context
            .get_task_state(&scheduled_task.task_id)
            .await
            .unwrap_or_else(|| TaskState::new(scheduled_task.task_id.clone()));
        task_state.mark_started();
        context
            .set_task_state(scheduled_task.task_id.clone(), task_state)
            .await;

        info!(
            "Executing task: {} (type: {})",
            scheduled_task.task_id, scheduled_task.task_type
        );

        let mut result = TaskResult::new(
            scheduled_task.task_id.clone(),
            scheduled_task.task_type.clone(),
        );
        result.start_time = start_time;

        // Map TaskType enum to string for registry lookup
        let task_type_str = match task_config.task_type {
            crate::parser::TaskType::Command => "command",
            crate::parser::TaskType::Compress => "compress",
            crate::parser::TaskType::Decompress => "decompress",
            crate::parser::TaskType::Checksum => "checksum",
            crate::parser::TaskType::ValidateChecksum => "validate_checksum",
            crate::parser::TaskType::S3Upload => "s3",
            crate::parser::TaskType::S3Download => "s3",
            crate::parser::TaskType::Email => "email",
            crate::parser::TaskType::Slack => "slack",
        };

        // Resolve templates in task config before execution
        let template_context = context.template_context.clone();
        let (_json_context, resolved_yaml_config) = match template_context.to_json() {
            Ok(json_context) => {
                match template_engine.resolve_json_templates(
                    &serde_json::to_value(&task_config.config).unwrap(),
                    &json_context,
                ) {
                    Ok(resolved_config) => match serde_yaml::to_value(&resolved_config) {
                        Ok(yaml_config) => (json_context, yaml_config),
                        Err(e) => {
                            let error_msg =
                                format!("Failed to convert resolved config to YAML: {}", e);
                            result.mark_completed(
                                TaskStatus::Failed,
                                None,
                                Some(error_msg.clone()),
                            );
                            error!("Task {} failed: {}", scheduled_task.task_id, error_msg);
                            return result;
                        }
                    },
                    Err(e) => {
                        let error_msg = format!("Failed to resolve task config templates: {}", e);
                        result.mark_completed(TaskStatus::Failed, None, Some(error_msg.clone()));
                        error!("Task {} failed: {}", scheduled_task.task_id, error_msg);
                        return result;
                    }
                }
            }
            Err(e) => {
                let error_msg = format!("Failed to convert template context to JSON: {}", e);
                result.mark_completed(TaskStatus::Failed, None, Some(error_msg.clone()));
                error!("Task {} failed: {}", scheduled_task.task_id, error_msg);
                return result;
            }
        };

        // Execute using task registry
        match task_registry
            .execute_task(
                scheduled_task.task_id.clone(),
                task_type_str,
                resolved_yaml_config,
                context.clone(),
            )
            .await
        {
            Ok(task_result) => {
                // Use the task result directly
                result = task_result;

                // Update task state based on result status
                let mut final_task_state = context
                    .get_task_state(&scheduled_task.task_id)
                    .await
                    .unwrap_or_else(|| TaskState::new(scheduled_task.task_id.clone()));

                match result.status {
                    TaskStatus::Success => {
                        final_task_state.mark_completed(TaskStatus::Success, None);
                        info!("Task {} completed successfully", scheduled_task.task_id);
                    }
                    TaskStatus::Failed => {
                        final_task_state.mark_completed(TaskStatus::Failed, result.error.clone());
                        error!("Task {} failed: {:?}", scheduled_task.task_id, result.error);
                    }
                    _ => {
                        // For other statuses, just update the state
                        final_task_state
                            .mark_completed(result.status.clone(), result.error.clone());
                    }
                }

                context
                    .set_task_state(scheduled_task.task_id.clone(), final_task_state)
                    .await;
            }
            Err(e) => {
                let error_msg = format!("Task execution failed: {}", e);
                result.mark_completed(TaskStatus::Failed, None, Some(error_msg.clone()));

                // Update task state to failed
                let mut final_task_state = context
                    .get_task_state(&scheduled_task.task_id)
                    .await
                    .unwrap_or_else(|| TaskState::new(scheduled_task.task_id.clone()));
                final_task_state.mark_completed(TaskStatus::Failed, Some(error_msg));
                context
                    .set_task_state(scheduled_task.task_id.clone(), final_task_state)
                    .await;

                error!("Task {} failed: {}", scheduled_task.task_id, e);
            }
        }

        result
    }

    /// Evaluate a conditional expression
    async fn evaluate_condition(
        &self,
        _condition: &str,
        _context: &ExecutionContext,
    ) -> Result<bool> {
        // Placeholder implementation - always returns true
        // In reality, this would parse and evaluate the condition expression
        Ok(true)
    }

    /// Check if workflow execution should be stopped due to failures
    async fn should_stop_execution(
        &self,
        workflow_result: &WorkflowResult,
        workflow: &Workflow,
    ) -> bool {
        // Check if any required tasks have failed
        for task_result in &workflow_result.tasks {
            if task_result.is_failed() {
                // Check if this task is required
                if let Some(task_config) = workflow.tasks.get(&task_result.task_id) {
                    if task_config.required {
                        return true;
                    }
                }
            }
        }

        // Check error handling configuration
        if let Some(ref error_config) = workflow.on_error {
            return !error_config.continue_on_error && workflow_result.has_failures();
        }

        false
    }

    /// Validate workflow before execution
    pub fn validate_workflow(&self, workflow: &Workflow) -> Result<()> {
        // Build dependency graph to check for cycles and missing dependencies
        let dependency_graph = DependencyGraph::from_workflow(workflow)?;
        dependency_graph.validate()?;

        // Validate that execution plan can be created
        let _execution_plan = dependency_graph.create_execution_plan()?;

        // Use WorkflowValidator with task registry for comprehensive validation
        let task_registry = crate::tasks::TaskRegistry::new();
        let validator = crate::parser::validation::WorkflowValidator::new()
            .with_task_registry(task_registry)
            .with_strict_mode(true);

        let validation_report =
            validator
                .validate(workflow)
                .map_err(|e| ExecutionError::ValidationError {
                    message: format!("Workflow validation failed: {}", e),
                })?;

        if !validation_report.is_valid {
            let error_messages: Vec<String> = validation_report
                .errors
                .iter()
                .map(|e| e.to_string())
                .collect();

            return Err(ExecutionError::ValidationError {
                message: format!("Workflow validation failed:\n{}", error_messages.join("\n")),
            });
        }

        // Log warnings if any
        for warning in &validation_report.warnings {
            warn!("Workflow validation warning: {}", warning);
        }

        info!("Workflow validation successful for: {}", workflow.name);
        Ok(())
    }

    /// Get executor statistics
    pub fn get_stats(&self) -> ExecutorStats {
        let resource_stats = self.scheduler.get_resource_stats();

        ExecutorStats {
            max_concurrent_tasks: resource_stats.max_concurrent,
            active_tasks: resource_stats.active_tasks,
            available_capacity: resource_stats.available_permits,
            utilization_percentage: resource_stats.utilization_percentage(),
        }
    }

    /// Shutdown the executor gracefully
    pub async fn shutdown(&self, timeout: Duration) -> Result<()> {
        info!("Shutting down task executor...");
        self.scheduler.shutdown(timeout).await
    }
}

#[derive(Debug, Clone)]
pub struct ExecutorStats {
    pub max_concurrent_tasks: usize,
    pub active_tasks: usize,
    pub available_capacity: usize,
    pub utilization_percentage: f64,
}

/// Simplified workflow engine interface that wraps TaskExecutor
/// This provides a more convenient API that matches what the tests expect
pub struct WorkflowEngine {
    executor: TaskExecutor,
}

impl std::fmt::Debug for WorkflowEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkflowEngine").finish()
    }
}

impl WorkflowEngine {
    pub fn new() -> Self {
        Self {
            executor: TaskExecutor::new(4).expect("Failed to create task executor"),
        }
    }

    pub async fn execute_workflow(&mut self, workflow: &Workflow) -> Result<WorkflowResult> {
        let variables = workflow.variables.clone();
        self.executor.execute_workflow(workflow, &variables).await
    }
}

impl Default for WorkflowEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{OutputConfig, TaskType};
    use indexmap::IndexMap;

    fn create_test_workflow() -> Workflow {
        let mut tasks = IndexMap::new();

        let task1 = TaskConfig {
            name: Some("task1".to_string()),
            description: None,
            task_type: TaskType::Command,
            depends_on: vec![],
            required: true,
            retry_config: None,
            when: None,
            timeout: Some(Duration::from_secs(30)),
            config: serde_yaml::to_value(crate::tasks::command::CommandConfig {
                command: Some("echo".to_string()),
                args: vec!["task1 completed".to_string()],
                ..Default::default()
            })
            .unwrap(),
        };
        tasks.insert("task1".to_string(), task1);

        let task2 = TaskConfig {
            name: Some("task2".to_string()),
            description: None,
            task_type: TaskType::Command,
            depends_on: vec!["task1".to_string()],
            required: true,
            retry_config: Some(crate::parser::task::RetryConfig {
                max_attempts: 3,
                backoff_multiplier: 2.0,
                initial_delay: Duration::from_secs(1),
            }),
            when: None,
            timeout: None,
            config: serde_yaml::to_value(crate::tasks::command::CommandConfig {
                command: Some("echo".to_string()),
                args: vec!["task2 completed".to_string()],
                ..Default::default()
            })
            .unwrap(),
        };
        tasks.insert("task2".to_string(), task2);

        Workflow {
            name: "test_workflow".to_string(),
            description: Some("Test workflow".to_string()),
            version: "1.0".to_string(),
            author: None,
            variables: HashMap::new(),
            tasks,
            output: OutputConfig::default(),
            on_error: None,
        }
    }

    #[tokio::test]
    async fn test_executor_creation() {
        let executor = TaskExecutor::new(4).unwrap();
        let stats = executor.get_stats();

        assert_eq!(stats.max_concurrent_tasks, 4);
        assert_eq!(stats.active_tasks, 0);
        assert_eq!(stats.utilization_percentage, 0.0);
    }

    #[tokio::test]
    async fn test_workflow_validation() {
        let executor = TaskExecutor::new(2).unwrap();
        let workflow = create_test_workflow();

        let result = executor.validate_workflow(&workflow);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_workflow_execution() {
        let executor = TaskExecutor::new(2).unwrap();
        let workflow = create_test_workflow();
        let variables = HashMap::new();

        let result = executor.execute_workflow(&workflow, &variables).await;

        assert!(result.is_ok());
        let workflow_result = result.unwrap();

        assert_eq!(workflow_result.workflow_name, "test_workflow");
        assert_eq!(workflow_result.tasks.len(), 2);
        assert!(matches!(
            workflow_result.status,
            crate::WorkflowStatus::Success
        ));
    }

    #[test]
    fn test_executor_stats() {
        let stats = ExecutorStats {
            max_concurrent_tasks: 4,
            active_tasks: 2,
            available_capacity: 2,
            utilization_percentage: 50.0,
        };

        assert_eq!(
            stats.active_tasks + stats.available_capacity,
            stats.max_concurrent_tasks
        );
    }
}
