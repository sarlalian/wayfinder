// ABOUTME: Workflow validation logic and dependency checking
// ABOUTME: Provides comprehensive validation for workflow structure and task dependencies

use petgraph::graph::NodeIndex;
use petgraph::Graph;
use std::collections::{HashMap, HashSet, VecDeque};

use super::error::{Result, ValidationError};
use super::task::TaskConfig;
use super::workflow::Workflow;
use crate::tasks::TaskRegistry;

#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
    pub is_valid: bool,
}

pub struct WorkflowValidator {
    strict_mode: bool,
    task_registry: Option<TaskRegistry>,
}

impl WorkflowValidator {
    pub fn new() -> Self {
        Self { 
            strict_mode: false,
            task_registry: None,
        }
    }

    pub fn with_strict_mode(mut self, strict: bool) -> Self {
        self.strict_mode = strict;
        self
    }

    pub fn with_task_registry(mut self, task_registry: TaskRegistry) -> Self {
        self.task_registry = Some(task_registry);
        self
    }

    /// Validate a complete workflow
    pub fn validate(&self, workflow: &Workflow) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();

        // Validate task dependencies
        self.validate_dependencies(workflow, &mut report)?;

        // Validate task configurations
        self.validate_task_configs(workflow, &mut report)?;

        // Validate templates (basic syntax check)
        self.validate_templates(workflow, &mut report)?;

        // Validate output configuration
        self.validate_output_config(workflow, &mut report)?;

        // Check for unreachable tasks
        self.check_unreachable_tasks(workflow, &mut report);

        report.is_valid = report.errors.is_empty();
        Ok(report)
    }

    /// Validate task dependencies and detect cycles
    fn validate_dependencies(
        &self,
        workflow: &Workflow,
        report: &mut ValidationReport,
    ) -> Result<()> {
        let task_ids: HashSet<String> = workflow.tasks.keys().cloned().collect();

        // Check that all dependencies exist
        for (task_id, task_config) in &workflow.tasks {
            for dep in &task_config.depends_on {
                if !task_ids.contains(dep) {
                    report.errors.push(ValidationError::UnknownDependency {
                        task: task_id.clone(),
                        dependency: dep.clone(),
                    });
                }
            }
        }

        // Check for circular dependencies using topological sort
        if let Err(cycle) = self.detect_cycles(workflow) {
            report
                .errors
                .push(ValidationError::CircularDependency { tasks: cycle });
        }

        Ok(())
    }

    /// Detect circular dependencies using DFS
    fn detect_cycles(&self, workflow: &Workflow) -> std::result::Result<(), Vec<String>> {
        let mut graph = Graph::new();
        let mut node_map: HashMap<String, NodeIndex> = HashMap::new();

        // Add all tasks as nodes
        for task_id in workflow.tasks.keys() {
            let node_id = graph.add_node(task_id.clone());
            node_map.insert(task_id.clone(), node_id);
        }

        // Add dependency edges
        for (task_id, task_config) in &workflow.tasks {
            let task_node = node_map[task_id];
            for dep in &task_config.depends_on {
                if let Some(&dep_node) = node_map.get(dep) {
                    graph.add_edge(dep_node, task_node, ());
                }
            }
        }

        // Use petgraph's cycle detection
        match petgraph::algo::toposort(&graph, None) {
            Ok(_) => Ok(()),
            Err(cycle) => {
                let cycle_tasks = vec![graph[cycle.node_id()].clone()];
                Err(cycle_tasks)
            }
        }
    }

    /// Validate individual task configurations
    fn validate_task_configs(
        &self,
        workflow: &Workflow,
        report: &mut ValidationReport,
    ) -> Result<()> {
        for (task_id, task_config) in &workflow.tasks {
            if let Err(error) = self.validate_single_task(task_id, task_config) {
                report.errors.push(error);
            }
        }
        Ok(())
    }

    /// Validate a single task configuration
    fn validate_single_task(&self, task_id: &str, task_config: &TaskConfig) -> std::result::Result<(), ValidationError> {
        // Validate basic structure
        if task_config.config.is_null() {
            return Err(ValidationError::InvalidTaskConfig {
                task: task_id.to_string(),
                reason: "task config cannot be null".to_string(),
            });
        }

        // Validate task type is supported if we have a task registry
        if let Some(ref registry) = self.task_registry {
            let task_type_str = match task_config.task_type {
                super::task::TaskType::Command => "command",
                super::task::TaskType::Compress => "compress",
                super::task::TaskType::Decompress => "decompress", 
                super::task::TaskType::Checksum => "checksum",
                super::task::TaskType::ValidateChecksum => "validate_checksum",
                super::task::TaskType::S3Upload => "s3_upload",
                super::task::TaskType::S3Download => "s3_download",
                super::task::TaskType::Email => "email",
                super::task::TaskType::Slack => "slack",
            };

            if registry.get_implementation(task_type_str).is_none() {
                let supported_types = registry.list_supported_tasks();
                return Err(ValidationError::UnsupportedTaskType {
                    task: task_id.to_string(),
                    task_type: task_type_str.to_string(),
                    supported_types: supported_types.into_iter().map(|s| s.to_string()).collect(),
                });
            }

            // Validate task configuration using the registry
            if let Err(e) = registry.validate_task_config(task_type_str, &task_config.config) {
                return Err(ValidationError::InvalidTaskConfig {
                    task: task_id.to_string(),
                    reason: e.to_string(),
                });
            }
        }

        // Validate retry configuration
        if let Some(ref retry) = task_config.retry_config {
            if retry.max_attempts == 0 {
                return Err(ValidationError::InvalidTaskConfig {
                    task: task_id.to_string(),
                    reason: "max_attempts must be greater than 0".to_string(),
                });
            }

            if retry.backoff_multiplier <= 0.0 {
                return Err(ValidationError::InvalidTaskConfig {
                    task: task_id.to_string(),
                    reason: "backoff_multiplier must be greater than 0".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Validate a single task configuration (detailed implementation)
    fn _validate_single_task_detailed(
        &self,
        _task_config: &TaskConfig,
    ) -> std::result::Result<(), String> {
        // This will be implemented once we have proper task parameter deserialization
        // For now, always return OK
        Ok(())
    }

    /// Basic template syntax validation
    fn validate_templates(&self, workflow: &Workflow, report: &mut ValidationReport) -> Result<()> {
        // This is a basic implementation - a full template engine would do more thorough validation
        for (task_id, task_config) in &workflow.tasks {
            if let Some(ref when_condition) = task_config.when {
                if let Err(error) = self.validate_template_syntax(when_condition) {
                    report.errors.push(ValidationError::InvalidTemplate {
                        field: format!("tasks.{}.when", task_id),
                        error,
                    });
                }
            }
        }

        Ok(())
    }

    /// Basic template syntax validation
    fn validate_template_syntax(&self, template: &str) -> std::result::Result<(), String> {
        let mut brace_count = 0;
        let mut in_template = false;

        for ch in template.chars() {
            match ch {
                '{' => {
                    brace_count += 1;
                    if brace_count == 2 {
                        in_template = true;
                    } else if brace_count > 2 && in_template {
                        return Err("nested template expressions not allowed".to_string());
                    }
                }
                '}' => {
                    if brace_count > 0 {
                        brace_count -= 1;
                        if brace_count == 0 {
                            in_template = false;
                        }
                    } else {
                        return Err("unmatched closing brace".to_string());
                    }
                }
                _ => {}
            }
        }

        if brace_count != 0 {
            return Err("unmatched template braces".to_string());
        }

        Ok(())
    }

    /// Validate output configuration
    fn validate_output_config(
        &self,
        workflow: &Workflow,
        report: &mut ValidationReport,
    ) -> Result<()> {
        let dest = &workflow.output.destination;

        // Basic URL validation
        if let Ok(url) = url::Url::parse(dest) {
            match url.scheme() {
                "file" => {
                    // Check if parent directory exists for file URLs
                    if self.strict_mode {
                        if let Ok(path) = url.to_file_path() {
                            if let Some(parent) = path.parent() {
                                if !parent.exists() {
                                    report.warnings.push(format!(
                                        "Output directory does not exist: {}",
                                        parent.display()
                                    ));
                                }
                            }
                        }
                    }
                }
                "s3" => {
                    // Validate S3 URL format
                    if url.host().is_none() {
                        report.errors.push(ValidationError::InvalidOutput {
                            reason: "S3 URL must include bucket name as host".to_string(),
                        });
                    }
                }
                _ => {
                    report.errors.push(ValidationError::InvalidOutput {
                        reason: format!("Unsupported output scheme: {}", url.scheme()),
                    });
                }
            }
        } else {
            report.errors.push(ValidationError::InvalidOutput {
                reason: "Invalid output destination URL".to_string(),
            });
        }

        Ok(())
    }

    /// Check for tasks that can never be executed
    fn check_unreachable_tasks(&self, workflow: &Workflow, report: &mut ValidationReport) {
        // Find tasks with no dependencies (root tasks)
        let root_tasks: Vec<String> = workflow
            .tasks
            .iter()
            .filter_map(|(task_id, task_config)| {
                if task_config.depends_on.is_empty() {
                    Some(task_id.clone())
                } else {
                    None
                }
            })
            .collect();

        if root_tasks.is_empty() && !workflow.tasks.is_empty() {
            report
                .warnings
                .push("No root tasks found - all tasks have dependencies".to_string());
            return;
        }

        // Find all reachable tasks using BFS
        let mut reachable = HashSet::new();
        let mut queue = VecDeque::from(root_tasks);

        while let Some(current) = queue.pop_front() {
            if reachable.insert(current.clone()) {
                // Add all dependent tasks to queue
                for dependent in workflow.get_dependent_tasks(&current) {
                    if !reachable.contains(&dependent) {
                        queue.push_back(dependent);
                    }
                }
            }
        }

        // Find unreachable tasks
        for task_id in workflow.tasks.keys() {
            if !reachable.contains(task_id) {
                report
                    .warnings
                    .push(format!("Task '{}' is unreachable", task_id));
            }
        }
    }
}

impl Default for ValidationReport {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidationReport {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
            is_valid: true,
        }
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

impl Default for WorkflowValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::workflow::Workflow;

    #[test]
    fn test_circular_dependency_detection() {
        let yaml = r#"
name: circular_test
tasks:
  task_a:
    type: command
    depends_on: [task_b]
    config:
      command: echo
  task_b:
    type: command
    depends_on: [task_a]
    config:
      command: echo
output:
  destination: "file://./output.json"
"#;

        let workflow = Workflow::from_yaml(yaml).unwrap();
        let validator = WorkflowValidator::new();
        let report = validator.validate(&workflow).unwrap();

        assert!(report.has_errors());
        assert!(matches!(
            report.errors[0],
            ValidationError::CircularDependency { .. }
        ));
    }

    #[test]
    fn test_unknown_dependency() {
        let yaml = r#"
name: unknown_dep_test
tasks:
  task_a:
    type: command
    depends_on: [nonexistent_task]
    config:
      command: echo
output:
  destination: "file://./output.json"
"#;

        let workflow = Workflow::from_yaml(yaml).unwrap();
        let validator = WorkflowValidator::new();
        let report = validator.validate(&workflow).unwrap();

        assert!(report.has_errors());
        assert!(matches!(
            report.errors[0],
            ValidationError::UnknownDependency { .. }
        ));
    }

    #[test]
    fn test_valid_workflow() {
        let yaml = r#"
name: valid_test
tasks:
  first:
    type: command
    config:
      command: echo
      args: ["Hello"]
  second:
    type: command
    depends_on: [first]
    config:
      command: echo
      args: ["World"]
output:
  destination: "file://./output.json"
"#;

        let workflow = Workflow::from_yaml(yaml).unwrap();
        let validator = WorkflowValidator::new();
        let report = validator.validate(&workflow).unwrap();

        assert!(!report.has_errors());
        assert!(report.is_valid);
    }
}
