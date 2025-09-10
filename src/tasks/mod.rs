// ABOUTME: Native task implementations for various task types
// ABOUTME: Contains concrete implementations for command, compression, S3, email, and Slack tasks

pub mod checksum;
pub mod command;
pub mod compress;
pub mod compression;
pub mod email;
pub mod s3;
pub mod slack;
pub mod template;

use crate::engine::error::Result;
use crate::engine::{ExecutionContext, TaskResult};
use async_trait::async_trait;
use std::collections::HashMap;

#[async_trait]
pub trait TaskImplementation: Send + Sync {
    async fn execute(
        &self,
        task_id: String,
        config: serde_yaml::Value,
        context: ExecutionContext,
    ) -> Result<TaskResult>;

    fn task_type(&self) -> &'static str;
    fn validate_config(&self, config: &serde_yaml::Value) -> Result<()>;
}

pub struct TaskRegistry {
    implementations: HashMap<String, Box<dyn TaskImplementation>>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            implementations: HashMap::new(),
        };

        // Register all built-in task implementations
        registry.register(Box::new(command::CommandTask));
        registry.register(Box::new(compress::CompressTask));
        registry.register(Box::new(checksum::ChecksumTask));
        registry.register(Box::new(compression::CompressionTask)); // Keep for backwards compatibility - rename to "archive"
        registry.register(Box::new(s3::S3Task));
        registry.register(Box::new(email::EmailTask));
        registry.register(Box::new(slack::SlackTask));
        registry.register(Box::new(template::TemplateTask));

        registry
    }

    pub fn register(&mut self, implementation: Box<dyn TaskImplementation>) {
        let task_type = implementation.task_type().to_string();
        self.implementations.insert(task_type, implementation);
    }

    pub fn get_implementation(&self, task_type: &str) -> Option<&dyn TaskImplementation> {
        self.implementations.get(task_type).map(|imp| imp.as_ref())
    }

    pub fn validate_task_config(&self, task_type: &str, config: &serde_yaml::Value) -> Result<()> {
        match self.get_implementation(task_type) {
            Some(implementation) => implementation.validate_config(config),
            None => Err(crate::engine::error::ExecutionError::TaskNotSupported {
                task_type: task_type.to_string(),
            }),
        }
    }

    pub async fn execute_task(
        &self,
        task_id: String,
        task_type: &str,
        config: serde_yaml::Value,
        context: ExecutionContext,
    ) -> Result<TaskResult> {
        match self.get_implementation(task_type) {
            Some(implementation) => implementation.execute(task_id, config, context).await,
            None => Err(crate::engine::error::ExecutionError::TaskNotSupported {
                task_type: task_type.to_string(),
            }),
        }
    }

    pub fn list_supported_tasks(&self) -> Vec<&str> {
        self.implementations.keys().map(|k| k.as_str()).collect()
    }
}

impl Default for TaskRegistry {
    fn default() -> Self {
        Self::new()
    }
}
