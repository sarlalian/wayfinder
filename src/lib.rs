// ABOUTME: Main library module for the wayfinder workflow engine
// ABOUTME: Exports all core modules and provides the public API

pub mod cli;
pub mod engine;
pub mod output;
pub mod parser;
pub mod reporting;
pub mod tasks;
pub mod template;

// Re-export commonly used types
pub use cli::{App, Args, Config};
pub use engine::{TaskStatus, WorkflowEngine, WorkflowStatus};
pub use output::{OutputHandler, OutputProcessor};
pub use parser::{TaskConfig, Workflow, WorkflowParser, WorkflowValidator};
pub use reporting::ReportingEngine;

// Error handling
pub type Result<T> = anyhow::Result<T>;

// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
