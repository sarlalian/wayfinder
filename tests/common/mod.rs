// ABOUTME: Common utilities and helpers for integration tests
// ABOUTME: Provides shared functionality for setting up test environments and workflows

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tokio::fs;

use wayfinder::engine::{TaskResult, TaskStatus, WorkflowResult};

pub struct TestWorkflowBuilder {
    name: String,
    description: String,
    version: String,
    variables: HashMap<String, String>,
    tasks: Vec<TestTask>,
}

pub struct TestTask {
    pub id: String,
    pub task_type: String,
    pub command: String,
    pub args: Vec<String>,
    pub depends_on: Vec<String>,
    pub timeout: String,
    pub retry_attempts: Option<u32>,
}

impl TestWorkflowBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: format!("Test workflow: {}", name),
            version: "1.0".to_string(),
            variables: HashMap::new(),
            tasks: Vec::new(),
        }
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = description.to_string();
        self
    }

    pub fn with_variable(mut self, key: &str, value: &str) -> Self {
        self.variables.insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_task(mut self, task: TestTask) -> Self {
        self.tasks.push(task);
        self
    }

    pub fn add_echo_task(mut self, id: &str, message: &str) -> Self {
        let task = TestTask {
            id: id.to_string(),
            task_type: "command".to_string(),
            command: "echo".to_string(),
            args: vec![message.to_string()],
            depends_on: Vec::new(),
            timeout: "30s".to_string(),
            retry_attempts: None,
        };
        self.tasks.push(task);
        self
    }

    pub fn add_dependent_task(mut self, id: &str, message: &str, depends_on: Vec<&str>) -> Self {
        let task = TestTask {
            id: id.to_string(),
            task_type: "command".to_string(),
            command: "echo".to_string(),
            args: vec![message.to_string()],
            depends_on: depends_on.into_iter().map(|s| s.to_string()).collect(),
            timeout: "30s".to_string(),
            retry_attempts: None,
        };
        self.tasks.push(task);
        self
    }

    pub fn add_failing_task(mut self, id: &str) -> Self {
        let task = TestTask {
            id: id.to_string(),
            task_type: "command".to_string(),
            command: "false".to_string(),
            args: Vec::new(),
            depends_on: Vec::new(),
            timeout: "10s".to_string(),
            retry_attempts: None,
        };
        self.tasks.push(task);
        self
    }

    pub fn add_retry_task(mut self, id: &str, max_attempts: u32) -> Self {
        let task = TestTask {
            id: id.to_string(),
            task_type: "command".to_string(),
            command: "sh".to_string(),
            args: vec!["-c".to_string(), "exit 1".to_string()],
            depends_on: Vec::new(),
            timeout: "10s".to_string(),
            retry_attempts: Some(max_attempts),
        };
        self.tasks.push(task);
        self
    }

    pub async fn write_to_file(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let yaml_content = self.generate_yaml();
        fs::write(path, yaml_content).await?;
        Ok(())
    }

    fn generate_yaml(&self) -> String {
        let mut yaml = format!(
            "name: {}\ndescription: \"{}\"\nversion: \"{}\"\n\n",
            self.name, self.description, self.version
        );

        if !self.variables.is_empty() {
            yaml.push_str("variables:\n");
            for (key, value) in &self.variables {
                yaml.push_str(&format!("  {}: \"{}\"\n", key, value));
            }
            yaml.push('\n');
        }

        yaml.push_str("tasks:\n");
        for task in &self.tasks {
            yaml.push_str(&format!("  {}:\n", task.id));
            yaml.push_str(&format!("    type: {}\n", task.task_type));
            yaml.push_str("    config:\n");
            yaml.push_str(&format!("      command: {}\n", task.command));

            if !task.args.is_empty() {
                yaml.push_str("      args:\n");
                for arg in &task.args {
                    yaml.push_str(&format!("        - \"{}\"\n", arg));
                }
            }

            if !task.depends_on.is_empty() {
                yaml.push_str("    depends_on:\n");
                for dep in &task.depends_on {
                    yaml.push_str(&format!("      - {}\n", dep));
                }
            }

            yaml.push_str(&format!("    timeout: {}\n", task.timeout));

            if let Some(attempts) = task.retry_attempts {
                yaml.push_str("    retry:\n");
                yaml.push_str(&format!("      max_attempts: {}\n", attempts));
                yaml.push_str("      delay: 1s\n");
                yaml.push_str("      backoff_multiplier: 2.0\n");
            }

            yaml.push('\n');
        }

        // Add required output section
        yaml.push_str("\noutput:\n  format: json\n  destination: console\n");

        yaml
    }
}

pub struct TestEnvironment {
    pub temp_dir: TempDir,
}

impl TestEnvironment {
    pub fn new() -> Self {
        Self {
            temp_dir: TempDir::new().expect("Failed to create temp directory"),
        }
    }

    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }

    pub fn workflow_file(&self, name: &str) -> PathBuf {
        self.path().join(format!("{}.yaml", name))
    }

    pub fn output_file(&self, name: &str) -> PathBuf {
        self.path().join(format!("{}_output.json", name))
    }

    pub async fn create_workflow_file(&self, name: &str, builder: &TestWorkflowBuilder) -> PathBuf {
        let workflow_file = self.workflow_file(name);
        builder
            .write_to_file(&workflow_file)
            .await
            .expect("Failed to write workflow file");
        workflow_file
    }
}

pub fn create_mock_workflow_result(name: &str, run_id: &str) -> WorkflowResult {
    let mut result = WorkflowResult::new(name.to_string(), run_id.to_string());

    // Add a successful task
    let mut success_task = TaskResult::new("success_task".to_string(), "command".to_string());
    success_task.mark_completed(
        TaskStatus::Success,
        Some("Task completed".to_string()),
        None,
    );
    result.add_task_result(success_task);

    result.mark_completed();
    result
}

pub fn create_mock_workflow_result_with_failure(name: &str, run_id: &str) -> WorkflowResult {
    let mut result = WorkflowResult::new(name.to_string(), run_id.to_string());

    // Add a successful task
    let mut success_task = TaskResult::new("success_task".to_string(), "command".to_string());
    success_task.mark_completed(
        TaskStatus::Success,
        Some("Task completed".to_string()),
        None,
    );
    result.add_task_result(success_task);

    // Add a failed task
    let mut failed_task = TaskResult::new("failed_task".to_string(), "command".to_string());
    failed_task.mark_completed(TaskStatus::Failed, None, Some("Task failed".to_string()));
    result.add_task_result(failed_task);

    result.mark_completed();
    result
}

pub fn create_complex_workflow_result(name: &str, run_id: &str) -> WorkflowResult {
    let mut result = WorkflowResult::new(name.to_string(), run_id.to_string());

    // Add various task types
    let mut success_task = TaskResult::new("success_task".to_string(), "command".to_string());
    success_task.mark_completed(
        TaskStatus::Success,
        Some("Success output".to_string()),
        None,
    );
    result.add_task_result(success_task);

    let mut failed_task = TaskResult::new("failed_task".to_string(), "command".to_string());
    failed_task.mark_completed(TaskStatus::Failed, None, Some("Failure error".to_string()));
    result.add_task_result(failed_task);

    let mut skipped_task = TaskResult::new("skipped_task".to_string(), "command".to_string());
    skipped_task.mark_completed(TaskStatus::Skipped, None, None);
    result.add_task_result(skipped_task);

    let mut retry_task = TaskResult::new("retry_task".to_string(), "command".to_string());
    retry_task.increment_retry();
    retry_task.increment_retry();
    retry_task.mark_completed(
        TaskStatus::Success,
        Some("Finally succeeded".to_string()),
        None,
    );
    result.add_task_result(retry_task);

    result.mark_completed();
    result
}

pub async fn wait_for_file_creation(file_path: &Path, max_wait_seconds: u64) -> bool {
    let max_iterations = max_wait_seconds * 10; // Check every 100ms

    for _ in 0..max_iterations {
        if file_path.exists() {
            return true;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    false
}

pub async fn read_json_output(
    file_path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(file_path).await?;
    let json: serde_json::Value = serde_json::from_str(&content)?;
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_builder() {
        let builder = TestWorkflowBuilder::new("test_workflow")
            .with_description("Test workflow description")
            .with_variable("env", "test")
            .add_echo_task("task1", "Hello World")
            .add_dependent_task("task2", "Dependent task", vec!["task1"]);

        let yaml = builder.generate_yaml();

        assert!(yaml.contains("name: test_workflow"));
        assert!(yaml.contains("description: \"Test workflow description\""));
        assert!(yaml.contains("env: \"test\""));
        assert!(yaml.contains("task1:"));
        assert!(yaml.contains("task2:"));
        assert!(yaml.contains("depends_on:"));
    }

    #[test]
    fn test_environment_setup() {
        let env = TestEnvironment::new();
        assert!(env.path().exists());

        let workflow_file = env.workflow_file("test");
        assert!(workflow_file.to_string_lossy().contains("test.yaml"));
    }

    #[test]
    fn test_mock_workflow_results() {
        let result = create_mock_workflow_result("test", "run1");
        assert_eq!(result.workflow_name, "test");
        assert_eq!(result.run_id, "run1");
        assert_eq!(result.summary.total_tasks, 1);
        assert_eq!(result.summary.successful_tasks, 1);

        let complex_result = create_complex_workflow_result("complex", "run2");
        assert_eq!(complex_result.summary.total_tasks, 4);
        assert_eq!(complex_result.summary.successful_tasks, 2);
        assert_eq!(complex_result.summary.failed_tasks, 1);
        assert_eq!(complex_result.summary.skipped_tasks, 1);
    }
}
