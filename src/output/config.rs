// ABOUTME: Configuration types for output handling
// ABOUTME: Defines structures for configuring output formatting and destinations

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default)]
    pub destinations: Vec<OutputDestination>,
    #[serde(default)]
    pub options: OutputOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputDestination {
    pub writer_type: String,
    #[serde(default)]
    pub config: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OutputOptions {
    #[serde(default)]
    pub include_metadata: bool,
    #[serde(default)]
    pub include_timestamps: bool,
    #[serde(default)]
    pub include_duration: bool,
    #[serde(default = "default_true")]
    pub include_task_results: bool,
    #[serde(default)]
    pub include_system_info: bool,
    #[serde(default)]
    pub filter_successful: bool,
    #[serde(default)]
    pub filter_failed: bool,
    #[serde(default)]
    pub max_output_length: Option<usize>,
    #[serde(default)]
    pub pretty_print: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWriterConfig {
    pub path: String,
    #[serde(default = "default_true")]
    pub create_dirs: bool,
    #[serde(default)]
    pub append: bool,
    #[serde(default)]
    pub backup_existing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3WriterConfig {
    pub bucket: String,
    pub key: String,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StdoutWriterConfig {
    #[serde(default = "default_true")]
    pub colored: bool,
    #[serde(default)]
    pub quiet: bool,
}

fn default_format() -> String {
    "json".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            format: default_format(),
            destinations: vec![OutputDestination {
                writer_type: "stdout".to_string(),
                config: HashMap::new(),
            }],
            options: OutputOptions::default(),
        }
    }
}

impl OutputDestination {
    pub fn new_stdout() -> Self {
        Self {
            writer_type: "stdout".to_string(),
            config: HashMap::new(),
        }
    }

    pub fn new_file<S: Into<String>>(path: S) -> Self {
        let mut config = HashMap::new();
        config.insert("path".to_string(), serde_yaml::Value::String(path.into()));

        Self {
            writer_type: "file".to_string(),
            config,
        }
    }

    pub fn new_s3<S: Into<String>>(bucket: S, key: S) -> Self {
        let mut config = HashMap::new();
        config.insert(
            "bucket".to_string(),
            serde_yaml::Value::String(bucket.into()),
        );
        config.insert("key".to_string(), serde_yaml::Value::String(key.into()));

        Self {
            writer_type: "s3".to_string(),
            config,
        }
    }

    pub fn get_config<T>(&self) -> Result<T, crate::output::error::OutputError>
    where
        T: serde::de::DeserializeOwned,
    {
        let config_value = serde_yaml::Value::Mapping(
            self.config
                .iter()
                .map(|(k, v)| (serde_yaml::Value::String(k.clone()), v.clone()))
                .collect(),
        );

        serde_yaml::from_value(config_value).map_err(|e| {
            crate::output::error::OutputError::ConfigError {
                message: format!("Failed to parse destination config: {}", e),
            }
        })
    }
}

impl OutputOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn include_all(mut self) -> Self {
        self.include_metadata = true;
        self.include_timestamps = true;
        self.include_duration = true;
        self.include_task_results = true;
        self.include_system_info = true;
        self
    }

    pub fn minimal(mut self) -> Self {
        self.include_metadata = false;
        self.include_timestamps = false;
        self.include_duration = false;
        self.include_system_info = false;
        self
    }

    pub fn pretty(mut self) -> Self {
        self.pretty_print = true;
        self
    }

    pub fn filter_status(mut self, successful: bool, failed: bool) -> Self {
        self.filter_successful = successful;
        self.filter_failed = failed;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_config_default() {
        let config = OutputConfig::default();
        assert_eq!(config.format, "json");
        assert_eq!(config.destinations.len(), 1);
        assert_eq!(config.destinations[0].writer_type, "stdout");
    }

    #[test]
    fn test_output_destination_constructors() {
        let stdout_dest = OutputDestination::new_stdout();
        assert_eq!(stdout_dest.writer_type, "stdout");

        let file_dest = OutputDestination::new_file("/tmp/output.json");
        assert_eq!(file_dest.writer_type, "file");
        assert!(file_dest.config.contains_key("path"));

        let s3_dest = OutputDestination::new_s3("my-bucket", "results/output.json");
        assert_eq!(s3_dest.writer_type, "s3");
        assert!(s3_dest.config.contains_key("bucket"));
        assert!(s3_dest.config.contains_key("key"));
    }

    #[test]
    fn test_output_options_builder() {
        let options = OutputOptions::new()
            .include_all()
            .pretty()
            .filter_status(true, false);

        assert!(options.include_metadata);
        assert!(options.include_timestamps);
        assert!(options.pretty_print);
        assert!(options.filter_successful);
        assert!(!options.filter_failed);
    }
}
