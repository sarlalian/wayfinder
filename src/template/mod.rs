// ABOUTME: Template engine module for wayfinder workflow engine
// ABOUTME: Provides template processing and variable resolution functionality

pub mod context;
pub mod engine;
pub mod error;
pub mod helpers;

pub use context::{SystemInfo, TemplateContext};
pub use engine::TemplateEngine;
pub use error::{Result, TemplateError};
