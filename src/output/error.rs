// ABOUTME: Error types for output handling operations
// ABOUTME: Defines specific error types for formatting and writing workflow results

use thiserror::Error;

#[derive(Error, Debug)]
pub enum OutputError {
    #[error("Formatter not found: {format}")]
    FormatterNotFound { format: String },

    #[error("Writer not found: {writer_type}")]
    WriterNotFound { writer_type: String },

    #[error("Format error: {message}")]
    FormatError { message: String },

    #[error("Write error: {message}")]
    WriteError { message: String },

    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("YAML serialization error: {0}")]
    YamlSerializationError(#[from] serde_yaml::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Template error: {0}")]
    TemplateError(#[from] crate::template::TemplateError),

    #[error("System error: {message}")]
    SystemError { message: String },
}

pub type Result<T> = std::result::Result<T, OutputError>;
