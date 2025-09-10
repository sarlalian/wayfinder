// ABOUTME: Command implementations for the wayfinder CLI
// ABOUTME: Handles execution of run, validate, init, and report commands

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::info;

use super::config::Config;
use crate::engine::executor::TaskExecutor;
use crate::parser::WorkflowParser;

/// Execute a workflow command
pub async fn run_workflow(
    workflow_path: PathBuf,
    vars: Vec<String>,
    dry_run: bool,
    _output: Option<String>,
    _max_concurrent: Option<usize>,
    _config: &Config,
) -> Result<()> {
    info!("Starting workflow execution: {}", workflow_path.display());

    // Parse variables
    let variables = parse_variables(&vars)?;
    info!("Parsed {} template variables", variables.len());

    // Load and parse workflow
    let workflow_parser = WorkflowParser::new();
    let workflow = workflow_parser
        .parse_file(&workflow_path)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to parse workflow: {}", e))?;
    info!("Loaded workflow: {}", workflow.name);

    if dry_run {
        // For dry run, just validate the workflow
        info!("Dry run - workflow validation successful");
        return Ok(());
    }

    // Create task executor with appropriate concurrency
    let max_concurrent_tasks = _max_concurrent.unwrap_or(4);
    let executor = TaskExecutor::new(max_concurrent_tasks)
        .map_err(|e| anyhow::anyhow!("Failed to create task executor: {}", e))?;

    // Execute workflow
    let workflow_result = executor
        .execute_workflow(&workflow, &variables)
        .await
        .map_err(|e| anyhow::anyhow!("Workflow execution failed: {}", e))?;

    // Output results
    if let Some(output_path) = _output {
        // Serialize workflow result to JSON and write to file
        let json_content = serde_json::to_string_pretty(&workflow_result)
            .map_err(|e| anyhow::anyhow!("Failed to serialize results to JSON: {}", e))?;

        std::fs::write(&output_path, json_content)
            .map_err(|e| anyhow::anyhow!("Failed to write output file '{}': {}", output_path, e))?;

        info!("Results written to: {}", output_path);
    } else {
        // Print to stdout
        println!(
            "Workflow '{}' completed with status: {}",
            workflow_result.workflow_name, workflow_result.status
        );

        for task_result in &workflow_result.tasks {
            println!("  Task '{}': {:?}", task_result.task_id, task_result.status);
            if let Some(ref output) = task_result.output {
                println!("    Output: {}", output.trim());
            }
        }
    }

    info!("Workflow execution completed");

    // Return error if workflow failed to ensure proper exit code
    use crate::engine::result::WorkflowStatus;
    match workflow_result.status {
        WorkflowStatus::Success => Ok(()),
        _ => Err(anyhow::anyhow!(
            "Workflow execution failed with status: {:?}",
            workflow_result.status
        )),
    }
}

/// Validate a workflow file
pub async fn validate_workflow(
    workflow_path: PathBuf,
    vars: Vec<String>,
    _config: &Config,
) -> Result<()> {
    info!("Validating workflow: {}", workflow_path.display());

    // Parse variables
    let variables = parse_variables(&vars)?;

    // Load and parse workflow
    let workflow_parser = WorkflowParser::new();
    let workflow = workflow_parser
        .parse_file(&workflow_path)
        .await
        .map_err(|e| anyhow::anyhow!("Workflow validation failed: {}", e))?;

    // Create executor to validate workflow structure
    let executor =
        TaskExecutor::new(1).map_err(|e| anyhow::anyhow!("Failed to create validator: {}", e))?;

    // Validate workflow
    executor
        .validate_workflow(&workflow)
        .map_err(|e| anyhow::anyhow!("Workflow validation failed: {}", e))?;

    println!("✓ Workflow '{}' is valid", workflow.name);
    println!("  Tasks: {}", workflow.tasks.len());
    println!("  Variables: {}", variables.len());

    info!("Workflow validation completed successfully");

    Ok(())
}

/// Initialize a new workflow file
pub async fn init_workflow(
    name: String,
    output_dir: PathBuf,
    template: String,
    _config: &Config,
) -> Result<()> {
    info!(
        "Initializing workflow '{}' in {}",
        name,
        output_dir.display()
    );

    // Ensure output directory exists
    if !output_dir.exists() {
        std::fs::create_dir_all(&output_dir)?;
    }

    let workflow_file = output_dir.join(format!("{}.yaml", name));

    if workflow_file.exists() {
        return Err(anyhow::anyhow!(
            "Workflow file already exists: {}",
            workflow_file.display()
        ));
    }

    let workflow_content = generate_workflow_template(&name, &template)?;
    std::fs::write(&workflow_file, workflow_content)?;

    info!("Created workflow file: {}", workflow_file.display());

    Ok(())
}

/// Generate a report from workflow results
pub async fn generate_report(config_path: PathBuf, _config: &Config) -> Result<()> {
    info!("Generating report from config: {}", config_path.display());

    // TODO: Load report configuration
    // TODO: Create reporting engine and generate report

    info!("Report generation completed");

    Ok(())
}

/// Parse variables from key=value format
fn parse_variables(vars: &[String]) -> Result<HashMap<String, String>> {
    let mut variables = HashMap::new();

    for var in vars {
        if let Some((key, value)) = var.split_once('=') {
            variables.insert(key.to_string(), value.to_string());
        } else {
            return Err(anyhow::anyhow!(
                "Invalid variable format '{}'. Expected 'key=value'",
                var
            ));
        }
    }

    Ok(variables)
}

/// Generate workflow template content
fn generate_workflow_template(name: &str, template_type: &str) -> Result<String> {
    match template_type {
        "basic" => Ok(generate_basic_template(name)),
        "complex" => Ok(generate_complex_template(name)),
        _ => Err(anyhow::anyhow!("Unknown template type: {}", template_type)),
    }
}

/// Generate a basic workflow template
fn generate_basic_template(name: &str) -> String {
    format!(
        r#"name: {}
description: A basic workflow template

variables:
  environment: "{{{{ env }}}}"
  timestamp: "{{{{ timestamp }}}}"

tasks:
  hello:
    type: command
    config:
      command: echo
      args: 
        - "Hello from {{{{ variables.environment }}}} at {{{{ variables.timestamp }}}}"
    required: true

output:
  destination: "file://./output/{{{{ workflow.name }}}}-{{{{ variables.timestamp }}}}.json"
"#,
        name
    )
}

/// Generate a complex workflow template
fn generate_complex_template(name: &str) -> String {
    format!(
        r#"name: {}
description: A complex workflow template with multiple task types

variables:
  environment: "{{{{ env }}}}"
  timestamp: "{{{{ timestamp }}}}"
  version: "1.0.0"

tasks:
  prepare:
    type: command
    config:
      command: mkdir
      args: ["-p", "./temp"]
    required: true

  create_file:
    type: command
    depends_on: [prepare]
    config:
      command: echo
      args: ["Sample content for {{{{ variables.environment }}}}"]
      stdout_file: "./temp/sample.txt"
    required: true

  compress_file:
    type: compress
    depends_on: [create_file]
    config:
      input_path: "./temp/sample.txt"
      output_path: "./temp/sample.txt.bz2"
      compression_type: bzip2
      preserve_original: false
    required: true

  checksum:
    type: checksum
    depends_on: [compress_file]
    config:
      input_path: "./temp/sample.txt.bz2"
      output_path: "./temp/sample.txt.bz2.checksum"
      algorithm: sha256
    required: false

  cleanup:
    type: command
    depends_on: [checksum]
    config:
      command: rm
      args: ["-rf", "./temp"]
    required: false

output:
  destination: "file://./output/{{{{ workflow.name }}}}-{{{{ variables.timestamp }}}}.json"

on_error:
  continue: false
  cleanup_tasks: [cleanup]
"#,
        name
    )
}
