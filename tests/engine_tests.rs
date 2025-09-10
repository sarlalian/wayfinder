// ABOUTME: Integration tests for the workflow execution engine
// ABOUTME: Tests task scheduling, dependency resolution, and workflow execution

use std::time::Duration;
use tempfile::TempDir;
use tokio::fs;

use wayfinder::engine::{TaskStatus, WorkflowEngine, WorkflowStatus};
use wayfinder::parser::WorkflowParser;

mod common;
use common::TestWorkflowBuilder;

#[tokio::test]
async fn test_engine_simple_execution() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("simple.yaml");

    let builder = TestWorkflowBuilder::new("simple_execution")
        .add_echo_task("task1", "Hello from engine test")
        .add_echo_task("task2", "Second task");

    builder.write_to_file(&workflow_file).await.unwrap();

    let parser = WorkflowParser::new();
    let workflow = parser.parse_file(&workflow_file).await.unwrap();

    let mut engine = WorkflowEngine::new();
    let result = engine.execute_workflow(&workflow).await.unwrap();

    assert_eq!(result.workflow_name, "simple_execution");
    assert_eq!(result.status, WorkflowStatus::Success);
    assert_eq!(result.summary.total_tasks, 2);
    assert_eq!(result.summary.successful_tasks, 2);
    assert_eq!(result.summary.failed_tasks, 0);

    // Check individual task results
    let task1 = result.get_task_result("task1").unwrap();
    assert_eq!(task1.status, TaskStatus::Success);
    assert!(task1
        .output
        .as_ref()
        .unwrap()
        .contains("Hello from engine test"));

    let task2 = result.get_task_result("task2").unwrap();
    assert_eq!(task2.status, TaskStatus::Success);
    assert!(task2.output.as_ref().unwrap().contains("Second task"));
}

#[tokio::test]
async fn test_engine_dependency_execution() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("dependencies.yaml");

    let builder = TestWorkflowBuilder::new("dependency_test")
        .add_echo_task("base", "Base task")
        .add_dependent_task("dependent1", "Dependent on base", vec!["base"])
        .add_dependent_task("dependent2", "Also dependent on base", vec!["base"])
        .add_dependent_task(
            "final",
            "Dependent on both",
            vec!["dependent1", "dependent2"],
        );

    builder.write_to_file(&workflow_file).await.unwrap();

    let parser = WorkflowParser::new();
    let workflow = parser.parse_file(&workflow_file).await.unwrap();

    let mut engine = WorkflowEngine::new();
    let result = engine.execute_workflow(&workflow).await.unwrap();

    assert_eq!(result.status, WorkflowStatus::Success);
    assert_eq!(result.summary.total_tasks, 4);
    assert_eq!(result.summary.successful_tasks, 4);

    // Verify execution order
    let base_task = result.get_task_result("base").unwrap();
    let dependent1 = result.get_task_result("dependent1").unwrap();
    let dependent2 = result.get_task_result("dependent2").unwrap();
    let final_task = result.get_task_result("final").unwrap();

    // Base should start first
    assert!(base_task.start_time <= dependent1.start_time);
    assert!(base_task.start_time <= dependent2.start_time);

    // Final should start after both dependents
    assert!(final_task.start_time >= dependent1.start_time);
    assert!(final_task.start_time >= dependent2.start_time);
}

#[tokio::test]
async fn test_engine_failure_handling() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("failure.yaml");

    let builder = TestWorkflowBuilder::new("failure_test")
        .add_echo_task("success1", "This will succeed")
        .add_failing_task("failure")
        .add_dependent_task("skipped", "This should be skipped", vec!["failure"])
        .add_echo_task("independent", "This should still run");

    builder.write_to_file(&workflow_file).await.unwrap();

    let parser = WorkflowParser::new();
    let workflow = parser.parse_file(&workflow_file).await.unwrap();

    let mut engine = WorkflowEngine::new();
    let result = engine.execute_workflow(&workflow).await.unwrap();

    // Should be partial success due to mixed results
    assert_eq!(result.status, WorkflowStatus::PartialSuccess);
    assert_eq!(result.summary.total_tasks, 4);
    assert_eq!(result.summary.successful_tasks, 2); // success1 and independent
    assert_eq!(result.summary.failed_tasks, 1); // failure
    assert_eq!(result.summary.skipped_tasks, 1); // skipped

    // Check individual task statuses
    let success_task = result.get_task_result("success1").unwrap();
    assert_eq!(success_task.status, TaskStatus::Success);

    let failure_task = result.get_task_result("failure").unwrap();
    assert_eq!(failure_task.status, TaskStatus::Failed);

    let skipped_task = result.get_task_result("skipped").unwrap();
    assert_eq!(skipped_task.status, TaskStatus::Skipped);

    let independent_task = result.get_task_result("independent").unwrap();
    assert_eq!(independent_task.status, TaskStatus::Success);
}

#[tokio::test]
async fn test_engine_retry_mechanism() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("retry.yaml");

    let builder = TestWorkflowBuilder::new("retry_test").add_retry_task("flaky", 3);

    builder.write_to_file(&workflow_file).await.unwrap();

    let parser = WorkflowParser::new();
    let workflow = parser.parse_file(&workflow_file).await.unwrap();

    let mut engine = WorkflowEngine::new();
    let result = engine.execute_workflow(&workflow).await.unwrap();

    // Task should fail after retries
    assert_eq!(result.status, WorkflowStatus::Failed);
    assert_eq!(result.summary.failed_tasks, 1);

    let flaky_task = result.get_task_result("flaky").unwrap();
    assert_eq!(flaky_task.status, TaskStatus::Failed);
    assert_eq!(flaky_task.retry_count, 3);
}

#[tokio::test]
async fn test_engine_timeout_handling() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("timeout.yaml");

    let workflow_yaml = r#"
name: timeout_test
version: "1.0"

tasks:
  timeout_task:
    type: command
    config:
      command: sleep
      args: ["10"]  # Sleep for 10 seconds
    timeout: 2s     # But timeout after 2 seconds

output:
  destination: "file://./test_output.json"
"#;

    fs::write(&workflow_file, workflow_yaml).await.unwrap();

    let parser = WorkflowParser::new();
    let workflow = parser.parse_file(&workflow_file).await.unwrap();

    let mut engine = WorkflowEngine::new();
    let result = engine.execute_workflow(&workflow).await.unwrap();

    assert_eq!(result.status, WorkflowStatus::Failed);

    let timeout_task = result.get_task_result("timeout_task").unwrap();
    assert_eq!(timeout_task.status, TaskStatus::Timeout);

    // Task should have completed quickly due to timeout
    let execution_time = timeout_task.duration.unwrap();
    assert!(execution_time >= Duration::from_secs(2));
    assert!(execution_time < Duration::from_secs(5)); // Should not take full 10 seconds
}

#[tokio::test]
async fn test_engine_variable_substitution() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("variables.yaml");

    let workflow_yaml = r#"
name: variable_test
version: "1.0"

variables:
  message: "Hello Variables"
  count: "42"
  environment: "test"

tasks:
  var_task:
    type: command
    config:
      command: echo
      args: ["{{variables.message}} - Count: {{variables.count}} - Env: {{variables.environment}}"]
    timeout: 30s

output:
  destination: "file://./test_output.json"
"#;

    fs::write(&workflow_file, workflow_yaml).await.unwrap();

    let parser = WorkflowParser::new();
    let workflow = parser.parse_file(&workflow_file).await.unwrap();

    let mut engine = WorkflowEngine::new();
    let result = engine.execute_workflow(&workflow).await.unwrap();

    assert_eq!(result.status, WorkflowStatus::Success);

    let var_task = result.get_task_result("var_task").unwrap();
    assert_eq!(var_task.status, TaskStatus::Success);

    let output = var_task.output.as_ref().unwrap();
    assert!(output.contains("Hello Variables"));
    assert!(output.contains("Count: 42"));
    assert!(output.contains("Env: test"));
}

#[tokio::test]
async fn test_engine_parallel_execution() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("parallel.yaml");

    let workflow_yaml = r#"
name: parallel_test
version: "1.0"

tasks:
  parallel1:
    type: command
    config:
      command: sleep
      args: ["1"]
    timeout: 10s
    
  parallel2:
    type: command
    config:
      command: sleep
      args: ["1"]
    timeout: 10s
    
  parallel3:
    type: command
    config:
      command: sleep
      args: ["1"]
    timeout: 10s

output:
  destination: "file://./test_output.json"
"#;

    fs::write(&workflow_file, workflow_yaml).await.unwrap();

    let parser = WorkflowParser::new();
    let workflow = parser.parse_file(&workflow_file).await.unwrap();

    let start_time = std::time::Instant::now();
    let mut engine = WorkflowEngine::new();
    let result = engine.execute_workflow(&workflow).await.unwrap();
    let total_time = start_time.elapsed();

    assert_eq!(result.status, WorkflowStatus::Success);
    assert_eq!(result.summary.successful_tasks, 3);

    // Should complete in roughly 1 second (parallel) rather than 3 seconds (sequential)
    assert!(total_time < Duration::from_secs(3));
    assert!(total_time >= Duration::from_secs(1));
}

#[tokio::test]
async fn test_engine_complex_dependency_graph() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("complex_deps.yaml");

    let workflow_yaml = r#"
name: complex_dependency_test
version: "1.0"

tasks:
  # Layer 1: No dependencies
  init_a:
    type: command
    config:
      command: echo
      args: ["Init A"]
    timeout: 30s
    
  init_b:
    type: command
    config:
      command: echo
      args: ["Init B"]
    timeout: 30s
    
  # Layer 2: Depend on layer 1
  process_a:
    type: command
    config:
      command: echo
      args: ["Process A"]
    depends_on:
      - init_a
    timeout: 30s
    
  process_b:
    type: command
    config:
      command: echo
      args: ["Process B"]
    depends_on:
      - init_b
    timeout: 30s
    
  # Layer 3: Cross dependencies
  combine:
    type: command
    config:
      command: echo
      args: ["Combine"]
    depends_on:
      - process_a
      - process_b
    timeout: 30s
    
  # Layer 4: Final tasks
  finalize_a:
    type: command
    config:
      command: echo
      args: ["Finalize A"]
    depends_on:
      - combine
    timeout: 30s
    
  finalize_b:
    type: command
    config:
      command: echo
      args: ["Finalize B"]
    depends_on:
      - combine
    timeout: 30s
    
  # Layer 5: Ultimate final task
  cleanup:
    type: command
    config:
      command: echo
      args: ["Cleanup"]
    depends_on:
      - finalize_a
      - finalize_b
    timeout: 30s

output:
  destination: "file://./test_output.json"
"#;

    fs::write(&workflow_file, workflow_yaml).await.unwrap();

    let parser = WorkflowParser::new();
    let workflow = parser.parse_file(&workflow_file).await.unwrap();

    let mut engine = WorkflowEngine::new();
    let result = engine.execute_workflow(&workflow).await.unwrap();

    assert_eq!(result.status, WorkflowStatus::Success);
    assert_eq!(result.summary.total_tasks, 8);
    assert_eq!(result.summary.successful_tasks, 8);

    // Verify execution order constraints
    let init_a = result.get_task_result("init_a").unwrap();
    let init_b = result.get_task_result("init_b").unwrap();
    let process_a = result.get_task_result("process_a").unwrap();
    let process_b = result.get_task_result("process_b").unwrap();
    let combine = result.get_task_result("combine").unwrap();
    let finalize_a = result.get_task_result("finalize_a").unwrap();
    let finalize_b = result.get_task_result("finalize_b").unwrap();
    let cleanup = result.get_task_result("cleanup").unwrap();

    // Layer 1 can run in parallel
    // Layer 2 must wait for layer 1
    assert!(process_a.start_time >= init_a.start_time);
    assert!(process_b.start_time >= init_b.start_time);

    // Layer 3 must wait for both layer 2 tasks
    assert!(combine.start_time >= process_a.start_time);
    assert!(combine.start_time >= process_b.start_time);

    // Layer 4 can run in parallel after layer 3
    assert!(finalize_a.start_time >= combine.start_time);
    assert!(finalize_b.start_time >= combine.start_time);

    // Layer 5 must wait for all layer 4
    assert!(cleanup.start_time >= finalize_a.start_time);
    assert!(cleanup.start_time >= finalize_b.start_time);
}

#[tokio::test]
async fn test_engine_workflow_metadata() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("metadata.yaml");

    let builder = TestWorkflowBuilder::new("metadata_test")
        .with_variable("test_var", "test_value")
        .add_echo_task("meta_task", "Task with metadata");

    builder.write_to_file(&workflow_file).await.unwrap();

    let parser = WorkflowParser::new();
    let workflow = parser.parse_file(&workflow_file).await.unwrap();

    let mut engine = WorkflowEngine::new();
    let result = engine.execute_workflow(&workflow).await.unwrap();

    // Check workflow result has proper metadata
    assert!(!result.run_id.is_empty());
    assert!(result.start_time <= result.end_time.unwrap());
    assert!(result.duration.is_some());

    // Check task metadata
    let task = result.get_task_result("meta_task").unwrap();
    assert!(!task.task_id.is_empty());
    assert_eq!(task.task_type, "command");
    assert!(task.start_time <= task.end_time.unwrap());
    assert!(task.duration.is_some());
    assert_eq!(task.retry_count, 0);
}

#[tokio::test]
async fn test_engine_concurrent_workflows() {
    let temp_dir = TempDir::new().unwrap();

    // Create multiple workflow files
    let workflow1_file = temp_dir.path().join("concurrent1.yaml");
    let workflow2_file = temp_dir.path().join("concurrent2.yaml");

    let builder1 =
        TestWorkflowBuilder::new("concurrent_workflow_1").add_echo_task("task1", "From workflow 1");

    let builder2 =
        TestWorkflowBuilder::new("concurrent_workflow_2").add_echo_task("task2", "From workflow 2");

    builder1.write_to_file(&workflow1_file).await.unwrap();
    builder2.write_to_file(&workflow2_file).await.unwrap();

    let parser = WorkflowParser::new();
    let workflow1 = parser.parse_file(&workflow1_file).await.unwrap();
    let workflow2 = parser.parse_file(&workflow2_file).await.unwrap();

    // Execute workflows concurrently
    let handle1 = {
        let mut engine = WorkflowEngine::new();
        let workflow = workflow1.clone();
        tokio::spawn(async move { engine.execute_workflow(&workflow).await.unwrap() })
    };

    let handle2 = {
        let mut engine = WorkflowEngine::new();
        let workflow = workflow2.clone();
        tokio::spawn(async move { engine.execute_workflow(&workflow).await.unwrap() })
    };

    let (result1, result2) = tokio::join!(handle1, handle2);

    let result1 = result1.unwrap();
    let result2 = result2.unwrap();

    // Both workflows should complete successfully
    assert_eq!(result1.status, WorkflowStatus::Success);
    assert_eq!(result2.status, WorkflowStatus::Success);

    assert_eq!(result1.workflow_name, "concurrent_workflow_1");
    assert_eq!(result2.workflow_name, "concurrent_workflow_2");
}
