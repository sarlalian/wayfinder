// ABOUTME: Integration tests for the complete wayfinder workflow engine
// ABOUTME: Tests end-to-end functionality including parsing, execution, and reporting

use tempfile::TempDir;
use tokio::fs;

use wayfinder::engine::{TaskStatus, WorkflowEngine, WorkflowResult};
use wayfinder::output::config::OutputConfig;
use wayfinder::output::{OutputHandler, OutputProcessor};
use wayfinder::parser::WorkflowParser;
use wayfinder::reporting::config::ReportingConfig;
use wayfinder::reporting::{ReportProcessor, ReportingEngine};

mod common;

#[tokio::test]
async fn test_simple_workflow_execution() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("simple_workflow.yaml");

    let workflow_yaml = r#"
name: simple_test_workflow
description: A simple workflow for testing
version: "1.0"

variables:
  message: "Hello from integration test"

tasks:
  echo_task:
    type: command
    config:
      command: echo
      args: ["{{variables.message}}"]
    timeout: 30s

  success_task:  
    type: command
    config:
      command: echo
      args: ["Task completed successfully"]
    depends_on:
      - echo_task
    timeout: 30s

output:
  destination: "file://./test_output.json"
"#;

    fs::write(&workflow_file, workflow_yaml).await.unwrap();

    // Parse workflow
    let parser = WorkflowParser::new();
    let workflow = parser.parse_file(&workflow_file).await.unwrap();

    assert_eq!(workflow.name, "simple_test_workflow");
    assert_eq!(workflow.tasks.len(), 2);

    // Execute workflow
    let mut engine = WorkflowEngine::new();
    let result = engine.execute_workflow(&workflow).await.unwrap();

    assert_eq!(result.workflow_name, "simple_test_workflow");
    assert_eq!(result.tasks.len(), 2);

    // Check task results
    let echo_task = result.get_task_result("echo_task").unwrap();
    assert_eq!(echo_task.status, TaskStatus::Success);
    assert!(echo_task
        .output
        .as_ref()
        .unwrap()
        .contains("Hello from integration test"));

    let success_task = result.get_task_result("success_task").unwrap();
    assert_eq!(success_task.status, TaskStatus::Success);
    assert!(success_task
        .output
        .as_ref()
        .unwrap()
        .contains("Task completed successfully"));
}

#[tokio::test]
async fn test_workflow_with_dependencies() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("dependency_workflow.yaml");

    let workflow_yaml = r#"
name: dependency_test_workflow
description: Testing task dependencies
version: "1.0"

tasks:
  task_a:
    type: command
    config:
      command: echo
      args: ["Task A executed"]
    timeout: 30s

  task_b:
    type: command
    config:
      command: echo
      args: ["Task B executed"]
    timeout: 30s

  task_c:
    type: command
    config:
      command: echo
      args: ["Task C executed after A and B"]
    depends_on:
      - task_a
      - task_b
    timeout: 30s

output:
  destination: "file://./test_output.json"
"#;

    fs::write(&workflow_file, workflow_yaml).await.unwrap();

    let parser = WorkflowParser::new();
    let workflow = parser.parse_file(&workflow_file).await.unwrap();

    let mut engine = WorkflowEngine::new();
    let result = engine.execute_workflow(&workflow).await.unwrap();

    // Debug output
    println!("Total tasks: {}", result.summary.total_tasks);
    println!("Successful tasks: {}", result.summary.successful_tasks);
    println!("Failed tasks: {}", result.summary.failed_tasks);
    println!("Skipped tasks: {}", result.summary.skipped_tasks);
    for task in &result.tasks {
        println!("Task {}: {:?}", task.task_id, task.status);
    }

    // All tasks should succeed
    assert_eq!(result.summary.successful_tasks, 3);
    assert_eq!(result.summary.failed_tasks, 0);

    // Check execution order - task_c should start after task_a and task_b
    let task_a = result.get_task_result("task_a").unwrap();
    let task_b = result.get_task_result("task_b").unwrap();
    let task_c = result.get_task_result("task_c").unwrap();

    assert!(task_c.start_time >= task_a.start_time);
    assert!(task_c.start_time >= task_b.start_time);
}

#[tokio::test]
async fn test_workflow_with_failure() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("failure_workflow.yaml");

    let workflow_yaml = r#"
name: failure_test_workflow
description: Testing workflow with failures
version: "1.0"

tasks:
  success_task:
    type: command
    config:
      command: echo
      args: ["This will succeed"]
    timeout: 30s

  failure_task:
    type: command
    config:
      command: false  # Command that always fails
    timeout: 30s

  dependent_task:
    type: command
    config:
      command: echo
      args: ["This should not run"]
    depends_on:
      - failure_task
    timeout: 30s

output:
  destination: "file://./test_output.json"
"#;

    fs::write(&workflow_file, workflow_yaml).await.unwrap();

    let parser = WorkflowParser::new();
    let workflow = parser.parse_file(&workflow_file).await.unwrap();

    let mut engine = WorkflowEngine::new();
    let result = engine.execute_workflow(&workflow).await.unwrap();

    // Should have 1 success, 1 failure, 1 skipped
    assert_eq!(result.summary.successful_tasks, 1);
    assert_eq!(result.summary.failed_tasks, 1);
    assert_eq!(result.summary.skipped_tasks, 1);

    let success_task = result.get_task_result("success_task").unwrap();
    assert_eq!(success_task.status, TaskStatus::Success);

    let failure_task = result.get_task_result("failure_task").unwrap();
    assert_eq!(failure_task.status, TaskStatus::Failed);

    let dependent_task = result.get_task_result("dependent_task").unwrap();
    assert_eq!(dependent_task.status, TaskStatus::Skipped);
}

#[tokio::test]
async fn test_workflow_with_retries() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("retry_workflow.yaml");

    let workflow_yaml = r#"
name: retry_test_workflow
description: Testing task retries
version: "1.0"

tasks:
  flaky_task:
    type: command
    config:
      command: sh
      args: ["-c", "exit 1"]  # Always fails for this test
    timeout: 10s
    retry_config:
      max_attempts: 3
      initial_delay: 1s
      backoff_multiplier: 2.0

output:
  destination: "file://./test_output.json"
"#;

    fs::write(&workflow_file, workflow_yaml).await.unwrap();

    let parser = WorkflowParser::new();
    let workflow = parser.parse_file(&workflow_file).await.unwrap();

    let mut engine = WorkflowEngine::new();
    let result = engine.execute_workflow(&workflow).await.unwrap();

    let flaky_task = result.get_task_result("flaky_task").unwrap();
    assert_eq!(flaky_task.status, TaskStatus::Failed);
    assert_eq!(flaky_task.retry_count, 3); // Should have retried 3 times
}

#[tokio::test]
async fn test_workflow_with_templates() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("template_workflow.yaml");

    let workflow_yaml = r#"
name: template_test_workflow
description: Testing template variable substitution
version: "1.0"

variables:
  user_name: "test_user"
  greeting: "Hello"
  count: "5"

tasks:
  template_task:
    type: command
    config:
      command: echo
      args: ["{{variables.greeting}} {{variables.user_name}}! Count: {{variables.count}}"]
    timeout: 30s

output:
  destination: "file://./test_output.json"
"#;

    fs::write(&workflow_file, workflow_yaml).await.unwrap();

    let parser = WorkflowParser::new();
    let workflow = parser.parse_file(&workflow_file).await.unwrap();

    let mut engine = WorkflowEngine::new();
    let result = engine.execute_workflow(&workflow).await.unwrap();

    let template_task = result.get_task_result("template_task").unwrap();
    assert_eq!(template_task.status, TaskStatus::Success);
    assert!(template_task
        .output
        .as_ref()
        .unwrap()
        .contains("Hello test_user! Count: 5"));
}

#[tokio::test]
async fn test_output_formatting() {
    let temp_dir = TempDir::new().unwrap();
    let output_file = temp_dir.path().join("test_output.json");

    // Create a simple workflow result
    let mut workflow_result =
        WorkflowResult::new("test_workflow".to_string(), "run_123".to_string());

    let mut task_result =
        wayfinder::engine::TaskResult::new("test_task".to_string(), "command".to_string());
    task_result.mark_completed(TaskStatus::Success, Some("Test output".to_string()), None);

    workflow_result.add_task_result(task_result);
    workflow_result.mark_completed();

    // Configure output to file
    let mut config = OutputConfig::default();
    config.format = "json".to_string();
    config.destinations = vec![wayfinder::output::config::OutputDestination::new_file(
        output_file.to_string_lossy().to_string(),
    )];
    config.options.include_task_results = true;
    config.options.include_timestamps = true;

    let output_handler = OutputHandler::new();
    output_handler
        .process_workflow_result(&workflow_result, &config)
        .await
        .unwrap();

    // Verify output file was created and contains expected data
    assert!(output_file.exists());
    let output_content = fs::read_to_string(&output_file).await.unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&output_content).unwrap();
    assert_eq!(parsed["workflow_name"].as_str().unwrap(), "test_workflow");
    assert_eq!(parsed["run_id"].as_str().unwrap(), "run_123");
    assert!(parsed["tasks"].is_array());
}

#[tokio::test]
async fn test_reporting_engine() {
    let _temp_dir = TempDir::new().unwrap();

    // Create a workflow result for reporting
    let mut workflow_result =
        WorkflowResult::new("reporting_test".to_string(), "run_456".to_string());

    let mut success_task =
        wayfinder::engine::TaskResult::new("success_task".to_string(), "command".to_string());
    success_task.mark_completed(TaskStatus::Success, Some("Success!".to_string()), None);

    let mut failed_task =
        wayfinder::engine::TaskResult::new("failed_task".to_string(), "command".to_string());
    failed_task.mark_completed(TaskStatus::Failed, None, Some("Task failed".to_string()));

    workflow_result.add_task_result(success_task);
    workflow_result.add_task_result(failed_task);
    workflow_result.mark_completed();

    // Configure reporting
    let mut reporting_config = ReportingConfig {
        workflow_completion_reports: true,
        ..Default::default()
    };

    let mut template = wayfinder::reporting::config::ReportTemplate::new(
        "test_template".to_string(),
        "summary".to_string(),
    );
    template.trigger_on_workflow_completion = true;
    template.delivery_channels =
        vec![wayfinder::reporting::config::DeliveryChannel::new_terminal()];

    reporting_config.templates.push(template);

    let reporting_engine = ReportingEngine::new();
    let result = reporting_engine
        .process_workflow_completion(&workflow_result, &reporting_config)
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_full_workflow_pipeline() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("pipeline_workflow.yaml");
    let output_file = temp_dir.path().join("pipeline_output.json");

    let workflow_yaml = r#"
name: full_pipeline_test
description: Complete pipeline test
version: "1.0"

variables:
  stage: "integration_test"
  
tasks:
  setup:
    type: command
    config:
      command: echo
      args: ["Setting up {{variables.stage}}"]
    timeout: 30s

  process:
    type: command
    config:
      command: echo
      args: ["Processing data for {{variables.stage}}"]
    depends_on:
      - setup
    timeout: 30s
    
  validate:
    type: command
    config:
      command: echo
      args: ["Validating results for {{variables.stage}}"]
    depends_on:
      - process
    timeout: 30s

  cleanup:
    type: command
    config:
      command: echo
      args: ["Cleaning up {{variables.stage}}"]
    depends_on:
      - validate
    timeout: 30s

output:
  destination: "file://./test_output.json"
"#;

    fs::write(&workflow_file, workflow_yaml).await.unwrap();

    // 1. Parse the workflow
    let parser = WorkflowParser::new();
    let workflow = parser.parse_file(&workflow_file).await.unwrap();

    // 2. Execute the workflow
    let mut engine = WorkflowEngine::new();
    let result = engine.execute_workflow(&workflow).await.unwrap();

    // 3. Output the results
    let mut config = OutputConfig::default();
    config.format = "json".to_string();
    config.destinations = vec![wayfinder::output::config::OutputDestination::new_file(
        output_file.to_string_lossy().to_string(),
    )];
    config.options.include_task_results = true;
    config.options.include_timestamps = true;
    config.options.include_duration = true;

    let output_handler = OutputHandler::new();
    output_handler
        .process_workflow_result(&result, &config)
        .await
        .unwrap();

    // 4. Generate reports
    let mut reporting_config = ReportingConfig {
        workflow_completion_reports: true,
        ..Default::default()
    };

    let mut template = wayfinder::reporting::config::ReportTemplate::new(
        "integration_report".to_string(),
        "detailed".to_string(),
    );
    template.trigger_on_workflow_completion = true;
    template.delivery_channels =
        vec![wayfinder::reporting::config::DeliveryChannel::new_terminal()];

    reporting_config.templates.push(template);

    let reporting_engine = ReportingEngine::new();
    reporting_engine
        .process_workflow_completion(&result, &reporting_config)
        .await
        .unwrap();

    // Verify everything worked
    assert_eq!(result.summary.total_tasks, 4);
    assert_eq!(result.summary.successful_tasks, 4);
    assert_eq!(result.summary.failed_tasks, 0);
    assert!(output_file.exists());

    // Verify output contains all expected data
    let output_content = fs::read_to_string(&output_file).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output_content).unwrap();

    assert_eq!(
        parsed["workflow_name"].as_str().unwrap(),
        "full_pipeline_test"
    );
    assert_eq!(parsed["tasks"].as_array().unwrap().len(), 4);
    assert_eq!(parsed["summary"]["total_tasks"], 4);
    assert_eq!(parsed["summary"]["successful_tasks"], 4);
}

#[tokio::test]
async fn test_concurrent_workflows() {
    let temp_dir = TempDir::new().unwrap();

    // Create multiple workflow files
    let workflows = vec![
        ("workflow1", "Concurrent workflow 1"),
        ("workflow2", "Concurrent workflow 2"),
        ("workflow3", "Concurrent workflow 3"),
    ];

    let mut handles = vec![];

    for (name, description) in workflows {
        let temp_dir_path = temp_dir.path().to_path_buf();
        let workflow_name = name.to_string();
        let workflow_desc = description.to_string();

        let handle = tokio::spawn(async move {
            let workflow_file = temp_dir_path.join(format!("{}.yaml", workflow_name));

            let workflow_yaml = format!(
                r#"
name: {}
description: {}
version: "1.0"

tasks:
  concurrent_task:
    type: command
    config:
      command: echo
      args: ["Executing {} concurrently"]
    timeout: 30s

output:
  destination: "file://./test_output.json"
"#,
                workflow_name, workflow_desc, workflow_name
            );

            fs::write(&workflow_file, workflow_yaml).await.unwrap();

            let parser = WorkflowParser::new();
            let workflow = parser.parse_file(&workflow_file).await.unwrap();

            let mut engine = WorkflowEngine::new();
            engine.execute_workflow(&workflow).await.unwrap()
        });

        handles.push(handle);
    }

    // Wait for all workflows to complete
    let results = futures::future::join_all(handles).await;

    // Verify all workflows completed successfully
    for result in results {
        let workflow_result = result.unwrap();
        assert_eq!(workflow_result.summary.successful_tasks, 1);
        assert_eq!(workflow_result.summary.failed_tasks, 0);
    }
}

#[tokio::test]
async fn test_complex_workflow_with_conditions() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("conditional_workflow.yaml");

    let workflow_yaml = r#"
name: conditional_test_workflow
description: Testing conditional execution
version: "1.0"

variables:
  environment: "test"
  run_cleanup: "true"

tasks:
  setup_env:
    type: command
    config:
      command: echo
      args: ["Setting up {{variables.environment}} environment"]
    timeout: 30s

  run_tests:
    type: command
    config:
      command: echo
      args: ["Running tests in {{variables.environment}}"]
    depends_on:
      - setup_env
    timeout: 30s

  generate_report:
    type: command
    config:
      command: echo
      args: ["Generating test report"]
    depends_on:
      - run_tests
    timeout: 30s

  cleanup:
    type: command
    config:
      command: echo
      args: ["Cleaning up {{variables.environment}} environment"]
    depends_on:
      - generate_report
    timeout: 30s

output:
  destination: "file://./test_output.json"
"#;

    fs::write(&workflow_file, workflow_yaml).await.unwrap();

    let parser = WorkflowParser::new();
    let workflow = parser.parse_file(&workflow_file).await.unwrap();

    let mut engine = WorkflowEngine::new();
    let result = engine.execute_workflow(&workflow).await.unwrap();

    // All tasks should execute in sequence
    assert_eq!(result.summary.total_tasks, 4);
    assert_eq!(result.summary.successful_tasks, 4);

    // Verify variable substitution worked
    let setup_task = result.get_task_result("setup_env").unwrap();
    assert!(setup_task
        .output
        .as_ref()
        .unwrap()
        .contains("test environment"));

    let tests_task = result.get_task_result("run_tests").unwrap();
    assert!(tests_task
        .output
        .as_ref()
        .unwrap()
        .contains("tests in test"));
}
