// ABOUTME: Error types for workflow parsing and validation
// ABOUTME: Defines specific error types for parser module operations

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("Failed to read workflow file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Failed to parse YAML: {0}")]
    YamlError(#[from] serde_yaml::Error),

    #[error("Invalid workflow format: {0}")]
    InvalidFormat(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Validation failed: {0}")]
    ValidationError(#[from] ValidationError),
}

#[derive(Error, Debug, Clone)]
pub enum ValidationError {
    #[error("Circular dependency detected in tasks: {tasks:?}")]
    CircularDependency { tasks: Vec<String> },

    #[error("Task '{task}' depends on unknown task '{dependency}'")]
    UnknownDependency { task: String, dependency: String },

    #[error("Invalid task configuration for '{task}': {reason}")]
    InvalidTaskConfig { task: String, reason: String },

    #[error("Invalid template syntax in '{field}': {error}")]
    InvalidTemplate { field: String, error: String },

    #[error("Invalid output configuration: {reason}")]
    InvalidOutput { reason: String },

    #[error("Duplicate task name: {task}")]
    DuplicateTask { task: String },

    #[error("Empty workflow: no tasks defined")]
    EmptyWorkflow,

    #[error("Unsupported task type '{task_type}' in task '{task}'. Supported types: {supported_types:?}")]
    UnsupportedTaskType {
        task: String,
        task_type: String,
        supported_types: Vec<String>,
    },
}

pub type Result<T> = std::result::Result<T, ParserError>;
