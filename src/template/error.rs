// ABOUTME: Error types for template engine operations
// ABOUTME: Defines specific error types for template processing and rendering

use thiserror::Error;

#[derive(Error, Debug)]
pub enum TemplateError {
    #[error("Template render error: {0}")]
    RenderError(String),

    #[error("Template syntax error: {0}")]
    SyntaxError(String),

    #[error("Missing template variable: {0}")]
    MissingVariable(String),

    #[error("Invalid template function: {0}")]
    InvalidFunction(String),

    #[error("System information error: {0}")]
    SystemError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Environment variable error: {0}")]
    EnvError(#[from] std::env::VarError),

    #[error("Handlebars error: {0}")]
    HandlebarsError(#[from] handlebars::RenderError),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, TemplateError>;
