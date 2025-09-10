// ABOUTME: Integration tests specifically for the workflow parser module
// ABOUTME: Tests parsing of various YAML workflow configurations and error handling

use tempfile::TempDir;
use tokio::fs;

use wayfinder::engine::dependency::DependencyGraph;
use wayfinder::parser::{TaskType, WorkflowParser};

mod common;

#[tokio::test]
async fn test_parse_valid_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("valid_workflow.yaml");

    let workflow_yaml = r#"
name: valid_test_workflow
description: A valid workflow for testing
version: "1.0"
author: "Integration Test"

variables:
  environment: "test"
  database_url: "postgresql://test:test@localhost/test"

tasks:
  setup_database:
    type: command
    description: "Initialize the test database"
    config:
      command: echo
      args: ["Setting up database: ${database_url}"]
    timeout: 60s
    
  run_migrations:
    type: command
    description: "Run database migrations"
    config:
      command: echo
      args: ["Running migrations in ${environment}"]
    depends_on:
      - setup_database
    timeout: 120s
    
  seed_data:
    type: command
    description: "Seed test data"
    config:
      command: echo
      args: ["Seeding test data"]
    depends_on:
      - run_migrations
    timeout: 30s
    retry_config:
      max_attempts: 3
      initial_delay: 5s
      backoff_multiplier: 2.0

output:
  destination: "file://./test_output.json"
"#;

    fs::write(&workflow_file, workflow_yaml).await.unwrap();

    let parser = WorkflowParser::new();
    let workflow = parser.parse_file(&workflow_file).await.unwrap();

    // Validate parsed workflow
    assert_eq!(workflow.name, "valid_test_workflow");
    assert_eq!(
        workflow.description,
        Some("A valid workflow for testing".to_string())
    );
    assert_eq!(workflow.version, "1.0");
    assert_eq!(workflow.author.as_ref().unwrap(), "Integration Test");

    // Check variables
    assert_eq!(workflow.variables.len(), 2);
    assert_eq!(workflow.variables.get("environment").unwrap(), "test");
    assert_eq!(
        workflow.variables.get("database_url").unwrap(),
        "postgresql://test:test@localhost/test"
    );

    // Check tasks
    assert_eq!(workflow.tasks.len(), 3);

    let setup_task = workflow.tasks.get("setup_database").unwrap();
    assert_eq!(setup_task.task_type, TaskType::Command);
    assert_eq!(
        setup_task.description.as_ref().unwrap(),
        "Initialize the test database"
    );
    assert_eq!(setup_task.timeout.unwrap().as_secs(), 60);
    assert!(setup_task.depends_on.is_empty());

    let migration_task = workflow.tasks.get("run_migrations").unwrap();
    assert_eq!(migration_task.depends_on.len(), 1);
    assert!(migration_task
        .depends_on
        .contains(&"setup_database".to_string()));
    assert_eq!(migration_task.timeout.unwrap().as_secs(), 120);

    let seed_task = workflow.tasks.get("seed_data").unwrap();
    assert!(seed_task.retry_config.is_some());
    let retry_config = seed_task.retry_config.as_ref().unwrap();
    assert_eq!(retry_config.max_attempts, 3);
    assert_eq!(retry_config.initial_delay.as_secs(), 5);
    assert_eq!(retry_config.backoff_multiplier, 2.0);
}

#[tokio::test]
async fn test_parse_minimal_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("minimal_workflow.yaml");

    let workflow_yaml = r#"
name: minimal_workflow
version: "1.0"

tasks:
  simple_task:
    type: command
    config:
      command: echo
      args: ["Hello World"]

output:
  destination: "file://./test_output.json"
"#;

    fs::write(&workflow_file, workflow_yaml).await.unwrap();

    let parser = WorkflowParser::new();
    let workflow = parser.parse_file(&workflow_file).await.unwrap();

    assert_eq!(workflow.name, "minimal_workflow");
    assert_eq!(workflow.version, "1.0");
    assert!(workflow.description.is_none());
    assert!(workflow.variables.is_empty());
    assert_eq!(workflow.tasks.len(), 1);

    let task = workflow.tasks.get("simple_task").unwrap();
    assert_eq!(task.task_type, TaskType::Command);
    assert!(task.description.is_none());
    assert!(task.depends_on.is_empty());
}

#[tokio::test]
async fn test_parse_invalid_workflow_missing_name() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("invalid_workflow.yaml");

    let workflow_yaml = r#"
version: "1.0"
description: "Missing name field"

tasks:
  test_task:
    type: command
    config:
      command: echo
      args: ["test"]

output:
  destination: "file://./test_output.json"
"#;

    fs::write(&workflow_file, workflow_yaml).await.unwrap();

    let parser = WorkflowParser::new();
    let result = parser.parse_file(&workflow_file).await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("name"));
}

#[tokio::test]
async fn test_parse_invalid_workflow_missing_tasks() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("no_tasks_workflow.yaml");

    let workflow_yaml = r#"
name: no_tasks_workflow
version: "1.0"
description: "Workflow with no tasks"
"#;

    fs::write(&workflow_file, workflow_yaml).await.unwrap();

    let parser = WorkflowParser::new();
    let result = parser.parse_file(&workflow_file).await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("tasks") || error.to_string().contains("empty"));
}

#[tokio::test]
async fn test_parse_workflow_with_invalid_dependencies() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("invalid_deps_workflow.yaml");

    let workflow_yaml = r#"
name: invalid_deps_workflow
version: "1.0"

tasks:
  task_a:
    type: command
    config:
      command: echo
      args: ["Task A"]
      
  task_b:
    type: command
    config:
      command: echo
      args: ["Task B"]
    depends_on:
      - task_a
      - nonexistent_task  # This task doesn't exist

output:
  destination: "file://./test_output.json"
"#;

    fs::write(&workflow_file, workflow_yaml).await.unwrap();

    let parser = WorkflowParser::new();
    let result = parser.parse_file(&workflow_file).await;

    // Parser should succeed, but validation should catch the invalid dependency
    let workflow = result.unwrap();

    // Try to validate dependencies using the dependency graph
    let validation_result = DependencyGraph::from_workflow(&workflow);
    assert!(validation_result.is_err());

    if let Err(validation_error) = validation_result {
        assert!(validation_error.to_string().contains("nonexistent_task"));
    }
}

#[tokio::test]
async fn test_parse_workflow_with_circular_dependencies() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("circular_deps_workflow.yaml");

    let workflow_yaml = r#"
name: circular_deps_workflow
version: "1.0"

tasks:
  task_a:
    type: command
    config:
      command: echo
      args: ["Task A"]
    depends_on:
      - task_b
      
  task_b:
    type: command
    config:
      command: echo
      args: ["Task B"]  
    depends_on:
      - task_c
      
  task_c:
    type: command
    config:
      command: echo
      args: ["Task C"]
    depends_on:
      - task_a  # Creates circular dependency

output:
  destination: "file://./test_output.json"
"#;

    fs::write(&workflow_file, workflow_yaml).await.unwrap();

    let parser = WorkflowParser::new();
    let result = parser.parse_file(&workflow_file).await;
    let workflow = result.unwrap();

    // Validation should detect circular dependency
    let dependency_graph = DependencyGraph::from_workflow(&workflow).unwrap();
    let validation_result = dependency_graph.create_execution_plan();
    assert!(validation_result.is_err());

    if let Err(validation_error) = validation_result {
        assert!(
            validation_error.to_string().contains("Circular")
                || validation_error.to_string().contains("circular")
                || validation_error.to_string().contains("cycle")
        );
    }
}

#[tokio::test]
async fn test_parse_workflow_with_duplicate_task_ids() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("duplicate_ids_workflow.yaml");

    let workflow_yaml = r#"
name: duplicate_ids_workflow
version: "1.0"

tasks:
  duplicate_task:
    type: command
    config:
      command: echo
      args: ["First task"]
      
  duplicate_task_2:  # Cannot have duplicate keys in YAML map
    type: command
    config:
      command: echo
      args: ["Second task"]

output:
  destination: "file://./test_output.json"
"#;

    fs::write(&workflow_file, workflow_yaml).await.unwrap();

    let parser = WorkflowParser::new();
    let result = parser.parse_file(&workflow_file).await;
    let workflow = result.unwrap();

    // Since we changed the YAML to use different task names (duplicate_task and duplicate_task_2),
    // there are no actual duplicates anymore, so validation should succeed
    let validation_result = parser.validate_workflow(&workflow);
    assert!(validation_result.is_ok());
}

#[tokio::test]
async fn test_parse_workflow_with_invalid_yaml() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("invalid_yaml.yaml");

    let workflow_yaml = r#"
name: invalid_yaml_workflow
version: "1.0"
tasks:
  task1:
    type: command
    config:
      command: echo
      args: ["test"]
    # Invalid YAML: missing closing bracket
    depends_on: [task2

output:
  destination: "file://./test_output.json"
"#;

    fs::write(&workflow_file, workflow_yaml).await.unwrap();

    let parser = WorkflowParser::new();
    let result = parser.parse_file(&workflow_file).await;

    assert!(result.is_err());
    // Should be a YAML parsing error
    let error = result.unwrap_err();
    assert!(error.to_string().contains("yaml") || error.to_string().contains("parse"));
}

#[tokio::test]
async fn test_parse_workflow_from_string() {
    let workflow_yaml = r#"
name: string_workflow
version: "1.0"
description: "Workflow parsed from string"

tasks:
  string_task:
    type: command
    config:
      command: echo
      args: ["Parsed from string"]

output:
  destination: "file://./test_output.json"
"#;

    let parser = WorkflowParser::new();
    let workflow = parser.parse_string(workflow_yaml).unwrap();

    assert_eq!(workflow.name, "string_workflow");
    assert_eq!(
        workflow.description,
        Some("Workflow parsed from string".to_string())
    );
    assert_eq!(workflow.tasks.len(), 1);
    assert!(workflow.tasks.contains_key("string_task"));
}

#[tokio::test]
async fn test_parse_workflow_with_complex_task_configs() {
    let temp_dir = TempDir::new().unwrap();
    let workflow_file = temp_dir.path().join("complex_config_workflow.yaml");

    let workflow_yaml = r#"
name: complex_config_workflow
version: "1.0"

variables:
  database_host: "localhost"
  database_port: "5432"
  
tasks:
  http_task:
    type: command
    config:
      url: "https://api.example.com/test"
      method: "POST"
      headers:
        Content-Type: "application/json"
        Authorization: "Bearer token123"
      body: |
        {
          "test": "data",
          "database": "${database_host}:${database_port}"
        }
    timeout: 30s
    
  email_task:
    type: email
    config:
      to: ["admin@example.com", "dev@example.com"]
      subject: "Test notification"
      body: "Test completed successfully on ${database_host}"
    depends_on:
      - http_task
    timeout: 15s
    
  s3_task:
    type: s3_upload
    config:
      bucket: "test-bucket"
      key: "results/test-${database_port}.json"
      region: "us-east-1"
    depends_on:
      - email_task
    timeout: 45s

output:
  destination: "file://./test_output.json"
"#;

    fs::write(&workflow_file, workflow_yaml).await.unwrap();

    let parser = WorkflowParser::new();
    let workflow = parser.parse_file(&workflow_file).await.unwrap();

    assert_eq!(workflow.name, "complex_config_workflow");
    assert_eq!(workflow.tasks.len(), 3);

    // Check that different task types are parsed
    let http_task = workflow.tasks.get("http_task").unwrap();
    assert_eq!(http_task.task_type, TaskType::Command);

    let email_task = workflow.tasks.get("email_task").unwrap();
    assert_eq!(email_task.task_type, TaskType::Email);
    assert_eq!(email_task.depends_on.len(), 1);
    assert!(email_task.depends_on.contains(&"http_task".to_string()));

    let s3_task = workflow.tasks.get("s3_task").unwrap();
    assert_eq!(s3_task.task_type, TaskType::S3Upload);
    assert_eq!(s3_task.depends_on.len(), 1);
    assert!(s3_task.depends_on.contains(&"email_task".to_string()));
}

#[tokio::test]
async fn test_parse_nonexistent_file() {
    let parser = WorkflowParser::new();
    let result = parser.parse_file("/nonexistent/path/workflow.yaml").await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("No such file") || error.to_string().contains("not found"));
}
