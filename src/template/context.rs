// ABOUTME: Template context management and system information collection
// ABOUTME: Provides template variables, environment info, and system details for rendering

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::env;

use super::error::{Result, TemplateError};

#[derive(Debug, Clone, Serialize)]
pub struct TemplateContext {
    pub variables: HashMap<String, String>,
    pub env: HashMap<String, String>,
    pub system: SystemInfo,
    pub workflow: WorkflowInfo,
    pub execution: ExecutionInfo,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub user: String,
    pub pwd: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowInfo {
    pub name: String,
    pub start_time: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecutionInfo {
    pub run_id: String,
    pub execution_id: String,
}

impl TemplateContext {
    /// Create a new template context with default system information
    pub fn new(variables: &HashMap<String, String>) -> Result<Self> {
        let system = SystemInfo::collect()?;
        let env = env::vars().collect();

        Ok(Self {
            variables: variables.clone(),
            env,
            system,
            workflow: WorkflowInfo::default(),
            execution: ExecutionInfo::default(),
        })
    }

    /// Create context for a specific workflow
    pub fn for_workflow(variables: &HashMap<String, String>, workflow_name: &str) -> Result<Self> {
        let mut context = Self::new(variables)?;
        context.workflow = WorkflowInfo {
            name: workflow_name.to_string(),
            start_time: Utc::now(),
        };
        Ok(context)
    }

    /// Add or update a variable
    pub fn set_variable(&mut self, key: String, value: String) {
        self.variables.insert(key, value);
    }

    /// Get a variable value
    pub fn get_variable(&self, key: &str) -> Option<&String> {
        self.variables.get(key)
    }

    /// Add multiple variables
    pub fn extend_variables(&mut self, vars: HashMap<String, String>) {
        self.variables.extend(vars);
    }

    /// Update execution information
    pub fn set_execution_info(&mut self, run_id: String, execution_id: String) {
        self.execution = ExecutionInfo {
            run_id,
            execution_id,
        };
    }

    /// Convert context to JSON for handlebars rendering
    pub fn to_json(&self) -> Result<serde_json::Value> {
        serde_json::to_value(self).map_err(TemplateError::JsonError)
    }
}

impl SystemInfo {
    /// Collect system information
    pub fn collect() -> Result<Self> {
        let hostname = hostname::get()
            .map_err(|_| TemplateError::SystemError("Failed to get hostname".to_string()))?
            .to_string_lossy()
            .to_string();

        let os = std::env::consts::OS.to_string();
        let arch = std::env::consts::ARCH.to_string();

        let user = env::var("USER")
            .or_else(|_| env::var("USERNAME"))
            .unwrap_or_else(|_| "unknown".to_string());

        let pwd = env::current_dir()
            .map_err(TemplateError::IoError)?
            .display()
            .to_string();

        Ok(Self {
            hostname,
            os,
            arch,
            user,
            pwd,
            timestamp: Utc::now(),
        })
    }
}

impl Default for WorkflowInfo {
    fn default() -> Self {
        Self {
            name: "unknown".to_string(),
            start_time: Utc::now(),
        }
    }
}

impl Default for ExecutionInfo {
    fn default() -> Self {
        use uuid::Uuid;

        let run_id = Uuid::new_v4().to_string();
        let execution_id = Uuid::new_v4().to_string();

        Self {
            run_id,
            execution_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_info_collection() {
        let system_info = SystemInfo::collect().unwrap();
        assert!(!system_info.hostname.is_empty());
        assert!(!system_info.os.is_empty());
        assert!(!system_info.arch.is_empty());
        assert!(!system_info.pwd.is_empty());
    }

    #[test]
    fn test_template_context_creation() {
        let mut variables = HashMap::new();
        variables.insert("env".to_string(), "test".to_string());
        variables.insert("version".to_string(), "1.0.0".to_string());

        let context = TemplateContext::new(&variables).unwrap();

        assert_eq!(context.get_variable("env"), Some(&"test".to_string()));
        assert_eq!(context.get_variable("version"), Some(&"1.0.0".to_string()));
        assert!(!context.system.hostname.is_empty());
    }

    #[test]
    fn test_workflow_context() {
        let variables = HashMap::new();
        let context = TemplateContext::for_workflow(&variables, "test-workflow").unwrap();

        assert_eq!(context.workflow.name, "test-workflow");
        assert!(!context.execution.run_id.is_empty());
        assert!(!context.execution.execution_id.is_empty());
    }

    #[test]
    fn test_context_json_conversion() {
        let variables = HashMap::new();
        let context = TemplateContext::new(&variables).unwrap();
        let json = context.to_json().unwrap();

        assert!(json.is_object());
        assert!(json["system"].is_object());
        assert!(json["variables"].is_object());
    }
}
