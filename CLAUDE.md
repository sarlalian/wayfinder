# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Wayfinder is a CLI-based workflow engine in Rust that executes declarative YAML workflows with task dependencies, templating, and native task types. The project is currently in bootstrap phase with a minimal "Hello, world!" application.

## Planned Architecture

Based on `docs/PLAN.md`, the system will include:

### Core Components
1. **CLI Application** - Main entry point using Clap
2. **Workflow Parser** - YAML parsing and validation
3. **Template Engine** - Text templating with functions like hostname, times, etc.
4. **Task Engine** - Execution framework with dependency resolution
5. **Native Tasks** - Built-in implementations for various task types
6. **Output Handler** - JSON output to file:// or s3:// destinations
7. **Reporting Engine** - Status reporting via email/slack/terminal

### Native Task Types (Planned)
- Command execution
- Compress/decompress (bzip2, xz, lza)
- Checksum calculation and validation (SHA256)
- S3 upload/download with tags and ACL
- Email via SMTP or AWS SES
- Slack messaging

### Key Design Decisions
- Modular task system with common interface
- Dependency graph using topological sort
- Template resolution at workflow start
- Configurable retry with exponential backoff
- Structured logging with task-specific context

## Current Project Structure

- `src/main.rs`: Main entry point (currently minimal)
- `Cargo.toml`: Package manifest using Rust 2024 edition
- `docs/PLAN.md`: Detailed implementation plan and architecture
- `/target`: Build artifacts directory (gitignored)

## Development Commands

### Building and Running
```bash
cargo build          # Compile the project
cargo run            # Build and run the application
cargo check          # Fast compile check without building binaries
```

### Testing
```bash
cargo test           # Run all tests
cargo test [NAME]    # Run specific test by name
cargo test -- --nocapture  # Run tests with stdout output
```

### Code Quality
```bash
cargo clippy         # Run lint checks with Clippy
cargo clippy --fix   # Automatically fix lint issues
cargo fmt            # Format code according to Rust standards
```

### Other Useful Commands
```bash
cargo clean          # Remove build artifacts
cargo doc --open     # Generate and open documentation
cargo bench          # Run benchmarks (if any)
```

## Architecture Notes

This is currently a minimal Rust application with a single main.rs file. The project uses:
- Rust 2024 edition
- Standard Cargo project layout
- No external dependencies currently configured

Since this appears to be a new/template project, future development should follow standard Rust patterns for organizing modules, tests, and dependencies as the codebase grows.

## Version Control

The project uses Jujutsu (jj) for version control based on the `.jj` directory presence, with git as a fallback option per the user's preferences.
