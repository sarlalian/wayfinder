// ABOUTME: Task scheduling and parallel execution management
// ABOUTME: Handles concurrent task execution with resource limits and backpressure control

use futures::future::join_all;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};

use super::context::ExecutionContext;
use super::error::{ExecutionError, Result};
use super::result::{TaskResult, TaskStatus};

pub struct TaskScheduler {
    max_concurrent: usize,
    semaphore: Arc<Semaphore>,
    default_timeout: Duration,
}

pub struct ScheduledTask {
    pub task_id: String,
    pub task_type: String,
    pub timeout: Option<Duration>,
    pub retry_config: Option<RetryConfig>,
}

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub backoff_multiplier: f64,
    pub max_delay: Duration,
}

impl TaskScheduler {
    /// Create a new task scheduler with specified concurrency limit
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            max_concurrent,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            default_timeout: Duration::from_secs(3600), // 1 hour default
        }
    }

    /// Set the default timeout for tasks
    pub fn with_default_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Execute a batch of tasks concurrently
    pub async fn execute_batch<F, Fut>(
        &self,
        tasks: Vec<ScheduledTask>,
        base_context: &ExecutionContext,
        executor_fn: F,
    ) -> Vec<TaskResult>
    where
        F: Fn(ScheduledTask, ExecutionContext) -> Fut + Send + Sync + Clone + 'static,
        Fut: futures::Future<Output = TaskResult> + Send + 'static,
    {
        if tasks.is_empty() {
            return Vec::new();
        }

        info!("Executing batch of {} tasks", tasks.len());

        let futures = tasks.into_iter().map(|task| {
            let permit = Arc::clone(&self.semaphore);
            let executor = executor_fn.clone();
            let timeout_duration = task.timeout.unwrap_or(self.default_timeout);
            let base_context = base_context.clone();

            tokio::spawn(async move {
                // Acquire permit for execution
                let _permit = permit.acquire().await.expect("Semaphore closed");

                debug!("Starting task execution: {}", task.task_id);

                // Create task-specific context from base context
                let task_context = base_context.for_task(task.task_id.clone());

                // Execute with timeout
                let task_id = task.task_id.clone();
                match timeout(timeout_duration, executor(task, task_context)).await {
                    Ok(result) => result,
                    Err(_) => {
                        error!("Task {} timed out after {:?}", task_id, timeout_duration);
                        TaskResult {
                            task_id: task_id.clone(),
                            task_type: "unknown".to_string(),
                            status: TaskStatus::Timeout,
                            start_time: chrono::Utc::now(),
                            end_time: Some(chrono::Utc::now()),
                            duration: Some(timeout_duration),
                            output: None,
                            error: Some(format!("Task timed out after {:?}", timeout_duration)),
                            metadata: std::collections::HashMap::new(),
                            retry_count: 0,
                        }
                    }
                }
            })
        });

        let results = join_all(futures).await;

        // Extract results and handle any join errors
        let mut task_results = Vec::new();
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(task_result) => {
                    debug!(
                        "Task {} completed with status: {}",
                        task_result.task_id, task_result.status
                    );
                    task_results.push(task_result);
                }
                Err(join_error) => {
                    error!("Task join error: {}", join_error);
                    // Create a failed task result for the join error
                    task_results.push(TaskResult {
                        task_id: format!("unknown_task_{}", i),
                        task_type: "unknown".to_string(),
                        status: TaskStatus::Failed,
                        start_time: chrono::Utc::now(),
                        end_time: Some(chrono::Utc::now()),
                        duration: Some(Duration::ZERO),
                        output: None,
                        error: Some(format!("Join error: {}", join_error)),
                        metadata: std::collections::HashMap::new(),
                        retry_count: 0,
                    });
                }
            }
        }

        info!("Batch execution completed. {} results", task_results.len());
        task_results
    }

    /// Execute a single task with retry logic
    pub async fn execute_task_with_retry<F, Fut>(
        &self,
        task: ScheduledTask,
        executor_fn: F,
        context: ExecutionContext,
    ) -> TaskResult
    where
        F: Fn(ScheduledTask, ExecutionContext) -> Fut + Send + Sync + Clone,
        Fut: futures::Future<Output = TaskResult> + Send,
    {
        let retry_config = task.retry_config.clone().unwrap_or_default();
        let mut current_attempt = 0;
        let mut last_result = None;

        while current_attempt < retry_config.max_attempts {
            current_attempt += 1;

            info!(
                "Executing task {} (attempt {}/{})",
                task.task_id, current_attempt, retry_config.max_attempts
            );

            // Execute the task with timeout if specified
            let result = if let Some(timeout_duration) = task.timeout {
                match timeout(
                    timeout_duration,
                    executor_fn(task.clone(), context.for_task(task.task_id.clone())),
                )
                .await
                {
                    Ok(task_result) => task_result,
                    Err(_) => {
                        // Task timed out
                        warn!(
                            "Task {} timed out after {:?}",
                            task.task_id, timeout_duration
                        );
                        TaskResult {
                            task_id: task.task_id.clone(),
                            task_type: task.task_type.clone(),
                            status: TaskStatus::Timeout,
                            start_time: chrono::Utc::now(),
                            end_time: Some(chrono::Utc::now()),
                            duration: Some(timeout_duration),
                            output: None,
                            error: Some(format!("Task timed out after {:?}", timeout_duration)),
                            metadata: std::collections::HashMap::new(),
                            retry_count: 0,
                        }
                    }
                }
            } else {
                executor_fn(task.clone(), context.for_task(task.task_id.clone())).await
            };

            match result.status {
                TaskStatus::Success => {
                    debug!(
                        "Task {} succeeded on attempt {}",
                        task.task_id, current_attempt
                    );
                    return result;
                }
                TaskStatus::Failed | TaskStatus::Timeout => {
                    warn!(
                        "Task {} failed on attempt {}: {:?}",
                        task.task_id, current_attempt, result.error
                    );

                    last_result = Some(result);

                    // If we have more attempts, wait before retrying
                    if current_attempt < retry_config.max_attempts {
                        let delay = retry_config.calculate_delay(current_attempt - 1);
                        debug!("Waiting {:?} before retry", delay);
                        sleep(delay).await;
                    }
                }
                _ => {
                    // For other statuses (Skipped, Cancelled), don't retry
                    return result;
                }
            }
        }

        // All retries exhausted, return the last result
        let mut final_result = last_result.expect("Should have at least one result");
        final_result.retry_count = current_attempt;

        error!(
            "Task {} failed after {} attempts",
            task.task_id, current_attempt
        );
        final_result
    }

    /// Get current resource usage statistics
    pub fn get_resource_stats(&self) -> ResourceStats {
        ResourceStats {
            max_concurrent: self.max_concurrent,
            available_permits: self.semaphore.available_permits(),
            active_tasks: self.max_concurrent - self.semaphore.available_permits(),
        }
    }

    /// Wait for all currently executing tasks to complete
    pub async fn wait_for_completion(&self) -> Result<()> {
        // Wait until all permits are available (no tasks running)
        let _permits = self
            .semaphore
            .acquire_many(self.max_concurrent as u32)
            .await
            .map_err(|_| ExecutionError::SystemError("Semaphore closed".to_string()))?;

        Ok(())
    }

    /// Gracefully shutdown the scheduler
    pub async fn shutdown(&self, timeout_duration: Duration) -> Result<()> {
        info!("Shutting down task scheduler...");

        // Try to wait for completion within timeout
        match timeout(timeout_duration, self.wait_for_completion()).await {
            Ok(Ok(())) => {
                info!("Task scheduler shutdown completed successfully");
                Ok(())
            }
            Ok(Err(e)) => {
                error!("Error during scheduler shutdown: {}", e);
                Err(e)
            }
            Err(_) => {
                warn!("Scheduler shutdown timed out after {:?}", timeout_duration);
                Err(ExecutionError::SystemError(format!(
                    "Shutdown timed out after {:?}",
                    timeout_duration
                )))
            }
        }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            max_delay: Duration::from_secs(300), // 5 minutes
        }
    }
}

impl RetryConfig {
    /// Calculate delay for a specific retry attempt (0-indexed)
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        let delay_ms = (self.initial_delay.as_millis() as f64
            * self.backoff_multiplier.powi(attempt as i32)) as u64;

        let delay = Duration::from_millis(delay_ms);

        // Cap at max_delay
        if delay > self.max_delay {
            self.max_delay
        } else {
            delay
        }
    }

    /// Create a retry config with exponential backoff
    pub fn exponential_backoff(
        max_attempts: u32,
        initial_delay: Duration,
        multiplier: f64,
    ) -> Self {
        Self {
            max_attempts,
            initial_delay,
            backoff_multiplier: multiplier,
            max_delay: Duration::from_secs(300), // 5 minutes default
        }
    }

    /// Create a retry config with fixed delay
    pub fn fixed_delay(max_attempts: u32, delay: Duration) -> Self {
        Self {
            max_attempts,
            initial_delay: delay,
            backoff_multiplier: 1.0, // No backoff
            max_delay: delay,
        }
    }
}

impl Clone for ScheduledTask {
    fn clone(&self) -> Self {
        Self {
            task_id: self.task_id.clone(),
            task_type: self.task_type.clone(),
            timeout: self.timeout,
            retry_config: self.retry_config.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResourceStats {
    pub max_concurrent: usize,
    pub available_permits: usize,
    pub active_tasks: usize,
}

impl ResourceStats {
    pub fn utilization_percentage(&self) -> f64 {
        if self.max_concurrent == 0 {
            0.0
        } else {
            (self.active_tasks as f64 / self.max_concurrent as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_scheduler_creation() {
        let scheduler = TaskScheduler::new(4);
        assert_eq!(scheduler.max_concurrent, 4);
        assert_eq!(scheduler.semaphore.available_permits(), 4);
    }

    #[tokio::test]
    async fn test_retry_config_delay_calculation() {
        let config = RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            backoff_multiplier: 2.0,
            max_delay: Duration::from_secs(1),
        };

        assert_eq!(config.calculate_delay(0), Duration::from_millis(100));
        assert_eq!(config.calculate_delay(1), Duration::from_millis(200));
        assert_eq!(config.calculate_delay(2), Duration::from_millis(400));

        // Test max delay cap
        let config_with_cap = RetryConfig {
            max_attempts: 5,
            initial_delay: Duration::from_millis(500),
            backoff_multiplier: 2.0,
            max_delay: Duration::from_millis(600),
        };

        assert_eq!(
            config_with_cap.calculate_delay(2),
            Duration::from_millis(600)
        ); // Capped
    }

    #[tokio::test]
    async fn test_concurrent_execution_limit() {
        let scheduler = TaskScheduler::new(2); // Limit to 2 concurrent
        let counter = Arc::new(AtomicU32::new(0));
        let max_concurrent = Arc::new(AtomicU32::new(0));

        let tasks: Vec<ScheduledTask> = (0..5)
            .map(|i| ScheduledTask {
                task_id: format!("task_{}", i),
                task_type: "test".to_string(),
                timeout: Some(Duration::from_millis(100)),
                retry_config: None,
            })
            .collect();

        let counter_clone = Arc::clone(&counter);
        let max_concurrent_clone = Arc::clone(&max_concurrent);

        // Create a dummy execution context for testing
        let variables = std::collections::HashMap::new();
        let template_context = crate::template::TemplateContext::new(&variables).unwrap();
        let test_context = ExecutionContext::new(
            "test_workflow".to_string(),
            "test_run".to_string(),
            "test_task".to_string(),
            template_context,
        );

        let results = scheduler
            .execute_batch(tasks, &test_context, move |task, _ctx| {
                let counter = Arc::clone(&counter_clone);
                let max_concurrent = Arc::clone(&max_concurrent_clone);

                async move {
                    let current = counter.fetch_add(1, Ordering::SeqCst) + 1;

                    // Track maximum concurrent tasks
                    loop {
                        let current_max = max_concurrent.load(Ordering::SeqCst);
                        if current <= current_max
                            || max_concurrent
                                .compare_exchange_weak(
                                    current_max,
                                    current,
                                    Ordering::SeqCst,
                                    Ordering::SeqCst,
                                )
                                .is_ok()
                        {
                            break;
                        }
                    }

                    // Simulate work
                    sleep(Duration::from_millis(50)).await;

                    counter.fetch_sub(1, Ordering::SeqCst);

                    TaskResult {
                        task_id: task.task_id,
                        task_type: task.task_type,
                        status: TaskStatus::Success,
                        start_time: chrono::Utc::now(),
                        end_time: Some(chrono::Utc::now()),
                        duration: Some(Duration::from_millis(50)),
                        output: None,
                        error: None,
                        metadata: std::collections::HashMap::new(),
                        retry_count: 0,
                    }
                }
            })
            .await;

        assert_eq!(results.len(), 5);
        assert!(results.iter().all(|r| r.status == TaskStatus::Success));

        // Should never have more than 2 concurrent tasks
        assert!(max_concurrent.load(Ordering::SeqCst) <= 2);
    }

    #[test]
    fn test_resource_stats() {
        let stats = ResourceStats {
            max_concurrent: 4,
            available_permits: 2,
            active_tasks: 2,
        };

        assert_eq!(stats.utilization_percentage(), 50.0);

        let empty_stats = ResourceStats {
            max_concurrent: 0,
            available_permits: 0,
            active_tasks: 0,
        };

        assert_eq!(empty_stats.utilization_percentage(), 0.0);
    }
}
