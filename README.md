# Wayfinder 🧭

A powerful CLI-based workflow engine for executing declarative YAML workflows with advanced templating, dependency management, and task orchestration capabilities.

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)]()
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)

## ✨ Features

- **📋 Declarative YAML Workflows** - Define complex workflows using simple, readable YAML syntax
- **🔗 Dependency Management** - Automatic task scheduling with dependency resolution and cycle detection
- **🎯 Template System** - Powerful Handlebars-based templating with built-in helpers for dynamic workflows
- **⚡ Parallel Execution** - Run independent tasks concurrently for maximum performance
- **🔄 Task Types** - Rich set of built-in task types including command, compression, checksum, and more
- **📊 Progress Tracking** - Real-time execution monitoring with detailed logging
- **🛡️ Error Handling** - Robust error handling with configurable retry policies and failure modes
- **📈 Reporting** - Generate detailed execution reports in multiple formats

## 🚀 Quick Start

### Installation

```bash
# Build from source
git clone https://github.com/sarlalian/wayfinder.git
cd wayfinder
cargo build --release

# The binary will be available at target/release/wayfinder
```

### Your First Workflow

Create a simple workflow file `hello.yaml`:

```yaml
name: hello_world
description: A simple hello world workflow

variables:
  greeting: "Hello"
  target: "World"

tasks:
  greet:
    type: command
    description: "Say hello"
    config:
      command: echo
      args: ["{{variables.greeting}} {{variables.target}}!"]
    required: true

output:
  destination: "stdout://"
```

Run it:

```bash
wayfinder run hello.yaml
```

## 📖 Usage

### CLI Commands

```bash
# Execute a workflow
wayfinder run workflow.yaml

# Validate without executing
wayfinder validate workflow.yaml

# Run with variable overrides
wayfinder run workflow.yaml --var environment=production --var debug=false

# Dry run to see execution plan
wayfinder run workflow.yaml --dry-run

# Control concurrency
wayfinder run workflow.yaml --max-concurrent 4

# Generate reports
wayfinder report results.json --format html --output report.html
```

### Workflow Structure

A workflow consists of:

```yaml
name: workflow_name                    # Required: Workflow identifier
description: "Workflow description"    # Optional: Human-readable description  
version: "1.0"                        # Optional: Version identifier
author: "Your Name"                   # Optional: Author information

variables:                            # Optional: Template variables
  key: "value"
  environment: "{{env 'ENVIRONMENT' 'development'}}"

tasks:                               # Required: Task definitions
  task_name:
    type: command                    # Required: Task type
    description: "Task description"  # Optional: Task description
    config:                         # Required: Task-specific configuration
      # ... task config
    depends_on: [other_task]        # Optional: Dependencies
    required: true                  # Optional: Whether task is required (default: true)
    when: "{{variables.environment == 'production'}}"  # Optional: Conditional execution
    timeout_seconds: 300            # Optional: Task timeout
    retry_config:                   # Optional: Retry configuration
      max_attempts: 3
      delay_seconds: 5

output:                             # Optional: Output configuration
  destination: "file://output.json"

on_error:                           # Optional: Error handling
  continue: false                   # Whether to continue on errors
  cleanup_tasks: [cleanup]          # Tasks to run on error
```

## 🛠️ Task Types

### Command Task

Execute shell commands or scripts:

```yaml
tasks:
  simple_command:
    type: command
    config:
      command: echo
      args: ["Hello World"]
      
  script_command:
    type: command
    config:
      script: |
        #!/bin/bash
        echo "Starting backup process..."
        mysqldump -u {{variables.db_user}} -p{{variables.db_pass}} mydb > backup.sql
        echo "Backup completed!"
      shell: /bin/bash
      env:
        MYSQL_PWD: "{{variables.db_pass}}"
    timeout_seconds: 300
```

### Compression Task

Compress files using various algorithms:

```yaml
tasks:
  compress_file:
    type: compress
    config:
      input_path: "./data.txt"
      output_path: "./data.txt.bz2"
      compression_type: bzip2      # Options: bzip2, xz, lzma
      compression_level: 9         # 1-9 (optional)
      preserve_original: false     # Keep original file (default: false)
```

### Checksum Task

Calculate file checksums:

```yaml
tasks:
  verify_integrity:
    type: checksum
    config:
      input_path: "./important_file.txt"
      output_path: "./important_file.txt.sha256"
      algorithm: sha256            # Options: sha256, sha512, md5
      base64_encode: false         # Encode output as base64 (default: false)
```

### S3 Upload/Download

Work with AWS S3:

```yaml
tasks:
  upload_to_s3:
    type: s3_upload
    config:
      local_path: "./backup.tar.gz"
      bucket: "my-backup-bucket"
      key: "backups/{{timestamp '%Y%m%d'}}/backup.tar.gz"
      region: "us-west-2"
```

### Email & Slack Notifications

Send notifications:

```yaml
tasks:
  notify_completion:
    type: email
    config:
      to: ["admin@company.com"]
      subject: "Workflow {{workflow.name}} completed"
      template_path: "./email_template.html"
      
  slack_alert:
    type: slack
    config:
      webhook_url: "{{env 'SLACK_WEBHOOK'}}"
      channel: "#deployments"
      message: "🚀 Deployment completed successfully!"
```

## 🎨 Template System

Wayfinder uses Handlebars templating with powerful built-in helpers:

### Variables
```yaml
variables:
  environment: "production"
  version: "1.2.3"

# Usage: {{variables.environment}}
```

### Built-in Helpers

- **`{{timestamp}}`** - Current timestamp
- **`{{timestamp '%Y-%m-%d'}}`** - Formatted timestamp
- **`{{env 'VAR_NAME' 'default'}}`** - Environment variables with defaults
- **`{{hostname}}`** - Current hostname
- **`{{uuid}}`** - Generate UUID v4
- **`{{base64_encode 'text'}}`** - Base64 encoding
- **`{{base64_decode 'dGV4dA=='}}`** - Base64 decoding
- **`{{upper 'text'}}`** / **`{{lower 'TEXT'}}`** - Case conversion
- **`{{default value 'fallback'}}`** - Default values

### Workflow Context

- **`{{workflow.name}}`** - Workflow name
- **`{{workflow.run_id}}`** - Unique execution ID
- **`{{system.hostname}}`** - System hostname
- **`{{system.username}}`** - Current user

## 📦 Example Workflows

### Database Backup Pipeline

```yaml
name: mysql-backup
description: Complete MySQL backup with compression and verification

variables:
  database_name: "production_db"
  backup_retention_days: "7"
  s3_bucket: "company-backups"

tasks:
  create_dump:
    type: command
    description: "Create MySQL database dump"
    config:
      script: |
        BACKUP_FILE="{{variables.database_name}}-{{timestamp '%Y%m%d'}}.sql"
        mysqldump -u backup_user -p{{env 'DB_PASSWORD'}} {{variables.database_name}} > "$BACKUP_FILE"
        echo "Created dump: $BACKUP_FILE"
      shell: /bin/bash
    required: true

  compress_backup:
    type: compress
    depends_on: [create_dump]
    config:
      input_path: "./{{variables.database_name}}-{{timestamp '%Y%m%d'}}.sql"
      output_path: "./{{variables.database_name}}-{{timestamp '%Y%m%d'}}.sql.bz2"
      compression_type: bzip2
      preserve_original: false

  verify_backup:
    type: checksum
    depends_on: [compress_backup]
    config:
      input_path: "./{{variables.database_name}}-{{timestamp '%Y%m%d'}}.sql.bz2"
      output_path: "./{{variables.database_name}}-{{timestamp '%Y%m%d'}}.sql.bz2.sha256"
      algorithm: sha256

  upload_to_s3:
    type: s3_upload
    depends_on: [verify_backup]
    config:
      local_path: "./{{variables.database_name}}-{{timestamp '%Y%m%d'}}.sql.bz2"
      bucket: "{{variables.s3_bucket}}"
      key: "mysql-backups/{{timestamp '%Y/%m/%d'}}/{{variables.database_name}}-{{timestamp '%Y%m%d'}}.sql.bz2"

  cleanup_old_backups:
    type: command
    depends_on: [upload_to_s3]
    config:
      script: |
        find ./backups -name "*.sql.bz2" -mtime +{{variables.backup_retention_days}} -delete
        echo "Cleaned up backups older than {{variables.backup_retention_days}} days"

  notify_success:
    type: slack
    depends_on: [upload_to_s3, cleanup_old_backups]
    config:
      webhook_url: "{{env 'SLACK_WEBHOOK'}}"
      message: "✅ Database backup completed successfully for {{variables.database_name}}"

output:
  destination: "file://./logs/backup-{{timestamp '%Y%m%d-%H%M%S'}}.json"

on_error:
  continue: false
  cleanup_tasks: [notify_failure]
```

### Parallel Processing Pipeline

```yaml
name: parallel-processing
description: Demonstrate parallel task execution

tasks:
  # These tasks will run in parallel
  process_data_1:
    type: command
    config:
      script: |
        echo "Processing dataset 1..."
        sleep 10
        echo "Dataset 1 complete"

  process_data_2:
    type: command
    config:
      script: |
        echo "Processing dataset 2..." 
        sleep 8
        echo "Dataset 2 complete"

  process_data_3:
    type: command
    config:
      script: |
        echo "Processing dataset 3..."
        sleep 12
        echo "Dataset 3 complete"

  # This task waits for all processing to complete
  combine_results:
    type: command
    depends_on: [process_data_1, process_data_2, process_data_3]
    config:
      command: echo
      args: ["All datasets processed, combining results..."]
```

## ⚙️ Configuration

Create a `wayfinder.toml` configuration file:

```toml
[logging]
level = "info"
format = "json"

[execution]
default_timeout_seconds = 300
max_concurrent_tasks = 4

[templates]
strict_mode = true

[reporting]
default_format = "json"
include_system_info = true

[s3]
default_region = "us-west-2"

[notifications]
default_slack_channel = "#workflows"
```

## 🧪 Testing & Validation

```bash
# Validate workflow syntax
wayfinder validate workflow.yaml

# Dry run to see execution plan
wayfinder run workflow.yaml --dry-run

# Run with verbose logging
wayfinder run workflow.yaml --verbose

# Test with different variable values
wayfinder run workflow.yaml --var environment=staging --var debug=true
```

## 🐛 Error Handling & Debugging

### Retry Configuration

```yaml
tasks:
  flaky_task:
    type: command
    config:
      command: ./flaky_script.sh
    retry_config:
      max_attempts: 3
      delay_seconds: 5
      backoff_multiplier: 2.0
```

### Conditional Execution

```yaml
tasks:
  production_only:
    type: command
    when: "{{variables.environment == 'production'}}"
    config:
      command: ./production_script.sh
```

### Error Handling

```yaml
on_error:
  continue: false                    # Stop on first error
  cleanup_tasks: [cleanup, notify]   # Run cleanup tasks on failure
```

## 📊 Reporting

Generate detailed execution reports:

```bash
# Generate HTML report
wayfinder report execution.json --format html --output report.html

# Generate CSV summary
wayfinder report execution.json --format csv --output summary.csv

# Generate JSON report with full details
wayfinder report execution.json --format json --output detailed.json
```

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Development Setup

```bash
# Clone the repository
git clone https://github.com/sarlalian/wayfinder.git
cd wayfinder

# Install dependencies
cargo build

# Run tests
cargo test

# Run with development logging
RUST_LOG=debug cargo run -- run examples/simple_workflow.yaml

# Check code formatting
cargo fmt --all -- --check

# Run linter
cargo clippy --all-targets --all-features -- -D warnings
```

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Built with [Rust](https://www.rust-lang.org/) for performance and safety
- Uses [Tokio](https://tokio.rs/) for async runtime
- Template engine powered by [Handlebars](https://handlebarsjs.com/)
- Dependency management via [petgraph](https://github.com/petgraph/petgraph)

## 📚 Additional Resources

- [Examples Directory](examples/) - Complete workflow examples
- [Configuration Reference](docs/configuration.md) - Detailed configuration options
- [Template Helper Reference](docs/templates.md) - All available template helpers
- [Task Type Reference](docs/tasks.md) - Complete task type documentation

---

**Wayfinder** - Navigate your workflows with confidence! 🧭