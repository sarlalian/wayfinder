// ABOUTME: Integration tests for the CLI application
// ABOUTME: Tests command-line interface functionality and end-to-end workflow execution

use std::process::Command;
use tokio::fs;

mod common;
use common::{TestEnvironment, TestWorkflowBuilder};

#[tokio::test]
async fn test_cli_help_command() {
    let output = Command::new("cargo")
        .args(["run", "--", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should contain basic help information
    assert!(stdout.contains("wayfinder") || stdout.contains("Workflow"));
    assert!(stdout.contains("--help"));
}

#[tokio::test]
async fn test_cli_version_command() {
    let output = Command::new("cargo")
        .args(["run", "--", "--version"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should contain version information
    assert!(stdout.contains("0.1.0") || stdout.contains("version"));
}

#[tokio::test]
async fn test_cli_execute_simple_workflow() {
    let env = TestEnvironment::new();

    let builder = TestWorkflowBuilder::new("cli_simple_test")
        .with_description("Simple CLI test workflow")
        .add_echo_task("hello", "Hello from CLI test");

    let workflow_file = env.create_workflow_file("simple", &builder).await;

    let output = Command::new("cargo")
        .args(["run", "--", "run", workflow_file.to_str().unwrap()])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should contain execution results
    assert!(stdout.contains("cli_simple_test") || stdout.contains("Workflow"));
    assert!(stdout.contains("Success") || stdout.contains("completed"));
}

#[tokio::test]
async fn test_cli_execute_workflow_with_output_file() {
    let env = TestEnvironment::new();
    let output_file = env.output_file("cli_output_test");

    let builder =
        TestWorkflowBuilder::new("cli_output_test").add_echo_task("task1", "Output test task");

    let workflow_file = env.create_workflow_file("output_test", &builder).await;

    let command_output = Command::new("cargo")
        .args([
            "run",
            "--",
            "run",
            workflow_file.to_str().unwrap(),
            "--output",
            output_file.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");

    assert!(command_output.status.success());

    // Check that output file was created
    assert!(output_file.exists());
    let content = fs::read_to_string(&output_file).await.unwrap();

    // Should be valid JSON containing workflow results
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["workflow_name"].as_str().unwrap(), "cli_output_test");
}

#[tokio::test]
async fn test_cli_execute_workflow_with_variables() {
    let env = TestEnvironment::new();

    let builder = TestWorkflowBuilder::new("cli_variables_test")
        .with_variable("test_var", "original_value")
        .add_echo_task("var_task", "Variable: {{variables.test_var}}");

    let workflow_file = env.create_workflow_file("variables", &builder).await;

    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "run",
            workflow_file.to_str().unwrap(),
            "--var",
            "test_var=overridden_value",
        ])
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        println!(
            "VARIABLES STDOUT: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        println!(
            "VARIABLES STDERR: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    println!("VARIABLES OUTPUT: {}", stdout);
    // Should contain the overridden variable value
    assert!(stdout.contains("overridden_value"));
}

#[tokio::test]
async fn test_cli_dry_run() {
    let env = TestEnvironment::new();

    let builder = TestWorkflowBuilder::new("cli_dry_run_test")
        .add_echo_task("dry_task", "This should not execute");

    let workflow_file = env.create_workflow_file("dry_run", &builder).await;

    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "run",
            workflow_file.to_str().unwrap(),
            "--dry-run",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should indicate dry run mode
    assert!(stdout.contains("dry") || stdout.contains("preview") || stdout.contains("would"));
}

#[tokio::test]
async fn test_cli_validate_workflow() {
    let env = TestEnvironment::new();

    let builder = TestWorkflowBuilder::new("cli_validate_test")
        .add_echo_task("validate_task", "Validation test");

    let workflow_file = env.create_workflow_file("validate", &builder).await;

    let output = Command::new("cargo")
        .args(["run", "--", "validate", workflow_file.to_str().unwrap()])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should indicate validation passed
    assert!(stdout.contains("valid") || stdout.contains("Valid") || stdout.contains("passed"));
}

#[tokio::test]
async fn test_cli_validate_invalid_workflow() {
    let env = TestEnvironment::new();
    let workflow_file = env.workflow_file("invalid");

    // Create an invalid workflow (missing required fields)
    let invalid_yaml = r#"
# Missing name and other required fields
version: "1.0"
"#;

    fs::write(&workflow_file, invalid_yaml).await.unwrap();

    let output = Command::new("cargo")
        .args(["run", "--", "validate", workflow_file.to_str().unwrap()])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should contain error information
    assert!(
        stderr.contains("error")
            || stderr.contains("Error")
            || stderr.contains("invalid")
            || stderr.contains("Invalid")
    );
}

#[ignore] // list-tasks subcommand not implemented yet
#[tokio::test]
async fn test_cli_list_tasks() {
    let output = Command::new("cargo")
        .args(["run", "--", "list-tasks"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should list available task types
    assert!(stdout.contains("command") || stdout.contains("Task types"));
    assert!(stdout.contains("http") || stdout.contains("email") || stdout.contains("s3"));
}

#[tokio::test]
async fn test_cli_nonexistent_workflow() {
    let output = Command::new("cargo")
        .args(["run", "--", "run", "/nonexistent/workflow.yaml"])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should contain file not found error
    assert!(stderr.contains("not found") || stderr.contains("No such file"));
}

#[tokio::test]
async fn test_cli_workflow_with_dependencies() {
    let env = TestEnvironment::new();

    let builder = TestWorkflowBuilder::new("cli_dependencies_test")
        .add_echo_task("first", "First task")
        .add_dependent_task("second", "Second task depends on first", vec!["first"])
        .add_dependent_task("third", "Third task depends on second", vec!["second"]);

    let workflow_file = env.create_workflow_file("dependencies", &builder).await;

    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "run",
            workflow_file.to_str().unwrap(),
            "--verbose",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should show execution of all tasks
    assert!(stdout.contains("first") || stdout.contains("First"));
    assert!(stdout.contains("second") || stdout.contains("Second"));
    assert!(stdout.contains("third") || stdout.contains("Third"));
}

#[tokio::test]
async fn test_cli_workflow_with_failure() {
    let env = TestEnvironment::new();

    let builder = TestWorkflowBuilder::new("cli_failure_test")
        .add_echo_task("success_task", "This will succeed")
        .add_failing_task("failure_task");

    let workflow_file = env.create_workflow_file("failure", &builder).await;

    let output = Command::new("cargo")
        .args(["run", "--", "run", workflow_file.to_str().unwrap()])
        .output()
        .expect("Failed to execute command");

    // CLI should return error code for failed workflow
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should indicate workflow failure
    let combined_output = format!("{}{}", stdout, stderr);
    assert!(
        combined_output.contains("failed")
            || combined_output.contains("Failed")
            || combined_output.contains("error")
            || combined_output.contains("Error")
    );
}

#[tokio::test]
async fn test_cli_config_file() {
    let env = TestEnvironment::new();
    let config_file = env.path().join("wayfinder.yaml");

    // Create a simple config file
    let config_yaml = r#"
max_concurrent_tasks: 2
logging:
  level: info
  format: pretty
template_vars:
  test_env: "test_environment"
"#;

    fs::write(&config_file, config_yaml).await.unwrap();

    let builder = TestWorkflowBuilder::new("cli_config_test")
        .add_echo_task("config_task", "Config test task");

    let workflow_file = env.create_workflow_file("config", &builder).await;

    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "run",
            workflow_file.to_str().unwrap(),
            "--config",
            config_file.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
}

#[tokio::test]
async fn test_cli_concurrent_execution() {
    let env = TestEnvironment::new();

    let builder = TestWorkflowBuilder::new("cli_concurrent_test")
        .add_echo_task("concurrent1", "Concurrent task 1")
        .add_echo_task("concurrent2", "Concurrent task 2")
        .add_echo_task("concurrent3", "Concurrent task 3");

    let workflow_file = env.create_workflow_file("concurrent", &builder).await;

    let start_time = std::time::Instant::now();

    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "run",
            workflow_file.to_str().unwrap(),
            "--max-concurrent",
            "3",
        ])
        .output()
        .expect("Failed to execute command");

    let execution_time = start_time.elapsed();

    assert!(output.status.success());

    // With parallel execution, should complete reasonably quickly
    // (This is a rough check - exact timing depends on system performance)
    assert!(execution_time < std::time::Duration::from_secs(120));
}

#[tokio::test]
async fn test_cli_output_formats() {
    let env = TestEnvironment::new();
    let json_file = env.path().join("output.json");
    let yaml_file = env.path().join("output.yaml");

    let builder =
        TestWorkflowBuilder::new("cli_formats_test").add_echo_task("format_task", "Format test");

    let workflow_file = env.create_workflow_file("formats", &builder).await;

    // Test JSON output
    let json_output = Command::new("cargo")
        .args([
            "run",
            "--",
            "run",
            workflow_file.to_str().unwrap(),
            "--output",
            json_file.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute JSON command");

    assert!(json_output.status.success());
    assert!(json_file.exists());

    let json_content = fs::read_to_string(&json_file).await.unwrap();
    let _: serde_json::Value = serde_json::from_str(&json_content).unwrap();

    // Test YAML output
    let yaml_output = Command::new("cargo")
        .args([
            "run",
            "--",
            "run",
            workflow_file.to_str().unwrap(),
            "--output",
            yaml_file.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute YAML command");

    assert!(yaml_output.status.success());
    assert!(yaml_file.exists());

    let yaml_content = fs::read_to_string(&yaml_file).await.unwrap();
    let _: serde_yaml::Value = serde_yaml::from_str(&yaml_content).unwrap();
}
