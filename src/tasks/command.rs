// ABOUTME: Command task implementation for executing shell commands and scripts
// ABOUTME: Supports both simple commands and complex templated shell scripts with full bash capabilities

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::process::Stdio;
use tempfile::NamedTempFile;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use tracing::{debug, error, info};

use super::TaskImplementation;
use crate::engine::error::{ExecutionError, Result};
use crate::engine::{ExecutionContext, TaskResult, TaskStatus};
use crate::template::TemplateEngine;

pub struct CommandTask;

/// Configuration for command task execution
///
/// Supports two execution modes:
/// 1. Simple command: Use `command` + `args` for single command execution
/// 2. Script mode: Use `script` for multi-line shell scripts with template processing
///
/// ## Examples
///
/// ### Simple Command
/// ```yaml
/// type: command
/// config:
///   command: echo
///   args: ["Hello, World!"]
/// ```
///
/// ### Shell Script with Templates
/// ```yaml
/// type: command
/// config:
///   script: |
///     BACKUP_DIR="/tmp/backup-{{variables.timestamp}}"
///     mkdir -p "$BACKUP_DIR"
///     echo "Created backup directory: $BACKUP_DIR"
///     
///     # System information
///     uname -a > "$BACKUP_DIR/system-info.txt"
///     date >> "$BACKUP_DIR/system-info.txt"
///     
///     echo "✓ Backup completed in $BACKUP_DIR"
///   env:
///     PATH: "/usr/local/bin:/usr/bin:/bin"
///   timeout_seconds: 300
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandConfig {
    /// Command to execute (for simple command mode). Mutually exclusive with `script`.
    #[serde(default)]
    pub command: Option<String>,

    /// Arguments for the command (only used with `command`, ignored in script mode)
    #[serde(default)]
    pub args: Vec<String>,

    /// Multi-line shell script (for script mode). Supports full template processing.
    /// Mutually exclusive with `command`.
    #[serde(default)]
    pub script: Option<String>,

    /// Shell interpreter to use for script execution (default: /bin/bash)
    #[serde(default = "default_shell")]
    pub shell: String,

    /// Environment variables to set during execution
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Working directory for command/script execution
    #[serde(default)]
    pub working_dir: Option<String>,

    /// Maximum execution time in seconds (default: 300)
    #[serde(default)]
    pub timeout_seconds: Option<u64>,

    /// Whether to capture stdout/stderr (default: true)
    #[serde(default = "default_capture_output")]
    pub capture_output: bool,

    /// List of exit codes considered successful (default: [0])
    #[serde(default)]
    pub expected_exit_codes: Vec<i32>,
}

fn default_capture_output() -> bool {
    true
}

fn default_shell() -> String {
    "/bin/bash".to_string()
}

impl Default for CommandConfig {
    fn default() -> Self {
        Self {
            command: None,
            args: Vec::new(),
            script: None,
            shell: default_shell(),
            env: HashMap::new(),
            working_dir: None,
            timeout_seconds: Some(300), // 5 minutes default
            capture_output: true,
            expected_exit_codes: vec![0], // Success by default
        }
    }
}

#[async_trait]
impl TaskImplementation for CommandTask {
    async fn execute(
        &self,
        task_id: String,
        config: serde_yaml::Value,
        context: ExecutionContext,
    ) -> Result<TaskResult> {
        let start_time = chrono::Utc::now();

        let config: CommandConfig =
            serde_yaml::from_value(config).map_err(|e| ExecutionError::ConfigError {
                task_id: task_id.clone(),
                message: format!("Invalid command configuration: {}", e),
            })?;

        let mut task_result = TaskResult::new(task_id.clone(), "command".to_string());
        task_result.start_time = start_time;
        task_result.mark_started();

        // Determine execution mode: script or command
        let execution_result = if let Some(ref script) = config.script {
            info!(
                "Executing script task: {} - using {}",
                task_id, config.shell
            );
            self.execute_script(&config, script, &context, &mut task_result)
                .await
        } else if let Some(ref command) = config.command {
            info!("Executing command task: {} - {}", task_id, command);
            self.execute_command(&config, command, &mut task_result)
                .await
        } else {
            let error_msg =
                "Either 'command' or 'script' must be provided in command task configuration";
            error!("{}", error_msg);
            task_result.mark_completed(TaskStatus::Failed, None, Some(error_msg.to_string()));
            return Ok(task_result);
        };

        // Handle execution result
        self.handle_execution_result(execution_result, &config, &mut task_result)
            .await;

        Ok(task_result)
    }

    fn task_type(&self) -> &'static str {
        "command"
    }

    fn validate_config(&self, config: &serde_yaml::Value) -> Result<()> {
        let config: CommandConfig =
            serde_yaml::from_value(config.clone()).map_err(|e| ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: format!("Invalid command configuration: {}", e),
            })?;

        // Either command or script must be provided, but not both
        match (&config.command, &config.script) {
            (None, None) => {
                return Err(ExecutionError::ConfigError {
                    task_id: "validation".to_string(),
                    message: "Either 'command' or 'script' must be provided".to_string(),
                });
            }
            (Some(_cmd), Some(_)) => {
                return Err(ExecutionError::ConfigError {
                    task_id: "validation".to_string(),
                    message: "Cannot specify both 'command' and 'script' - use only one"
                        .to_string(),
                });
            }
            (Some(cmd), None) => {
                if cmd.is_empty() {
                    return Err(ExecutionError::ConfigError {
                        task_id: "validation".to_string(),
                        message: "Command cannot be empty".to_string(),
                    });
                }
            }
            (None, Some(script)) => {
                if script.is_empty() {
                    return Err(ExecutionError::ConfigError {
                        task_id: "validation".to_string(),
                        message: "Script cannot be empty".to_string(),
                    });
                }
                // Validate shell exists (basic check)
                if config.shell.is_empty() {
                    return Err(ExecutionError::ConfigError {
                        task_id: "validation".to_string(),
                        message: "Shell interpreter cannot be empty when using script mode"
                            .to_string(),
                    });
                }
            }
        }

        if let Some(timeout_secs) = config.timeout_seconds {
            if timeout_secs == 0 {
                return Err(ExecutionError::ConfigError {
                    task_id: "validation".to_string(),
                    message: "Timeout must be greater than 0".to_string(),
                });
            }
        }

        Ok(())
    }
}

impl CommandTask {
    /// Execute a script using the configured shell interpreter
    async fn execute_script(
        &self,
        config: &CommandConfig,
        script: &str,
        context: &ExecutionContext,
        task_result: &mut TaskResult,
    ) -> std::result::Result<std::process::Output, Box<dyn std::error::Error + Send + Sync>> {
        // Process script through template engine
        let template_engine = TemplateEngine::new()
            .map_err(|e| format!("Failed to create template engine: {}", e))?;

        let template_context = context
            .template_context
            .to_json()
            .map_err(|e| format!("Failed to convert template context: {}", e))?;

        let processed_script = template_engine
            .render_template(script, &template_context)
            .map_err(|e| format!("Failed to process script template: {}", e))?;

        debug!(
            "Processed script ({} chars): {}",
            processed_script.len(),
            if processed_script.len() > 200 {
                format!("{}...", &processed_script[..200])
            } else {
                processed_script.clone()
            }
        );

        // Create temporary script file
        let mut temp_file = NamedTempFile::new()
            .map_err(|e| format!("Failed to create temporary script file: {}", e))?;

        // Write script content
        use std::io::Write;
        temp_file
            .write_all(processed_script.as_bytes())
            .map_err(|e| format!("Failed to write script to temporary file: {}", e))?;

        // Make script executable
        let temp_path = temp_file.path();
        let mut perms = std::fs::metadata(temp_path)
            .map_err(|e| format!("Failed to get temporary file metadata: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(temp_path, perms)
            .map_err(|e| format!("Failed to make script executable: {}", e))?;

        // Add metadata about script execution
        task_result.add_metadata("shell".to_string(), config.shell.clone());
        task_result.add_metadata(
            "script_size".to_string(),
            processed_script.len().to_string(),
        );

        // Build and execute command
        let mut cmd = Command::new(&config.shell);
        cmd.arg(temp_path);

        self.configure_command(&mut cmd, config);

        // Keep temp file alive during execution
        let output = cmd.output().await;

        // Clean up happens automatically when temp_file is dropped
        output.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    /// Execute a traditional command with args
    async fn execute_command(
        &self,
        config: &CommandConfig,
        command: &str,
        task_result: &mut TaskResult,
    ) -> std::result::Result<std::process::Output, Box<dyn std::error::Error + Send + Sync>> {
        let mut cmd = Command::new(command);
        cmd.args(&config.args);

        self.configure_command(&mut cmd, config);

        // Add metadata
        task_result.add_metadata("command".to_string(), command.to_string());
        if !config.args.is_empty() {
            task_result.add_metadata("args".to_string(), config.args.join(" "));
        }

        debug!("Command: {} {:?}", command, config.args);

        cmd.output()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    /// Configure common command settings (env, working_dir, stdio)
    fn configure_command(&self, cmd: &mut Command, config: &CommandConfig) {
        // Set environment variables
        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        // Set working directory
        if let Some(ref working_dir) = config.working_dir {
            cmd.current_dir(working_dir);
        }

        // Configure output capture
        if config.capture_output {
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
        }
    }

    /// Handle the result of command/script execution
    async fn handle_execution_result(
        &self,
        result: std::result::Result<std::process::Output, Box<dyn std::error::Error + Send + Sync>>,
        config: &CommandConfig,
        task_result: &mut TaskResult,
    ) {
        // Apply timeout if configured
        let final_result = if let Some(timeout_secs) = config.timeout_seconds {
            match timeout(Duration::from_secs(timeout_secs), async { result }).await {
                Ok(res) => res,
                Err(_) => {
                    let error_msg = format!("Command timed out after {} seconds", timeout_secs);
                    error!("{}", error_msg);
                    task_result.mark_completed(TaskStatus::Timeout, None, Some(error_msg));
                    return;
                }
            }
        } else {
            result
        };

        match final_result {
            Ok(output) => {
                let exit_code = output.status.code().unwrap_or(-1);
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                debug!("Command completed with exit code: {}", exit_code);

                // Check if exit code is expected
                let is_success = config.expected_exit_codes.is_empty()
                    || config.expected_exit_codes.contains(&exit_code);

                let output_text = if !stdout.is_empty() {
                    if !stderr.is_empty() {
                        format!("STDOUT:\n{}\nSTDERR:\n{}", stdout, stderr)
                    } else {
                        stdout
                    }
                } else if !stderr.is_empty() {
                    format!("STDERR:\n{}", stderr)
                } else {
                    String::new()
                };

                // Add metadata
                task_result.add_metadata("exit_code".to_string(), exit_code.to_string());

                if is_success {
                    task_result.mark_completed(
                        TaskStatus::Success,
                        if output_text.is_empty() {
                            None
                        } else {
                            Some(output_text)
                        },
                        None,
                    );
                } else {
                    let error_msg = format!(
                        "Command exited with unexpected code: {} (expected one of: {:?})",
                        exit_code, config.expected_exit_codes
                    );
                    error!("{}", error_msg);
                    task_result.mark_completed(
                        TaskStatus::Failed,
                        if output_text.is_empty() {
                            None
                        } else {
                            Some(output_text)
                        },
                        Some(error_msg),
                    );
                }
            }
            Err(e) => {
                let error_msg = format!("Failed to execute: {}", e);
                error!("{}", error_msg);
                task_result.mark_completed(TaskStatus::Failed, None, Some(error_msg));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::TemplateContext;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_simple_command_execution() {
        let task = CommandTask;
        let variables = HashMap::new();
        let template_context = TemplateContext::new(&variables).unwrap();
        let context = ExecutionContext::new(
            "test_workflow".to_string(),
            "run_123".to_string(),
            "cmd_task".to_string(),
            template_context,
        );

        let config = serde_yaml::to_value(CommandConfig {
            command: Some("echo".to_string()),
            args: vec!["hello world".to_string()],
            ..Default::default()
        })
        .unwrap();

        let result = task
            .execute("test_cmd".to_string(), config, context)
            .await
            .unwrap();

        assert_eq!(result.status, TaskStatus::Success);
        assert!(result.output.is_some());
        assert!(result.output.unwrap().contains("hello world"));
    }

    #[tokio::test]
    async fn test_command_with_expected_exit_codes() {
        let task = CommandTask;
        let variables = HashMap::new();
        let template_context = TemplateContext::new(&variables).unwrap();
        let context = ExecutionContext::new(
            "test_workflow".to_string(),
            "run_123".to_string(),
            "cmd_task".to_string(),
            template_context,
        );

        // Test that exit code 1 is treated as success when expected
        let config = serde_yaml::to_value(CommandConfig {
            command: Some("bash".to_string()),
            args: vec!["-c".to_string(), "exit 1".to_string()],
            expected_exit_codes: vec![1],
            ..Default::default()
        })
        .unwrap();

        let result = task
            .execute("test_cmd".to_string(), config, context)
            .await
            .unwrap();

        assert_eq!(result.status, TaskStatus::Success);
        assert_eq!(result.metadata.get("exit_code"), Some(&"1".to_string()));
    }

    #[tokio::test]
    async fn test_command_validation() {
        let task = CommandTask;

        // Test empty command validation
        let empty_config = serde_yaml::to_value(CommandConfig {
            command: Some("".to_string()),
            ..Default::default()
        })
        .unwrap();

        let result = task.validate_config(&empty_config);
        assert!(result.is_err());

        // Test zero timeout validation
        let zero_timeout_config = serde_yaml::to_value(CommandConfig {
            command: Some("echo".to_string()),
            timeout_seconds: Some(0),
            ..Default::default()
        })
        .unwrap();

        let result = task.validate_config(&zero_timeout_config);
        assert!(result.is_err());

        // Test valid configuration
        let valid_config = serde_yaml::to_value(CommandConfig {
            command: Some("echo".to_string()),
            args: vec!["test".to_string()],
            timeout_seconds: Some(30),
            ..Default::default()
        })
        .unwrap();

        let result = task.validate_config(&valid_config);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_script_execution() {
        let task = CommandTask;
        let variables = HashMap::new();
        let template_context = TemplateContext::new(&variables).unwrap();
        let context = ExecutionContext::new(
            "test_workflow".to_string(),
            "run_123".to_string(),
            "script_task".to_string(),
            template_context,
        );

        let config = serde_yaml::to_value(CommandConfig {
            script: Some("echo 'Hello from script'\necho 'Line 2'".to_string()),
            shell: "/bin/bash".to_string(),
            ..Default::default()
        })
        .unwrap();

        let result = task
            .execute("test_script".to_string(), config, context)
            .await
            .unwrap();

        assert_eq!(result.status, TaskStatus::Success);
        assert!(result.output.is_some());
        let output = result.output.unwrap();
        assert!(output.contains("Hello from script"));
        assert!(output.contains("Line 2"));
    }

    #[tokio::test]
    async fn test_script_with_template_variables() {
        let task = CommandTask;
        let mut variables = HashMap::new();
        variables.insert("name".to_string(), "World".to_string());
        variables.insert("count".to_string(), "42".to_string());

        let template_context = TemplateContext::for_workflow(&variables, "test_workflow").unwrap();
        let context = ExecutionContext::new(
            "test_workflow".to_string(),
            "run_123".to_string(),
            "template_script_task".to_string(),
            template_context,
        );

        let config = serde_yaml::to_value(CommandConfig {
            script: Some(
                r#"
echo "Hello, {{variables.name}}!"
echo "Count: {{variables.count}}"
echo "Workflow: {{workflow.name}}"
            "#
                .to_string(),
            ),
            shell: "/bin/bash".to_string(),
            ..Default::default()
        })
        .unwrap();

        let result = task
            .execute("test_template_script".to_string(), config, context)
            .await
            .unwrap();

        assert_eq!(result.status, TaskStatus::Success);
        assert!(result.output.is_some());
        let output = result.output.unwrap();
        assert!(output.contains("Hello, World!"));
        assert!(output.contains("Count: 42"));
        assert!(output.contains("Workflow: test_workflow"));
    }

    #[tokio::test]
    async fn test_script_with_environment_variables() {
        let task = CommandTask;
        let variables = HashMap::new();
        let template_context = TemplateContext::new(&variables).unwrap();
        let context = ExecutionContext::new(
            "test_workflow".to_string(),
            "run_123".to_string(),
            "env_script_task".to_string(),
            template_context,
        );

        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "test_value".to_string());
        env.insert("ANOTHER_VAR".to_string(), "another_value".to_string());

        let config = serde_yaml::to_value(CommandConfig {
            script: Some(
                r#"
echo "TEST_VAR: $TEST_VAR"
echo "ANOTHER_VAR: $ANOTHER_VAR"
            "#
                .to_string(),
            ),
            shell: "/bin/bash".to_string(),
            env,
            ..Default::default()
        })
        .unwrap();

        let result = task
            .execute("test_env_script".to_string(), config, context)
            .await
            .unwrap();

        assert_eq!(result.status, TaskStatus::Success);
        assert!(result.output.is_some());
        let output = result.output.unwrap();
        assert!(output.contains("TEST_VAR: test_value"));
        assert!(output.contains("ANOTHER_VAR: another_value"));
    }

    #[tokio::test]
    async fn test_script_validation() {
        let task = CommandTask;

        // Test missing both command and script
        let no_command_script = serde_yaml::to_value(CommandConfig {
            command: None,
            script: None,
            ..Default::default()
        })
        .unwrap();

        assert!(task.validate_config(&no_command_script).is_err());

        // Test both command and script provided
        let both_config = serde_yaml::to_value(CommandConfig {
            command: Some("echo".to_string()),
            script: Some("echo 'test'".to_string()),
            ..Default::default()
        })
        .unwrap();

        assert!(task.validate_config(&both_config).is_err());

        // Test empty script
        let empty_script = serde_yaml::to_value(CommandConfig {
            script: Some("".to_string()),
            ..Default::default()
        })
        .unwrap();

        assert!(task.validate_config(&empty_script).is_err());

        // Test valid script config
        let valid_script = serde_yaml::to_value(CommandConfig {
            script: Some("echo 'test'".to_string()),
            shell: "/bin/bash".to_string(),
            ..Default::default()
        })
        .unwrap();

        assert!(task.validate_config(&valid_script).is_ok());
    }

    #[tokio::test]
    async fn test_script_with_exit_codes() {
        let task = CommandTask;
        let variables = HashMap::new();
        let template_context = TemplateContext::new(&variables).unwrap();
        let context = ExecutionContext::new(
            "test_workflow".to_string(),
            "run_123".to_string(),
            "exit_code_task".to_string(),
            template_context,
        );

        // Test script that exits with code 2, but we expect it
        let config = serde_yaml::to_value(CommandConfig {
            script: Some("echo 'Custom exit code'; exit 2".to_string()),
            shell: "/bin/bash".to_string(),
            expected_exit_codes: vec![2],
            ..Default::default()
        })
        .unwrap();

        let result = task
            .execute("test_exit_code".to_string(), config, context)
            .await
            .unwrap();

        assert_eq!(result.status, TaskStatus::Success);
        assert_eq!(result.metadata.get("exit_code"), Some(&"2".to_string()));
    }

    #[tokio::test]
    async fn test_backward_compatibility() {
        let task = CommandTask;
        let variables = HashMap::new();
        let template_context = TemplateContext::new(&variables).unwrap();
        let context = ExecutionContext::new(
            "test_workflow".to_string(),
            "run_123".to_string(),
            "compat_task".to_string(),
            template_context,
        );

        // Test that old-style command configs still work
        let config = serde_yaml::to_value(CommandConfig {
            command: Some("echo".to_string()),
            args: vec!["backward", "compatibility", "test"]
                .into_iter()
                .map(|s| s.to_string())
                .collect(),
            ..Default::default()
        })
        .unwrap();

        let result = task
            .execute("test_compat".to_string(), config, context)
            .await
            .unwrap();

        assert_eq!(result.status, TaskStatus::Success);
        assert!(result
            .output
            .unwrap()
            .contains("backward compatibility test"));
        assert_eq!(result.metadata.get("command"), Some(&"echo".to_string()));
        assert_eq!(
            result.metadata.get("args"),
            Some(&"backward compatibility test".to_string())
        );
    }
}
