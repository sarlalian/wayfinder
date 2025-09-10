// ABOUTME: Core workflow data structures and parsing functionality
// ABOUTME: Defines the main Workflow struct and related configuration types

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use super::error::{ParserError, Result};
use super::task::TaskConfig;
use tokio::fs;

fn default_version() -> String {
    "1.0".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_version")]
    pub version: String,
    pub author: Option<String>,
    #[serde(default)]
    pub variables: HashMap<String, String>,
    pub tasks: IndexMap<String, TaskConfig>,
    pub output: OutputConfig,
    pub on_error: Option<ErrorConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub destination: String,
    #[serde(default)]
    pub format: OutputFormat,
    #[serde(default)]
    pub include_logs: bool,
    #[serde(default)]
    pub compress: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Json,
    Yaml,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorConfig {
    #[serde(default)]
    pub continue_on_error: bool,
    pub cleanup_tasks: Option<Vec<String>>,
    pub notification: Option<NotificationConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum NotificationConfig {
    Email {
        to: Vec<String>,
        subject: String,
        template: Option<String>,
    },
    Slack {
        channel: String,
        template: Option<String>,
    },
}

impl Workflow {
    /// Parse workflow from YAML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(ParserError::IoError)?;
        Self::from_yaml(&content)
    }

    /// Parse workflow from YAML string
    pub fn from_yaml(content: &str) -> Result<Self> {
        let mut workflow: Workflow =
            serde_yaml::from_str(content).map_err(ParserError::YamlError)?;

        // Set task names from keys if not explicitly set
        for (task_id, task_config) in &mut workflow.tasks {
            if task_config.name.is_none() {
                task_config.name = Some(task_id.clone());
            }
        }

        // Validate the workflow structure
        workflow.validate_structure()?;

        Ok(workflow)
    }

    /// Validate basic workflow structure
    fn validate_structure(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(ParserError::MissingField("name".to_string()));
        }

        if self.tasks.is_empty() {
            return Err(ParserError::ValidationError(
                super::error::ValidationError::EmptyWorkflow,
            ));
        }

        // Check for duplicate task names (keys are unique in IndexMap, but check names)
        let mut task_names = std::collections::HashSet::new();
        for (task_id, task_config) in &self.tasks {
            let task_name = task_config.name.as_ref().unwrap_or(task_id);
            if !task_names.insert(task_name.clone()) {
                return Err(ParserError::ValidationError(
                    super::error::ValidationError::DuplicateTask {
                        task: task_name.clone(),
                    },
                ));
            }
        }

        // Validate output destination
        if self.output.destination.trim().is_empty() {
            return Err(ParserError::ValidationError(
                super::error::ValidationError::InvalidOutput {
                    reason: "destination cannot be empty".to_string(),
                },
            ));
        }

        Ok(())
    }

    /// Get all task IDs in the workflow
    pub fn task_ids(&self) -> Vec<String> {
        self.tasks.keys().cloned().collect()
    }

    /// Get task configuration by ID
    pub fn get_task(&self, task_id: &str) -> Option<&TaskConfig> {
        self.tasks.get(task_id)
    }

    /// Get all dependencies for a specific task
    pub fn get_task_dependencies(&self, task_id: &str) -> Vec<String> {
        if let Some(task) = self.get_task(task_id) {
            task.depends_on.clone()
        } else {
            Vec::new()
        }
    }

    /// Check if a task exists in the workflow
    pub fn has_task(&self, task_id: &str) -> bool {
        self.tasks.contains_key(task_id)
    }

    /// Get all tasks that depend on a specific task
    pub fn get_dependent_tasks(&self, task_id: &str) -> Vec<String> {
        self.tasks
            .iter()
            .filter_map(|(id, task)| {
                if task.depends_on.contains(&task_id.to_string()) {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Merge variables from external source
    pub fn merge_variables(&mut self, vars: HashMap<String, String>) {
        self.variables.extend(vars);
    }

    /// Convert workflow back to YAML string
    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml::to_string(self).map_err(ParserError::YamlError)
    }

    /// Save workflow to file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let yaml = self.to_yaml()?;
        std::fs::write(path.as_ref(), yaml).map_err(ParserError::IoError)?;
        Ok(())
    }
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            destination: "file://./output/workflow-result.json".to_string(),
            format: OutputFormat::Json,
            include_logs: false,
            compress: false,
        }
    }
}

impl OutputFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            OutputFormat::Json => "json",
            OutputFormat::Yaml => "yaml",
        }
    }

    pub fn file_extension(&self) -> &'static str {
        match self {
            OutputFormat::Json => "json",
            OutputFormat::Yaml => "yaml",
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorkflowParser;

impl WorkflowParser {
    pub fn new() -> Self {
        Self
    }

    pub async fn parse_file<P: AsRef<Path>>(&self, path: P) -> Result<Workflow> {
        let content = fs::read_to_string(path.as_ref())
            .await
            .map_err(ParserError::IoError)?;
        self.parse_string(&content)
    }

    pub fn parse_string(&self, content: &str) -> Result<Workflow> {
        Workflow::from_yaml(content)
    }

    pub fn validate_workflow(&self, workflow: &Workflow) -> Result<()> {
        workflow.validate_structure()
    }
}

impl Default for WorkflowParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_basic_workflow() {
        let yaml = r#"
name: test_workflow
description: A test workflow

variables:
  env: production

tasks:
  hello:
    type: command
    config:
      command: echo
      args: ["Hello World"]

output:
  destination: "file://./output.json"
"#;

        let workflow = Workflow::from_yaml(yaml).unwrap();
        assert_eq!(workflow.name, "test_workflow");
        assert_eq!(workflow.tasks.len(), 1);
        assert!(workflow.tasks.contains_key("hello"));
        assert_eq!(
            workflow.variables.get("env"),
            Some(&"production".to_string())
        );
    }

    #[test]
    fn test_parse_workflow_with_dependencies() {
        let yaml = r#"
name: dependency_test

tasks:
  first:
    type: command
    config:
      command: echo
      args: ["First"]
  
  second:
    type: command
    depends_on: [first]
    config:
      command: echo
      args: ["Second"]

output:
  destination: "file://./output.json"
"#;

        let workflow = Workflow::from_yaml(yaml).unwrap();
        let second_deps = workflow.get_task_dependencies("second");
        assert_eq!(second_deps, vec!["first"]);

        let first_dependents = workflow.get_dependent_tasks("first");
        assert_eq!(first_dependents, vec!["second"]);
    }

    #[test]
    fn test_workflow_validation_empty_name() {
        let yaml = r#"
name: ""
tasks:
  test:
    type: command
    config:
      command: echo
output:
  destination: "file://./output.json"
"#;

        let result = Workflow::from_yaml(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_workflow_validation_no_tasks() {
        let yaml = r#"
name: empty_workflow
tasks: {}
output:
  destination: "file://./output.json"
"#;

        let result = Workflow::from_yaml(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_workflow_file_operations() {
        let workflow = create_test_workflow();

        // Test save and load
        let mut temp_file = NamedTempFile::new().unwrap();
        let yaml = workflow.to_yaml().unwrap();
        temp_file.write_all(yaml.as_bytes()).unwrap();

        let loaded = Workflow::from_file(temp_file.path()).unwrap();
        assert_eq!(loaded.name, workflow.name);
        assert_eq!(loaded.tasks.len(), workflow.tasks.len());
    }

    fn create_test_workflow() -> Workflow {
        let yaml = r#"
name: test_workflow
tasks:
  test:
    type: command
    config:
      command: echo
      args: ["test"]
output:
  destination: "file://./output.json"
"#;
        Workflow::from_yaml(yaml).unwrap()
    }
}
