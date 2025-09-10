// ABOUTME: Integration tests for the reporting engine
// ABOUTME: Tests report generation, delivery, metrics collection, and template rendering

use std::collections::HashMap;
use tempfile::TempDir;
use tokio::fs;

use wayfinder::reporting::config::{DeliveryChannel, ReportTemplate, ReportingConfig};
use wayfinder::reporting::metrics::MetricsCollector;
use wayfinder::reporting::{ReportPeriod, ReportProcessor, ReportingEngine, WorkflowContext};

mod common;
use common::{create_complex_workflow_result, create_mock_workflow_result};

#[tokio::test]
async fn test_reporting_workflow_completion() {
    let reporting_engine = ReportingEngine::new();
    let workflow_result = create_mock_workflow_result("test_workflow", "run_123");

    let mut config = ReportingConfig {
        workflow_completion_reports: true,
        ..Default::default()
    };

    let mut template = ReportTemplate::new("completion_report".to_string(), "summary".to_string());
    template.trigger_on_workflow_completion = true;
    template.delivery_channels = vec![DeliveryChannel::new_terminal()];
    config.templates.push(template);

    let result = reporting_engine
        .process_workflow_completion(&workflow_result, &config)
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_reporting_task_failure() {
    let reporting_engine = ReportingEngine::new();

    // Create a failed task result
    let mut task_result =
        wayfinder::engine::TaskResult::new("failed_task".to_string(), "command".to_string());
    task_result.mark_completed(
        wayfinder::engine::TaskStatus::Failed,
        None,
        Some("Task execution failed".to_string()),
    );

    let context = WorkflowContext {
        workflow_name: "test_workflow".to_string(),
        run_id: "run_456".to_string(),
        start_time: chrono::Utc::now(),
        metadata: HashMap::new(),
    };

    let mut config = ReportingConfig {
        task_completion_reports: true,
        ..Default::default()
    };

    let mut template = ReportTemplate::new("failure_report".to_string(), "detailed".to_string());
    template.trigger_on_task_failure = true;
    template.delivery_channels = vec![DeliveryChannel::new_terminal()];
    config.templates.push(template);

    let result = reporting_engine
        .process_task_completion(&task_result, &context, &config)
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_reporting_file_delivery() {
    let temp_dir = TempDir::new().unwrap();
    let report_file = temp_dir.path().join("test_report.txt");

    let reporting_engine = ReportingEngine::new();
    let workflow_result = create_complex_workflow_result("complex_workflow", "run_789");

    let mut config = ReportingConfig {
        workflow_completion_reports: true,
        ..Default::default()
    };

    let mut template = ReportTemplate::new("file_report".to_string(), "detailed".to_string());
    template.trigger_on_workflow_completion = true;
    template.delivery_channels = vec![DeliveryChannel::new_file(
        report_file.to_string_lossy().to_string(),
    )];
    config.templates.push(template);

    let result = reporting_engine
        .process_workflow_completion(&workflow_result, &config)
        .await;
    assert!(result.is_ok());

    // Check that report file was created
    assert!(report_file.exists());
    let content = fs::read_to_string(&report_file).await.unwrap();
    assert!(content.contains("complex_workflow"));
    assert!(content.contains("Task Details"));
}

#[tokio::test]
async fn test_reporting_multiple_delivery_channels() {
    let temp_dir = TempDir::new().unwrap();
    let report_file = temp_dir.path().join("multi_channel_report.txt");

    let reporting_engine = ReportingEngine::new();
    let workflow_result = create_mock_workflow_result("multi_channel_test", "run_multi");

    let mut config = ReportingConfig {
        workflow_completion_reports: true,
        ..Default::default()
    };

    let mut template =
        ReportTemplate::new("multi_channel_report".to_string(), "summary".to_string());
    template.trigger_on_workflow_completion = true;
    template.delivery_channels = vec![
        DeliveryChannel::new_terminal(),
        DeliveryChannel::new_file(report_file.to_string_lossy().to_string()),
    ];
    config.templates.push(template);

    let result = reporting_engine
        .process_workflow_completion(&workflow_result, &config)
        .await;
    assert!(result.is_ok());

    // File delivery should have worked
    assert!(report_file.exists());
    let content = fs::read_to_string(&report_file).await.unwrap();
    assert!(content.contains("multi_channel_test"));
}

#[tokio::test]
async fn test_metrics_collection() {
    let collector = MetricsCollector::new();
    let workflow_result = create_complex_workflow_result("metrics_test", "run_metrics");

    // Record workflow completion
    collector.record_workflow_completion(&workflow_result).await;

    // Get execution summary
    let summary = collector.get_execution_summary().await;
    assert_eq!(summary.total_workflows, 1);
    assert_eq!(summary.successful_workflows, 1);
    assert!(summary.workflow_success_rate > 0.0);

    // Get workflow-specific metrics
    let workflow_metrics = collector.get_workflow_metrics("metrics_test", None).await;
    assert_eq!(workflow_metrics.len(), 1);
    assert_eq!(workflow_metrics[0].workflow_name, "metrics_test");
    assert_eq!(workflow_metrics[0].run_id, "run_metrics");
}

#[tokio::test]
async fn test_periodic_metrics_reporting() {
    let reporting_engine = ReportingEngine::new();

    // Add some metrics data first
    let workflow_result = create_complex_workflow_result("periodic_test", "run_periodic");
    reporting_engine
        .get_metrics()
        .record_workflow_completion(&workflow_result)
        .await;

    let mut config = ReportingConfig {
        periodic_reports: true,
        ..Default::default()
    };

    let mut template = ReportTemplate::new("hourly_metrics".to_string(), "metrics".to_string());
    template.periodic_schedule = Some(wayfinder::reporting::config::PeriodicSchedule::hourly());
    template.delivery_channels = vec![DeliveryChannel::new_terminal()];
    config.templates.push(template);

    let result = reporting_engine
        .generate_periodic_report(ReportPeriod::Hourly, &config)
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_report_filtering() {
    let reporting_engine = ReportingEngine::new();
    let workflow_result = create_complex_workflow_result("filtered_test", "run_filtered");

    let mut config = ReportingConfig {
        workflow_completion_reports: true,
        ..Default::default()
    };

    let mut template = ReportTemplate::new("filtered_report".to_string(), "detailed".to_string());
    template.trigger_on_workflow_completion = true;
    template.filters.workflow_names = Some(vec!["filtered_test".to_string()]);
    template.filters.task_types = Some(vec!["command".to_string()]);
    template.delivery_channels = vec![DeliveryChannel::new_terminal()];
    config.templates.push(template);

    let result = reporting_engine
        .process_workflow_completion(&workflow_result, &config)
        .await;
    assert!(result.is_ok());

    // Test with workflow that doesn't match filter
    let other_result = create_mock_workflow_result("other_workflow", "run_other");
    let result = reporting_engine
        .process_workflow_completion(&other_result, &config)
        .await;
    assert!(result.is_ok()); // Should succeed but not generate report
}

#[tokio::test]
async fn test_report_template_variables() {
    let temp_dir = TempDir::new().unwrap();
    let report_file = temp_dir.path().join("template_vars_report.txt");

    let reporting_engine = ReportingEngine::new();
    let workflow_result = create_mock_workflow_result("template_test", "run_template");

    let mut config = ReportingConfig {
        workflow_completion_reports: true,
        ..Default::default()
    };
    config
        .global_variables
        .insert("environment".to_string(), "test".to_string());
    config
        .global_variables
        .insert("version".to_string(), "1.0".to_string());

    let mut template =
        ReportTemplate::new("template_vars_report".to_string(), "summary".to_string());
    template.trigger_on_workflow_completion = true;
    template
        .variables
        .insert("report_type".to_string(), "integration_test".to_string());
    template.delivery_channels = vec![DeliveryChannel::new_file(
        report_file.to_string_lossy().to_string(),
    )];
    config.templates.push(template);

    let result = reporting_engine
        .process_workflow_completion(&workflow_result, &config)
        .await;
    assert!(result.is_ok());

    // Verify template variables were processed (file should contain workflow info)
    assert!(report_file.exists());
    let content = fs::read_to_string(&report_file).await.unwrap();
    assert!(content.contains("template_test"));
}

#[tokio::test]
async fn test_reporting_with_disabled_channels() {
    let temp_dir = TempDir::new().unwrap();
    let report_file = temp_dir.path().join("disabled_channel_report.txt");

    let reporting_engine = ReportingEngine::new();
    let workflow_result = create_mock_workflow_result("disabled_test", "run_disabled");

    let mut config = ReportingConfig {
        workflow_completion_reports: true,
        ..Default::default()
    };

    let mut template =
        ReportTemplate::new("disabled_channel_report".to_string(), "summary".to_string());
    template.trigger_on_workflow_completion = true;

    // Add a disabled delivery channel
    let mut disabled_channel = DeliveryChannel::new_file(report_file.to_string_lossy().to_string());
    disabled_channel.enabled = false;
    template.delivery_channels = vec![disabled_channel];

    config.templates.push(template);

    let result = reporting_engine
        .process_workflow_completion(&workflow_result, &config)
        .await;
    assert!(result.is_ok());

    // File should not be created since channel is disabled
    assert!(!report_file.exists());
}

#[tokio::test]
async fn test_reporting_rate_limits() {
    let reporting_engine = ReportingEngine::new();

    let mut config = ReportingConfig {
        workflow_completion_reports: true,
        ..Default::default()
    };

    // Configure rate limits
    config.rate_limits.reports_per_minute = Some(2);

    let mut template =
        ReportTemplate::new("rate_limited_report".to_string(), "summary".to_string());
    template.trigger_on_workflow_completion = true;
    template.delivery_channels = vec![DeliveryChannel::new_terminal()];
    config.templates.push(template);

    // Send multiple reports quickly - all should succeed as rate limiting is just configured
    for i in 0..3 {
        let workflow_result =
            create_mock_workflow_result(&format!("rate_test_{}", i), &format!("run_rate_{}", i));
        let result = reporting_engine
            .process_workflow_completion(&workflow_result, &config)
            .await;
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_reporting_generators() {
    let reporting_engine = ReportingEngine::new();

    // Test that all built-in generators are available
    let generators = reporting_engine.list_generators();
    assert!(generators.contains(&"summary"));
    assert!(generators.contains(&"detailed"));
    assert!(generators.contains(&"metrics"));

    // Test that all built-in deliverers are available
    let deliverers = reporting_engine.list_deliverers();
    assert!(deliverers.contains(&"email"));
    assert!(deliverers.contains(&"slack"));
    assert!(deliverers.contains(&"file"));
    assert!(deliverers.contains(&"terminal"));
}

#[tokio::test]
async fn test_workflow_context_creation() {
    let context = WorkflowContext {
        workflow_name: "context_test".to_string(),
        run_id: "run_context".to_string(),
        start_time: chrono::Utc::now(),
        metadata: {
            let mut meta = HashMap::new();
            meta.insert("environment".to_string(), "test".to_string());
            meta.insert("version".to_string(), "1.0".to_string());
            meta
        },
    };

    assert_eq!(context.workflow_name, "context_test");
    assert_eq!(context.run_id, "run_context");
    assert_eq!(context.metadata.len(), 2);
    assert_eq!(context.metadata.get("environment").unwrap(), "test");
}

#[tokio::test]
async fn test_reporting_error_handling() {
    let reporting_engine = ReportingEngine::new();
    let workflow_result = create_mock_workflow_result("error_test", "run_error");

    let mut config = ReportingConfig {
        workflow_completion_reports: true,
        ..Default::default()
    };

    // Create template with non-existent generator
    let mut template = ReportTemplate::new(
        "error_report".to_string(),
        "nonexistent_generator".to_string(),
    );
    template.trigger_on_workflow_completion = true;
    template.delivery_channels = vec![DeliveryChannel::new_terminal()];
    config.templates.push(template);

    let result = reporting_engine
        .process_workflow_completion(&workflow_result, &config)
        .await;
    assert!(result.is_err());

    // Error should mention the missing generator
    let error = result.unwrap_err();
    assert!(error.to_string().contains("nonexistent_generator"));
}

#[tokio::test]
async fn test_concurrent_reporting() {
    let reporting_engine = std::sync::Arc::new(ReportingEngine::new());

    let mut config = ReportingConfig {
        workflow_completion_reports: true,
        ..Default::default()
    };

    let mut template = ReportTemplate::new("concurrent_report".to_string(), "summary".to_string());
    template.trigger_on_workflow_completion = true;
    template.delivery_channels = vec![DeliveryChannel::new_terminal()];
    config.templates.push(template);

    let config = std::sync::Arc::new(config);

    // Generate multiple reports concurrently
    let mut handles = vec![];
    for i in 0..5 {
        let engine = reporting_engine.clone();
        let config = config.clone();

        let handle = tokio::spawn(async move {
            let workflow_result = create_mock_workflow_result(
                &format!("concurrent_{}", i),
                &format!("run_concurrent_{}", i),
            );
            engine
                .process_workflow_completion(&workflow_result, &config)
                .await
        });

        handles.push(handle);
    }

    // Wait for all reports to complete
    let results = futures::future::join_all(handles).await;

    // All reports should succeed
    for result in results {
        assert!(result.unwrap().is_ok());
    }
}

#[tokio::test]
async fn test_metrics_period_calculation() {
    let collector = MetricsCollector::new();

    // Add some test data
    let workflow_result1 = create_mock_workflow_result("period_test_1", "run_1");
    let workflow_result2 = create_complex_workflow_result("period_test_2", "run_2");

    collector
        .record_workflow_completion(&workflow_result1)
        .await;
    collector
        .record_workflow_completion(&workflow_result2)
        .await;

    // Test different period metrics
    let hourly_metrics = collector.get_period_metrics(&ReportPeriod::Hourly).await;
    assert!(matches!(hourly_metrics.period, ReportPeriod::Hourly));

    let daily_metrics = collector.get_period_metrics(&ReportPeriod::Daily).await;
    assert!(matches!(daily_metrics.period, ReportPeriod::Daily));

    let weekly_metrics = collector.get_period_metrics(&ReportPeriod::Weekly).await;
    assert!(matches!(weekly_metrics.period, ReportPeriod::Weekly));

    let monthly_metrics = collector.get_period_metrics(&ReportPeriod::Monthly).await;
    assert!(matches!(monthly_metrics.period, ReportPeriod::Monthly));
}

#[tokio::test]
async fn test_metrics_collection_limits() {
    let collector = MetricsCollector::new().with_retention_limit(2);

    // Add more workflows than the retention limit
    for i in 0..5 {
        let workflow_result =
            create_mock_workflow_result(&format!("retention_test_{}", i), &format!("run_{}", i));
        collector.record_workflow_completion(&workflow_result).await;
    }

    // Should only have the last 2 workflows
    let all_metrics = collector
        .get_workflow_metrics("retention_test_3", None)
        .await;
    assert!(all_metrics.len() <= 1);

    let latest_metrics = collector
        .get_workflow_metrics("retention_test_4", None)
        .await;
    assert_eq!(latest_metrics.len(), 1);
}
