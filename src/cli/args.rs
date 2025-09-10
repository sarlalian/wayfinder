// ABOUTME: Command line argument definitions and parsing using Clap
// ABOUTME: Defines the main CLI structure and subcommands for wayfinder

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "wayfinder")]
#[command(about = "A CLI-based workflow engine for executing declarative YAML workflows")]
#[command(version)]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(short, long, global = true, help = "Enable verbose output")]
    pub verbose: bool,

    #[arg(short, long, global = true, help = "Path to configuration file")]
    pub config: Option<PathBuf>,

    #[arg(long, global = true, help = "Disable colored output")]
    pub no_color: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Execute a workflow from a YAML file
    Run {
        #[arg(help = "Path to workflow YAML file")]
        workflow: PathBuf,

        #[arg(
            short = 'V',
            long = "var",
            help = "Override template variables (key=value)"
        )]
        vars: Vec<String>,

        #[arg(long, help = "Dry run - validate without executing")]
        dry_run: bool,

        #[arg(short, long, help = "Output file or destination URL")]
        output: Option<String>,

        #[arg(long, help = "Maximum number of concurrent tasks")]
        max_concurrent: Option<usize>,
    },

    /// Validate a workflow file without executing
    Validate {
        #[arg(help = "Path to workflow YAML file")]
        workflow: PathBuf,

        #[arg(long = "var", help = "Template variables for validation (key=value)")]
        vars: Vec<String>,
    },

    /// Initialize a new workflow file from template
    Init {
        #[arg(help = "Name of the workflow to create")]
        name: String,

        #[arg(short, long, help = "Output directory", default_value = ".")]
        output_dir: PathBuf,

        #[arg(long, help = "Workflow template type", default_value = "basic")]
        template: String,
    },

    /// Generate reports from workflow results
    Report {
        #[arg(help = "Path to report configuration YAML")]
        config: PathBuf,
    },
}

impl Args {
    /// Parse command line arguments
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// Parse variables from key=value format
    pub fn parse_variables(
        vars: &[String],
    ) -> anyhow::Result<std::collections::HashMap<String, String>> {
        let mut variables = std::collections::HashMap::new();

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_variables() {
        let vars = vec![
            "env=production".to_string(),
            "version=1.0.0".to_string(),
            "debug=true".to_string(),
        ];

        let parsed = Args::parse_variables(&vars).unwrap();

        assert_eq!(parsed.get("env"), Some(&"production".to_string()));
        assert_eq!(parsed.get("version"), Some(&"1.0.0".to_string()));
        assert_eq!(parsed.get("debug"), Some(&"true".to_string()));
    }

    #[test]
    fn test_parse_variables_invalid() {
        let vars = vec!["invalid_format".to_string()];
        let result = Args::parse_variables(&vars);
        assert!(result.is_err());
    }
}
