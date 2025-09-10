# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Enhanced command task type with script support
- Full template processing in shell scripts
- Comprehensive GitHub Actions CI/CD pipeline
- Multi-platform binary releases
- Automated dependency management
- Documentation deployment to GitHub Pages

### Changed

- Improved template engine to disable HTML escaping for shell scripts
- Enhanced workflow variable resolution at execution time
- Updated examples to showcase new script functionality

### Fixed

- Template processing in workflow variables
- Command task validation for dual command/script modes
- Integration test compatibility with new features

## [0.1.0] - 2024-XX-XX

### Added

- Initial release of Wayfinder workflow engine
- CLI-based workflow execution with declarative YAML
- Template engine with Handlebars support
- Multiple task types:
  - Command execution
  - File compression and decompression
  - Checksum generation and validation
  - S3 upload/download
  - Email notifications (SMTP/SES)
  - Slack notifications
- AWS integration for S3 and SES services
- Dependency graph resolution and parallel execution
- Comprehensive error handling and reporting
- Example workflows demonstrating core features
- Just-based build system with development recipes

### Features

- **Template System**: Full Handlebars template support with built-in helpers
  - System information: `{{system.hostname}}`, `{{system.os}}`, `{{system.user}}`
  - Time functions: `{{timestamp}}`, `{{format_time}}`
  - Environment variables: `{{env 'VAR' 'default'}}`
  - Utility functions: `{{uuid}}`, `{{base64_encode}}`, `{{upper}}`, `{{lower}}`
- **Task Types**: Comprehensive task library for common workflow operations
- **Parallel Execution**: Intelligent dependency resolution and concurrent task execution
- **Error Handling**: Robust error handling with cleanup tasks and retry logic
- **Configuration**: Flexible configuration system with CLI overrides
- **Output Formats**: Multiple output formats including JSON and structured reports

### Technical Details

- Built with Rust 2021 edition
- Async/await throughout with Tokio runtime
- Comprehensive test suite with unit and integration tests
- Cross-platform support (Linux, macOS, Windows)
- Memory-safe execution with proper resource cleanup
