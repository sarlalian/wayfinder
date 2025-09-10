// ABOUTME: Main application orchestration for wayfinder CLI
// ABOUTME: Coordinates between CLI arguments, configuration, and command execution

use anyhow::Result;
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;

use super::commands;
use super::{Args, Commands, Config};

pub struct App {
    config: Config,
}

impl App {
    /// Create a new application instance
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Initialize logging based on configuration
    pub fn init_logging(&self, verbose: bool, no_color: bool) -> Result<()> {
        let log_level = if verbose {
            "debug"
        } else {
            &self.config.logging.level
        };

        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

        match self.config.logging.format.as_str() {
            "compact" => {
                tracing_subscriber::fmt()
                    .compact()
                    .with_env_filter(env_filter)
                    .with_ansi(!no_color)
                    .with_target(false)
                    .init();
            }
            _ => {
                tracing_subscriber::fmt()
                    .with_env_filter(env_filter)
                    .with_ansi(!no_color)
                    .with_target(false)
                    .init();
            }
        }

        debug!("Logging initialized with level: {}", log_level);
        Ok(())
    }

    /// Run the application with parsed arguments
    pub async fn run(&mut self, args: Args) -> Result<()> {
        // Initialize logging
        self.init_logging(args.verbose, args.no_color)?;

        info!("Starting wayfinder v{}", env!("CARGO_PKG_VERSION"));
        debug!("Configuration loaded from: {:?}", args.config);

        // Merge any command-specific variables into config
        match &args.command {
            Commands::Run { vars, .. } | Commands::Validate { vars, .. } => {
                let variables = Args::parse_variables(vars)?;
                self.config.merge_variables(variables);
            }
            _ => {}
        }

        // Execute the appropriate command
        match args.command {
            Commands::Run {
                workflow,
                vars,
                dry_run,
                output,
                max_concurrent,
            } => {
                let max_concurrent = max_concurrent.unwrap_or(self.config.max_concurrent_tasks);
                commands::run_workflow(
                    workflow,
                    vars,
                    dry_run,
                    output,
                    Some(max_concurrent),
                    &self.config,
                )
                .await
            }

            Commands::Validate { workflow, vars } => {
                commands::validate_workflow(workflow, vars, &self.config).await
            }

            Commands::Init {
                name,
                output_dir,
                template,
            } => commands::init_workflow(name, output_dir, template, &self.config).await,

            Commands::Report { config } => commands::generate_report(config, &self.config).await,
        }
    }

    /// Create application from command line arguments
    pub async fn from_args() -> Result<Self> {
        let args = Args::parse_args();
        let config = Config::load(args.config.clone())?;
        Ok(Self::new(config))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_app_creation() {
        let config = Config::default();
        let app = App::new(config);
        assert_eq!(app.config.max_concurrent_tasks, 4);
    }

    #[tokio::test]
    async fn test_app_from_args_with_config_file() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("wayfinder.yaml");

        let config_content = r#"
max_concurrent_tasks: 8
logging:
  level: debug
  format: json
"#;

        fs::write(&config_path, config_content).unwrap();

        // Note: This test would need to mock command line args
        // For now, we'll just test config loading directly
        let config = Config::load(Some(config_path)).unwrap();
        assert_eq!(config.max_concurrent_tasks, 8);
        assert_eq!(config.logging.level, "debug");
        assert_eq!(config.logging.format, "json");
    }
}
