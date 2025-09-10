// ABOUTME: S3 task implementation for AWS S3 operations
// ABOUTME: Handles file upload, download, and S3 bucket operations

use async_trait::async_trait;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::{primitives::ByteStream, Client};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;
use tracing::{error, info};

use super::TaskImplementation;
use crate::engine::error::{ExecutionError, Result};
use crate::engine::{ExecutionContext, TaskResult, TaskStatus};

pub struct S3Task;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Config {
    pub operation: S3Operation,
    pub bucket: String,
    pub key: String,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub local_path: Option<String>,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub acl: Option<String>,
    #[serde(default)]
    pub storage_class: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum S3Operation {
    Upload,
    Download,
    Delete,
    List,
}

#[async_trait]
impl TaskImplementation for S3Task {
    async fn execute(
        &self,
        task_id: String,
        config: serde_yaml::Value,
        _context: ExecutionContext,
    ) -> Result<TaskResult> {
        let start_time = chrono::Utc::now();

        let config: S3Config =
            serde_yaml::from_value(config).map_err(|e| ExecutionError::ConfigError {
                task_id: task_id.clone(),
                message: format!("Invalid S3 configuration: {}", e),
            })?;

        info!(
            "Executing S3 task: {} - {:?} on s3://{}/{}",
            task_id, config.operation, config.bucket, config.key
        );

        let mut task_result = TaskResult::new(task_id.clone(), "s3".to_string());
        task_result.start_time = start_time;
        task_result.mark_started();

        // Create S3 client
        let client = match self.create_s3_client(&config).await {
            Ok(client) => client,
            Err(e) => {
                let error_msg = format!("Failed to create S3 client: {}", e);
                error!("{}", error_msg);
                task_result.mark_completed(TaskStatus::Failed, None, Some(error_msg));
                return Ok(task_result);
            }
        };

        // Execute the operation
        let result = match config.operation {
            S3Operation::Upload => self.upload_file(&client, &config).await,
            S3Operation::Download => self.download_file(&client, &config).await,
            S3Operation::Delete => self.delete_object(&client, &config).await,
            S3Operation::List => self.list_objects(&client, &config).await,
        };

        match result {
            Ok(output) => {
                info!("S3 operation completed successfully: {}", output);
                task_result.mark_completed(TaskStatus::Success, Some(output), None);
            }
            Err(e) => {
                let error_msg = format!("S3 operation failed: {}", e);
                error!("{}", error_msg);
                task_result.mark_completed(TaskStatus::Failed, None, Some(error_msg));
            }
        }

        Ok(task_result)
    }

    fn task_type(&self) -> &'static str {
        "s3"
    }

    fn validate_config(&self, config: &serde_yaml::Value) -> Result<()> {
        let config: S3Config =
            serde_yaml::from_value(config.clone()).map_err(|e| ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: format!("Invalid S3 configuration: {}", e),
            })?;

        if config.bucket.is_empty() {
            return Err(ExecutionError::ConfigError {
                task_id: "validation".to_string(),
                message: "S3 bucket cannot be empty".to_string(),
            });
        }

        match config.operation {
            S3Operation::Upload => {
                if config.key.is_empty() {
                    return Err(ExecutionError::ConfigError {
                        task_id: "validation".to_string(),
                        message: "S3 key cannot be empty for upload operation".to_string(),
                    });
                }
                if config.local_path.is_none() {
                    return Err(ExecutionError::ConfigError {
                        task_id: "validation".to_string(),
                        message: "Local path is required for upload operation".to_string(),
                    });
                }
            }
            S3Operation::Download => {
                if config.key.is_empty() {
                    return Err(ExecutionError::ConfigError {
                        task_id: "validation".to_string(),
                        message: "S3 key cannot be empty for download operation".to_string(),
                    });
                }
                if config.local_path.is_none() {
                    return Err(ExecutionError::ConfigError {
                        task_id: "validation".to_string(),
                        message: "Local path is required for download operation".to_string(),
                    });
                }
            }
            S3Operation::Delete => {
                if config.key.is_empty() {
                    return Err(ExecutionError::ConfigError {
                        task_id: "validation".to_string(),
                        message: "S3 key cannot be empty for delete operation".to_string(),
                    });
                }
            }
            S3Operation::List => {
                // For list operations, we use either key as prefix or explicit prefix
                if config.key.is_empty() && config.prefix.is_none() {
                    return Err(ExecutionError::ConfigError {
                        task_id: "validation".to_string(),
                        message: "Either key or prefix is required for list operation".to_string(),
                    });
                }
            }
        }

        Ok(())
    }
}

impl S3Task {
    async fn create_s3_client(&self, config: &S3Config) -> Result<Client> {
        let region_provider = if let Some(region_str) = &config.region {
            RegionProviderChain::first_try(Some(Region::new(region_str.clone())))
        } else {
            RegionProviderChain::default_provider()
        };

        let shared_config = aws_config::from_env().region(region_provider).load().await;

        Ok(Client::new(&shared_config))
    }

    async fn upload_file(
        &self,
        client: &Client,
        config: &S3Config,
    ) -> std::result::Result<String, String> {
        let local_path = config
            .local_path
            .as_ref()
            .ok_or("Local path is required for upload")?;
        let file_path = Path::new(local_path);

        if !file_path.exists() {
            return Err(format!("Local file does not exist: {}", local_path));
        }

        // Read the file content
        let file_content = fs::read(file_path)
            .await
            .map_err(|e| format!("Failed to read file {}: {}", local_path, e))?;

        let body = ByteStream::from(file_content);

        // Build the put_object request
        let mut put_object_request = client
            .put_object()
            .bucket(&config.bucket)
            .key(&config.key)
            .body(body);

        // Add content type if specified
        if let Some(content_type) = &config.content_type {
            put_object_request = put_object_request.content_type(content_type);
        }

        // Add ACL if specified
        if let Some(acl) = &config.acl {
            put_object_request =
                put_object_request.acl(acl.parse().map_err(|e| format!("Invalid ACL: {}", e))?);
        }

        // Add storage class if specified
        if let Some(storage_class) = &config.storage_class {
            put_object_request = put_object_request.storage_class(
                storage_class
                    .parse()
                    .map_err(|e| format!("Invalid storage class: {}", e))?,
            );
        }

        // Add metadata
        for (key, value) in &config.metadata {
            put_object_request = put_object_request.metadata(key, value);
        }

        // Execute the upload
        let _response = put_object_request
            .send()
            .await
            .map_err(|e| format!("Failed to upload file to S3: {}", e))?;

        let file_size = fs::metadata(file_path).await.map(|m| m.len()).unwrap_or(0);

        Ok(format!(
            "Successfully uploaded {} ({} bytes) to s3://{}/{}",
            local_path, file_size, config.bucket, config.key
        ))
    }

    async fn download_file(
        &self,
        client: &Client,
        config: &S3Config,
    ) -> std::result::Result<String, String> {
        let local_path = config
            .local_path
            .as_ref()
            .ok_or("Local path is required for download")?;

        // Create parent directories if they don't exist
        if let Some(parent) = Path::new(local_path).parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("Failed to create directories for {}: {}", local_path, e))?;
        }

        // Download the object
        let response = client
            .get_object()
            .bucket(&config.bucket)
            .key(&config.key)
            .send()
            .await
            .map_err(|e| format!("Failed to download object from S3: {}", e))?;

        // Get the content length
        let content_length = response.content_length();

        // Stream the data to file
        let data = response
            .body
            .collect()
            .await
            .map_err(|e| format!("Failed to read S3 object data: {}", e))?;

        fs::write(local_path, data.into_bytes())
            .await
            .map_err(|e| format!("Failed to write file {}: {}", local_path, e))?;

        Ok(format!(
            "Successfully downloaded s3://{}/{} to {} ({} bytes)",
            config.bucket, config.key, local_path, content_length
        ))
    }

    async fn delete_object(
        &self,
        client: &Client,
        config: &S3Config,
    ) -> std::result::Result<String, String> {
        let _response = client
            .delete_object()
            .bucket(&config.bucket)
            .key(&config.key)
            .send()
            .await
            .map_err(|e| format!("Failed to delete S3 object: {}", e))?;

        Ok(format!(
            "Successfully deleted s3://{}/{}",
            config.bucket, config.key
        ))
    }

    async fn list_objects(
        &self,
        client: &Client,
        config: &S3Config,
    ) -> std::result::Result<String, String> {
        let prefix = config.prefix.as_deref().or(Some(&config.key));

        let mut list_request = client.list_objects_v2().bucket(&config.bucket);

        if let Some(prefix_value) = prefix {
            list_request = list_request.prefix(prefix_value);
        }

        let response = list_request
            .send()
            .await
            .map_err(|e| format!("Failed to list S3 objects: {}", e))?;

        let objects = response.contents().unwrap_or_default();
        let mut output = format!(
            "Found {} objects in s3://{}:\n",
            objects.len(),
            config.bucket
        );

        for object in objects {
            let key = object.key().unwrap_or("unknown");
            let size = object.size();
            let last_modified = object
                .last_modified()
                .map(|dt| {
                    dt.fmt(aws_sdk_s3::primitives::DateTimeFormat::DateTime)
                        .unwrap_or_else(|_| "unknown".to_string())
                })
                .unwrap_or_else(|| "unknown".to_string());

            output.push_str(&format!(
                "  {} ({} bytes, modified: {})\n",
                key, size, last_modified
            ));
        }

        Ok(output.trim_end().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;

    #[test]
    fn test_s3_config_validation() {
        let task = S3Task;

        // Test valid upload configuration
        let upload_config = serde_yaml::to_value(S3Config {
            operation: S3Operation::Upload,
            bucket: "test-bucket".to_string(),
            key: "test/file.txt".to_string(),
            local_path: Some("/local/file.txt".to_string()),
            region: None,
            content_type: None,
            metadata: HashMap::new(),
            prefix: None,
            acl: None,
            storage_class: None,
        })
        .unwrap();

        assert!(task.validate_config(&upload_config).is_ok());

        // Test invalid upload configuration (missing local_path)
        let invalid_upload_config = serde_yaml::to_value(S3Config {
            operation: S3Operation::Upload,
            bucket: "test-bucket".to_string(),
            key: "test/file.txt".to_string(),
            local_path: None,
            region: None,
            content_type: None,
            metadata: HashMap::new(),
            prefix: None,
            acl: None,
            storage_class: None,
        })
        .unwrap();

        assert!(task.validate_config(&invalid_upload_config).is_err());

        // Test valid download configuration
        let download_config = serde_yaml::to_value(S3Config {
            operation: S3Operation::Download,
            bucket: "test-bucket".to_string(),
            key: "test/file.txt".to_string(),
            local_path: Some("/local/download.txt".to_string()),
            region: Some("us-west-2".to_string()),
            content_type: None,
            metadata: HashMap::new(),
            prefix: None,
            acl: None,
            storage_class: None,
        })
        .unwrap();

        assert!(task.validate_config(&download_config).is_ok());

        // Test valid delete configuration
        let delete_config = serde_yaml::to_value(S3Config {
            operation: S3Operation::Delete,
            bucket: "test-bucket".to_string(),
            key: "test/file.txt".to_string(),
            local_path: None,
            region: None,
            content_type: None,
            metadata: HashMap::new(),
            prefix: None,
            acl: None,
            storage_class: None,
        })
        .unwrap();

        assert!(task.validate_config(&delete_config).is_ok());

        // Test valid list configuration with key as prefix
        let list_config = serde_yaml::to_value(S3Config {
            operation: S3Operation::List,
            bucket: "test-bucket".to_string(),
            key: "test/".to_string(),
            local_path: None,
            region: None,
            content_type: None,
            metadata: HashMap::new(),
            prefix: None,
            acl: None,
            storage_class: None,
        })
        .unwrap();

        assert!(task.validate_config(&list_config).is_ok());

        // Test valid list configuration with explicit prefix
        let list_config_with_prefix = serde_yaml::to_value(S3Config {
            operation: S3Operation::List,
            bucket: "test-bucket".to_string(),
            key: "".to_string(),
            local_path: None,
            region: None,
            content_type: None,
            metadata: HashMap::new(),
            prefix: Some("documents/".to_string()),
            acl: None,
            storage_class: None,
        })
        .unwrap();

        assert!(task.validate_config(&list_config_with_prefix).is_ok());

        // Test invalid list configuration (no key and no prefix)
        let invalid_list_config = serde_yaml::to_value(S3Config {
            operation: S3Operation::List,
            bucket: "test-bucket".to_string(),
            key: "".to_string(),
            local_path: None,
            region: None,
            content_type: None,
            metadata: HashMap::new(),
            prefix: None,
            acl: None,
            storage_class: None,
        })
        .unwrap();

        assert!(task.validate_config(&invalid_list_config).is_err());

        // Test invalid configuration (empty bucket)
        let empty_bucket_config = serde_yaml::to_value(S3Config {
            operation: S3Operation::Delete,
            bucket: "".to_string(),
            key: "test/file.txt".to_string(),
            local_path: None,
            region: None,
            content_type: None,
            metadata: HashMap::new(),
            prefix: None,
            acl: None,
            storage_class: None,
        })
        .unwrap();

        assert!(task.validate_config(&empty_bucket_config).is_err());
    }

    #[test]
    fn test_s3_config_serialization() {
        // Test configuration with all options
        let config = S3Config {
            operation: S3Operation::Upload,
            bucket: "my-bucket".to_string(),
            key: "uploads/document.pdf".to_string(),
            local_path: Some("/tmp/document.pdf".to_string()),
            region: Some("eu-west-1".to_string()),
            content_type: Some("application/pdf".to_string()),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("author".to_string(), "test".to_string());
                meta.insert("version".to_string(), "1.0".to_string());
                meta
            },
            prefix: Some("uploads/".to_string()),
            acl: Some("private".to_string()),
            storage_class: Some("STANDARD_IA".to_string()),
        };

        // Test serialization to YAML
        let yaml_value = serde_yaml::to_value(&config).unwrap();
        assert!(yaml_value.get("operation").is_some());
        assert!(yaml_value.get("bucket").is_some());
        assert!(yaml_value.get("key").is_some());
        assert!(yaml_value.get("local_path").is_some());
        assert!(yaml_value.get("region").is_some());
        assert!(yaml_value.get("content_type").is_some());
        assert!(yaml_value.get("metadata").is_some());
        assert!(yaml_value.get("prefix").is_some());
        assert!(yaml_value.get("acl").is_some());
        assert!(yaml_value.get("storage_class").is_some());

        // Test deserialization from YAML
        let deserialized: S3Config = serde_yaml::from_value(yaml_value).unwrap();
        assert_eq!(deserialized.bucket, config.bucket);
        assert_eq!(deserialized.key, config.key);
        assert_eq!(deserialized.local_path, config.local_path);
        assert_eq!(deserialized.region, config.region);
        assert_eq!(deserialized.content_type, config.content_type);
        assert_eq!(deserialized.metadata, config.metadata);
        assert_eq!(deserialized.prefix, config.prefix);
        assert_eq!(deserialized.acl, config.acl);
        assert_eq!(deserialized.storage_class, config.storage_class);
    }

    #[test]
    fn test_s3_task_type() {
        let task = S3Task;
        assert_eq!(task.task_type(), "s3");
    }

    #[test]
    fn test_s3_operation_serialization() {
        // Test that operations serialize to lowercase
        assert_eq!(
            serde_yaml::to_string(&S3Operation::Upload).unwrap().trim(),
            "upload"
        );
        assert_eq!(
            serde_yaml::to_string(&S3Operation::Download)
                .unwrap()
                .trim(),
            "download"
        );
        assert_eq!(
            serde_yaml::to_string(&S3Operation::Delete).unwrap().trim(),
            "delete"
        );
        assert_eq!(
            serde_yaml::to_string(&S3Operation::List).unwrap().trim(),
            "list"
        );
    }
}
