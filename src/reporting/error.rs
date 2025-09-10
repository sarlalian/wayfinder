// ABOUTME: Error types for reporting engine operations
// ABOUTME: Defines specific error types for report generation and delivery

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReportingError {
    #[error("Report generator not found: {generator_type}")]
    GeneratorNotFound { generator_type: String },

    #[error("Report deliverer not found: {deliverer_type}")]
    DelivererNotFound { deliverer_type: String },

    #[error("Report generation failed: {message}")]
    GenerationError { message: String },

    #[error("Report delivery failed: {message}")]
    DeliveryError { message: String },

    #[error("Template error: {message}")]
    TemplateError { message: String },

    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    #[error("Metrics collection error: {message}")]
    MetricsError { message: String },

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("YAML serialization error: {0}")]
    YamlSerializationError(#[from] serde_yaml::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Template engine error: {0}")]
    TemplateEngineError(#[from] crate::template::TemplateError),

    #[error("Output error: {0}")]
    OutputError(#[from] crate::output::error::OutputError),

    #[error("HTTP request error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("System error: {message}")]
    SystemError { message: String },
}

pub type Result<T> = std::result::Result<T, ReportingError>;
