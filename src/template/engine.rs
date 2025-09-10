// ABOUTME: Main template engine implementation using Handlebars
// ABOUTME: Provides template rendering, workflow resolution, and variable substitution

use handlebars::Handlebars;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

use super::context::TemplateContext;
use super::error::{Result, TemplateError};
use super::helpers;
use crate::parser::{TaskConfig, Workflow};

#[derive(Clone)]
pub struct TemplateEngine {
    handlebars: Handlebars<'static>,
}

impl TemplateEngine {
    /// Create a new template engine with all built-in helpers
    pub fn new() -> Result<Self> {
        let mut handlebars = Handlebars::new();

        // Configure handlebars
        handlebars.set_strict_mode(false);
        handlebars.set_dev_mode(false);

        // Disable HTML escaping since we're generating shell scripts, not HTML
        handlebars.register_escape_fn(handlebars::no_escape);

        // Register built-in helpers
        helpers::register_helpers(&mut handlebars)
            .map_err(|e| TemplateError::SystemError(e.to_string()))?;

        Ok(Self { handlebars })
    }

    /// Render a template string with the given context
    pub fn render_template(&self, template: &str, context: &JsonValue) -> Result<String> {
        self.handlebars
            .render_template(template, context)
            .map_err(TemplateError::HandlebarsError)
    }

    /// Render a template string with the given context
    pub fn render(&self, template: &str, context: &TemplateContext) -> Result<String> {
        let json_context = context.to_json()?;
        self.handlebars
            .render_template(template, &json_context)
            .map_err(TemplateError::HandlebarsError)
    }

    /// Render a template string with JSON context
    pub fn render_with_json(&self, template: &str, context: &JsonValue) -> Result<String> {
        self.handlebars
            .render_template(template, context)
            .map_err(TemplateError::HandlebarsError)
    }

    /// Resolve all templates in a workflow
    pub fn resolve_workflow(
        &self,
        workflow: &mut Workflow,
        variables: &HashMap<String, String>,
    ) -> Result<()> {
        // Create context for this workflow
        let context = TemplateContext::for_workflow(variables, &workflow.name)?;
        let json_context = context.to_json()?;

        // Resolve workflow-level variables
        let mut resolved_variables = HashMap::new();
        for (key, value) in &workflow.variables {
            let resolved_value = self
                .handlebars
                .render_template(value, &json_context)
                .map_err(TemplateError::HandlebarsError)?;
            resolved_variables.insert(key.clone(), resolved_value);
        }
        workflow.variables = resolved_variables;

        // Update context with resolved variables
        let mut updated_context = context;
        updated_context.extend_variables(workflow.variables.clone());
        let updated_json_context = updated_context.to_json()?;

        // Resolve task configurations
        for (_task_id, task_config) in &mut workflow.tasks {
            self.resolve_task_config(task_config, &updated_json_context)?;

            // Resolve conditional execution
            if let Some(ref when_condition) = task_config.when {
                let resolved_condition = self
                    .handlebars
                    .render_template(when_condition, &updated_json_context)
                    .map_err(TemplateError::HandlebarsError)?;
                task_config.when = Some(resolved_condition);
            }
        }

        // Resolve output configuration
        let resolved_destination = self
            .handlebars
            .render_template(&workflow.output.destination, &updated_json_context)
            .map_err(TemplateError::HandlebarsError)?;
        workflow.output.destination = resolved_destination;

        Ok(())
    }

    /// Resolve templates in a single task configuration
    fn resolve_task_config(&self, task_config: &mut TaskConfig, context: &JsonValue) -> Result<()> {
        // Convert serde_yaml::Value to serde_json::Value for processing
        let json_config: JsonValue =
            serde_json::to_value(&task_config.config).map_err(TemplateError::JsonError)?;

        let resolved_config = self.resolve_json_templates(&json_config, context)?;

        // Convert back to serde_yaml::Value
        task_config.config = serde_yaml::to_value(&resolved_config)
            .map_err(|e| TemplateError::SystemError(format!("YAML conversion error: {}", e)))?;

        Ok(())
    }

    /// Recursively resolve templates in JSON values
    pub fn resolve_json_templates(&self, value: &JsonValue, context: &JsonValue) -> Result<JsonValue> {
        match value {
            JsonValue::String(s) => {
                let resolved = self
                    .handlebars
                    .render_template(s, context)
                    .map_err(TemplateError::HandlebarsError)?;
                Ok(JsonValue::String(resolved))
            }
            JsonValue::Array(arr) => {
                let resolved_array: Result<Vec<JsonValue>> = arr
                    .iter()
                    .map(|v| self.resolve_json_templates(v, context))
                    .collect();
                Ok(JsonValue::Array(resolved_array?))
            }
            JsonValue::Object(obj) => {
                let mut resolved_obj = serde_json::Map::new();
                for (key, val) in obj {
                    let resolved_key = if key.contains("{{") {
                        self.handlebars
                            .render_template(key, context)
                            .map_err(TemplateError::HandlebarsError)?
                    } else {
                        key.clone()
                    };
                    let resolved_val = self.resolve_json_templates(val, context)?;
                    resolved_obj.insert(resolved_key, resolved_val);
                }
                Ok(JsonValue::Object(resolved_obj))
            }
            // Numbers, booleans, and null values don't need template resolution
            other => Ok(other.clone()),
        }
    }

    /// Validate template syntax without rendering
    pub fn validate_template(&self, template: &str) -> Result<()> {
        // Try to compile the template to check for syntax errors
        match handlebars::Template::compile(template) {
            Ok(_) => Ok(()),
            Err(e) => Err(TemplateError::SyntaxError(e.to_string())),
        }
    }

    /// Check if a string contains template expressions
    pub fn has_templates(&self, text: &str) -> bool {
        text.contains("{{") && text.contains("}}")
    }

    /// Register a custom helper function
    pub fn register_helper<F>(&mut self, name: &str, helper: F) -> Result<()>
    where
        F: handlebars::HelperDef + Send + Sync + 'static,
    {
        self.handlebars.register_helper(name, Box::new(helper));
        Ok(())
    }

    /// Create a template engine with custom helpers
    pub fn with_custom_helpers<F>(mut self, register_fn: F) -> Result<Self>
    where
        F: FnOnce(&mut Handlebars) -> Result<()>,
    {
        register_fn(&mut self.handlebars)?;
        Ok(self)
    }
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new().expect("Failed to create default template engine")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_basic_template_rendering() {
        let engine = TemplateEngine::new().unwrap();
        let mut variables = HashMap::new();
        variables.insert("name".to_string(), "World".to_string());

        let context = TemplateContext::new(&variables).unwrap();
        let result = engine
            .render("Hello {{variables.name}}!", &context)
            .unwrap();

        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_system_info_templates() {
        let engine = TemplateEngine::new().unwrap();
        let variables = HashMap::new();
        let context = TemplateContext::new(&variables).unwrap();

        let result = engine
            .render("Host: {{system.hostname}}, OS: {{system.os}}", &context)
            .unwrap();
        assert!(result.starts_with("Host: "));
        assert!(result.contains("OS: "));
    }

    #[test]
    fn test_helper_functions() {
        let engine = TemplateEngine::new().unwrap();
        let variables = HashMap::new();
        let context = TemplateContext::new(&variables).unwrap();

        // Test timestamp helper
        let result = engine.render("{{timestamp}}", &context).unwrap();
        assert!(!result.is_empty());

        // Test UUID helper
        let result = engine.render("{{uuid}}", &context).unwrap();
        assert_eq!(result.len(), 36);

        // Test hostname helper
        let result = engine.render("{{hostname}}", &context).unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_workflow_resolution() {
        let engine = TemplateEngine::new().unwrap();

        // Create a test workflow with templates
        let workflow_yaml = r#"
name: template-test
description: Testing template resolution

variables:
  environment: "{{env \"ENVIRONMENT\" \"development\"}}"
  timestamp: "{{timestamp \"%Y-%m-%d\"}}"

tasks:
  hello:
    type: command
    config:
      command: echo
      args: 
        - "Hello from {{variables.environment}} at {{variables.timestamp}}"
    required: true

output:
  destination: "file://./output/{{workflow.name}}-{{variables.timestamp}}.json"
"#;

        let mut workflow = crate::parser::Workflow::from_yaml(workflow_yaml).unwrap();
        let mut variables = HashMap::new();
        variables.insert("custom_var".to_string(), "custom_value".to_string());

        // This should resolve all templates in the workflow
        let result = engine.resolve_workflow(&mut workflow, &variables);
        assert!(result.is_ok());

        // Check that variables were resolved
        assert!(workflow.variables.contains_key("environment"));
        assert!(workflow.variables.contains_key("timestamp"));

        // Check that output destination was resolved
        assert!(workflow
            .output
            .destination
            .starts_with("file://./output/template-test-"));
        assert!(workflow.output.destination.ends_with(".json"));
    }

    #[test]
    fn test_json_template_resolution() {
        let engine = TemplateEngine::new().unwrap();
        let context = json!({
            "name": "test",
            "value": 42
        });

        let input = json!({
            "message": "Hello {{name}}",
            "number": "{{value}}",
            "nested": {
                "greeting": "Hi {{name}}!"
            }
        });

        let resolved = engine.resolve_json_templates(&input, &context).unwrap();

        assert_eq!(resolved["message"], "Hello test");
        assert_eq!(resolved["number"], "42");
        assert_eq!(resolved["nested"]["greeting"], "Hi test!");
    }

    #[test]
    fn test_template_validation() {
        let engine = TemplateEngine::new().unwrap();

        // Valid template
        assert!(engine.validate_template("Hello {{name}}").is_ok());

        // Invalid template (unmatched braces)
        assert!(engine.validate_template("Hello {{name}").is_err());

        // Valid complex template
        assert!(engine
            .validate_template("{{#if condition}}true{{else}}false{{/if}}")
            .is_ok());
    }

    #[test]
    fn test_has_templates() {
        let engine = TemplateEngine::new().unwrap();

        assert!(engine.has_templates("Hello {{name}}"));
        assert!(engine.has_templates("{{timestamp}}"));
        assert!(!engine.has_templates("Hello world"));
        assert!(!engine.has_templates("No templates here"));
    }

    #[test]
    fn test_custom_helper() {
        let mut engine = TemplateEngine::new().unwrap();

        // Register a custom helper
        engine
            .register_helper(
                "multiply",
                |h: &handlebars::Helper,
                 _: &Handlebars,
                 _: &handlebars::Context,
                 _: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output| {
                    let a = h.param(0).and_then(|v| v.value().as_u64()).ok_or_else(|| {
                        handlebars::RenderError::new("First parameter must be a number")
                    })?;

                    let b = h.param(1).and_then(|v| v.value().as_u64()).ok_or_else(|| {
                        handlebars::RenderError::new("Second parameter must be a number")
                    })?;

                    out.write(&(a * b).to_string())?;
                    Ok(())
                },
            )
            .unwrap();

        let context = json!({});
        let result = engine
            .render_with_json("{{multiply 6 7}}", &context)
            .unwrap();
        assert_eq!(result, "42");
    }
}
