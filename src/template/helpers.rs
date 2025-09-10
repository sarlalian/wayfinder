// ABOUTME: Handlebars helper functions for template rendering
// ABOUTME: Implements built-in template functions for hostname, timestamps, environment variables, etc.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::{DateTime, TimeZone, Utc};
use handlebars::{Context, Handlebars, Helper, Output, RenderContext, RenderError};
use std::env;
use std::path::Path;
use uuid::Uuid;

/// Hostname helper - returns the system hostname
pub fn hostname_helper(
    _h: &Helper,
    _: &Handlebars,
    _: &Context,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> std::result::Result<(), RenderError> {
    let hostname_os = hostname::get().map_err(|_| RenderError::new("Failed to get hostname"))?;
    let hostname = hostname_os.to_string_lossy();
    out.write(&hostname)?;
    Ok(())
}

/// Timestamp helper - formats current time with optional format string
pub fn timestamp_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> std::result::Result<(), RenderError> {
    let format = h
        .param(0)
        .and_then(|v| v.value().as_str())
        .unwrap_or("%Y-%m-%d %H:%M:%S");

    let now = Utc::now();
    let formatted = now.format(format).to_string();
    out.write(&formatted)?;
    Ok(())
}

/// UUID helper - generates a new UUID v4
pub fn uuid_helper(
    _h: &Helper,
    _: &Handlebars,
    _: &Context,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> std::result::Result<(), RenderError> {
    let uuid = Uuid::new_v4().to_string();
    out.write(&uuid)?;
    Ok(())
}

/// Environment variable helper - gets environment variable value
pub fn env_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> std::result::Result<(), RenderError> {
    let var_name = h
        .param(0)
        .and_then(|v| v.value().as_str())
        .ok_or_else(|| RenderError::new("env helper requires variable name parameter"))?;

    let default_value = h.param(1).and_then(|v| v.value().as_str()).unwrap_or("");

    let value = env::var(var_name).unwrap_or_else(|_| default_value.to_string());
    out.write(&value)?;
    Ok(())
}

/// File exists helper - checks if file exists
pub fn file_exists_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> std::result::Result<(), RenderError> {
    let file_path = h
        .param(0)
        .and_then(|v| v.value().as_str())
        .ok_or_else(|| RenderError::new("file_exists helper requires file path parameter"))?;

    let exists = Path::new(file_path).exists();
    out.write(&exists.to_string())?;
    Ok(())
}

/// Base64 encode helper
pub fn base64_encode_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> std::result::Result<(), RenderError> {
    let input = h
        .param(0)
        .and_then(|v| v.value().as_str())
        .ok_or_else(|| RenderError::new("base64_encode helper requires input parameter"))?;

    let encoded = BASE64.encode(input.as_bytes());
    out.write(&encoded)?;
    Ok(())
}

/// Base64 decode helper
pub fn base64_decode_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> std::result::Result<(), RenderError> {
    let input = h
        .param(0)
        .and_then(|v| v.value().as_str())
        .ok_or_else(|| RenderError::new("base64_decode helper requires input parameter"))?;

    let decoded_bytes = BASE64
        .decode(input)
        .map_err(|e| RenderError::new(format!("Base64 decode error: {}", e)))?;

    let decoded_str = String::from_utf8(decoded_bytes)
        .map_err(|e| RenderError::new(format!("UTF-8 decode error: {}", e)))?;

    out.write(&decoded_str)?;
    Ok(())
}

/// Uppercase helper
pub fn upper_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> std::result::Result<(), RenderError> {
    let input = h
        .param(0)
        .and_then(|v| v.value().as_str())
        .ok_or_else(|| RenderError::new("upper helper requires input parameter"))?;

    out.write(&input.to_uppercase())?;
    Ok(())
}

/// Lowercase helper
pub fn lower_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> std::result::Result<(), RenderError> {
    let input = h
        .param(0)
        .and_then(|v| v.value().as_str())
        .ok_or_else(|| RenderError::new("lower helper requires input parameter"))?;

    out.write(&input.to_lowercase())?;
    Ok(())
}

/// Join helper - joins array elements with separator
pub fn join_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> std::result::Result<(), RenderError> {
    let separator = h.param(0).and_then(|v| v.value().as_str()).unwrap_or(",");

    // Get the array from the context (this would need to be passed as a parameter in real usage)
    let array = h
        .param(1)
        .and_then(|v| v.value().as_array())
        .ok_or_else(|| RenderError::new("join helper requires array parameter"))?;

    let strings: std::result::Result<Vec<String>, RenderError> = array
        .iter()
        .map(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| RenderError::new("Array elements must be strings"))
        })
        .collect();

    let joined = strings?.join(separator);
    out.write(&joined)?;
    Ok(())
}

/// Default helper - provides default value if variable is empty
pub fn default_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> std::result::Result<(), RenderError> {
    let value = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");

    let default_value = h
        .param(1)
        .and_then(|v| v.value().as_str())
        .ok_or_else(|| RenderError::new("default helper requires default value parameter"))?;

    let result = if value.is_empty() {
        default_value
    } else {
        value
    };

    out.write(result)?;
    Ok(())
}

/// Format time helper - formats a timestamp with custom format
pub fn format_time_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> std::result::Result<(), RenderError> {
    let timestamp_str = h
        .param(0)
        .and_then(|v| v.value().as_str())
        .ok_or_else(|| RenderError::new("format_time helper requires timestamp parameter"))?;

    let format = h
        .param(1)
        .and_then(|v| v.value().as_str())
        .unwrap_or("%Y-%m-%d %H:%M:%S");

    // Try to parse the timestamp (assuming RFC3339 format)
    let datetime = DateTime::parse_from_rfc3339(timestamp_str)
        .or_else(|_| {
            // Try parsing as Unix timestamp
            timestamp_str
                .parse::<i64>()
                .map(|ts| Utc.timestamp_opt(ts, 0).single().unwrap().into())
        })
        .map_err(|e| RenderError::new(format!("Failed to parse timestamp: {}", e)))?;

    let formatted = datetime.format(format).to_string();
    out.write(&formatted)?;
    Ok(())
}

/// Register all built-in helpers with a Handlebars instance
pub fn register_helpers(
    handlebars: &mut Handlebars,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    handlebars.register_helper("hostname", Box::new(hostname_helper));
    handlebars.register_helper("timestamp", Box::new(timestamp_helper));
    handlebars.register_helper("uuid", Box::new(uuid_helper));
    handlebars.register_helper("env", Box::new(env_helper));
    handlebars.register_helper("file_exists", Box::new(file_exists_helper));
    handlebars.register_helper("base64_encode", Box::new(base64_encode_helper));
    handlebars.register_helper("base64_decode", Box::new(base64_decode_helper));
    handlebars.register_helper("upper", Box::new(upper_helper));
    handlebars.register_helper("lower", Box::new(lower_helper));
    handlebars.register_helper("join", Box::new(join_helper));
    handlebars.register_helper("default", Box::new(default_helper));
    handlebars.register_helper("format_time", Box::new(format_time_helper));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use handlebars::Handlebars;

    fn create_test_handlebars() -> Handlebars<'static> {
        let mut handlebars = Handlebars::new();
        register_helpers(&mut handlebars).unwrap();
        handlebars
    }

    #[test]
    fn test_hostname_helper() {
        let handlebars = create_test_handlebars();
        let result = handlebars
            .render_template("{{hostname}}", &serde_json::json!({}))
            .unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_timestamp_helper() {
        let handlebars = create_test_handlebars();
        let result = handlebars
            .render_template("{{timestamp}}", &serde_json::json!({}))
            .unwrap();
        assert!(!result.is_empty());

        let result_formatted = handlebars
            .render_template("{{timestamp \"%Y\"}}", &serde_json::json!({}))
            .unwrap();
        assert_eq!(result_formatted.len(), 4); // Year should be 4 digits
    }

    #[test]
    fn test_uuid_helper() {
        let handlebars = create_test_handlebars();
        let result = handlebars
            .render_template("{{uuid}}", &serde_json::json!({}))
            .unwrap();
        assert_eq!(result.len(), 36); // UUID v4 length
        assert!(result.contains('-'));
    }

    #[test]
    fn test_env_helper() {
        std::env::set_var("TEST_VAR", "test_value");
        let handlebars = create_test_handlebars();
        let result = handlebars
            .render_template("{{env \"TEST_VAR\"}}", &serde_json::json!({}))
            .unwrap();
        assert_eq!(result, "test_value");

        let result_default = handlebars
            .render_template(
                "{{env \"NONEXISTENT_VAR\" \"default_value\"}}",
                &serde_json::json!({}),
            )
            .unwrap();
        assert_eq!(result_default, "default_value");
    }

    #[test]
    fn test_base64_helpers() {
        let handlebars = create_test_handlebars();
        let input = "hello world";
        let encoded = handlebars
            .render_template("{{base64_encode \"hello world\"}}", &serde_json::json!({}))
            .unwrap();
        assert_eq!(encoded, "aGVsbG8gd29ybGQ=");

        let template = format!("{{{{base64_decode \"{}\"}}}}", encoded);
        let decoded = handlebars
            .render_template(&template, &serde_json::json!({}))
            .unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn test_case_helpers() {
        let handlebars = create_test_handlebars();
        let upper = handlebars
            .render_template("{{upper \"hello world\"}}", &serde_json::json!({}))
            .unwrap();
        assert_eq!(upper, "HELLO WORLD");

        let lower = handlebars
            .render_template("{{lower \"HELLO WORLD\"}}", &serde_json::json!({}))
            .unwrap();
        assert_eq!(lower, "hello world");
    }

    #[test]
    fn test_default_helper() {
        let handlebars = create_test_handlebars();
        let result = handlebars
            .render_template("{{default \"\" \"fallback\"}}", &serde_json::json!({}))
            .unwrap();
        assert_eq!(result, "fallback");

        let result2 = handlebars
            .render_template("{{default \"value\" \"fallback\"}}", &serde_json::json!({}))
            .unwrap();
        assert_eq!(result2, "value");
    }
}
