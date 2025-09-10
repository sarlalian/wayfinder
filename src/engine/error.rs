// ABOUTME: Error types for task execution engine operations
// ABOUTME: Defines specific error types for workflow execution and task processing

use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("Task execution failed: {task_id} - {message}")]
    TaskFailed { task_id: String, message: String },

    #[error("Task timeout: {task_id} - exceeded {timeout:?}")]
    TaskTimeout { task_id: String, timeout: Duration },

    #[error("Dependency error: {message}")]
    DependencyError { message: String },

    #[error("Circular dependency detected: {tasks:?}")]
    CircularDependency { tasks: Vec<String> },

    #[error("Task not found: {task_id}")]
    TaskNotFound { task_id: String },

    #[error("Task type not supported: {task_type}")]
    TaskNotSupported { task_type: String },

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Task execution error: {0}")]
    TaskExecutionError(String),

    #[error("Invalid task configuration: {task_id} - {reason}")]
    InvalidConfiguration { task_id: String, reason: String },

    #[error("Configuration error for task {task_id}: {message}")]
    ConfigError { task_id: String, message: String },

    #[error("Execution context error: {message}")]
    ContextError { message: String },

    #[error("Workflow execution aborted: {reason}")]
    WorkflowAborted { reason: String },

    #[error("Resource limit exceeded: {resource} - {limit}")]
    ResourceLimitExceeded { resource: String, limit: String },

    #[error("Template error: {0}")]
    TemplateError(#[from] crate::template::TemplateError),

    #[error("Validation error: {message}")]
    ValidationError { message: String },

    #[error("Parser error: {0}")]
    ParserError(#[from] crate::parser::ParserError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),

    #[error("System error: {0}")]
    SystemError(String),
}

pub type Result<T> = std::result::Result<T, ExecutionError>;
