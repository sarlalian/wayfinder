Building a CLI-based workflow engine in Rust that executes declarative YAML workflows with task dependencies, templating, and native task types, including:
- command execution
- compress/decompress
- checksum/validate checksum
- Send email via SMTP or AWS SES
- S3 upload/download
- Post messages to slack


Core Components

1. **CLI Application** - Main entry point using Cobra
2. **Workflow Parser** - YAML parsing and validation
3. **Template Engine** - text/template with functions like hostname, times, etc.
4. **Task Engine** - Execution framework with dependency resolution.
5. **Native Tasks** - Implementations for compress, checksum, S3 upload, command, etc.
6. **Output Handler** - JSON output of results, including task status and overall workflow status to file:// or s3:// destinations.
7. **Reporting Engine** - Reads files from file:// and s3:// destinations, and generates a report to email/slack/terminal of the overal status of expected tasks.


### Key Design Decisions

1. **Modular Task System**: Each task type implements a common interface
2. **Dependency Graph**: Use topological sort for execution order
3. **Template Resolution**: Evaluate all templates once at workflow start
4. **Error Handling**: Distinguish between required and optional tasks
5. **Retry Logic**: Configurable retry with exponential backoff
6. **Logging**: Structured logging with task-specific context

## Implementation Phases

### Phase 1: Foundation (Steps 1-4)
- Project setup with Rust modules
- Core data structures and interfaces
- Basic CLI skeleton using clap
- Unit test framework

### Phase 2: Workflow Engine Core (Steps 5-8)
- YAML parser and workflow model
- Dependency resolver
- Task executor framework
- Template engine integration, most strings should support templating.

### Phase 3: Native Tasks (Steps 9-12)
- Command task implementation
- Compress/Decompress task implementation
- Checksum/Validate task implementation
- S3 upload/download task implementation
- SMTP / SES Email Sending
- Posting to Slack

### Phase 4: Output & Error Handling (Steps 13-15)
- JSON output formatter
- File and S3 output handlers
- Error handling and retry logic

### Phase 5: Integration & Polish (Steps 16-18)
- End-to-end workflow execution
- Comprehensive testing
- Documentation and examples

## Detailed Implementation Steps

### Step 1: Project Bootstrap
- Initialize Rust module
- Set up project structure
- Add essential dependencies (Clap, YAML, AWS SDK)
- Create Justfile for common tasks

### Step 2: Core Interfaces
- Define Task interface
- Define Workflow and TaskConfig structs
- Create error types
- Set up logging framework

### Step 3: CLI Framework
- Implement main command with Clap
- Add version and help commands
- Parse command-line arguments
- Set up configuration loading

### Step 4: Test Infrastructure
- Set up testing utilities
- Create mock implementations
- Add test fixtures
- Implement test helpers

### Step 5: YAML Parser
- Implement workflow YAML unmarshaling
- Add validation logic
- Create parser tests
- Handle configuration errors

### Step 6: Template Engine
- Integrate text/template with Sprig
- Implement built-in variables
- Create template resolver
- Add template testing

### Step 7: Dependency Resolver
- Implement topological sort
- Detect circular dependencies
- Create execution plan
- Test dependency scenarios

### Step 8: Task Executor
- Implement sequential executor
- Add task lifecycle management
- Handle task results
- Implement retry logic

### Step 9: Compress Task
- Implement bzip2 compression
- Add xz compression
- Add lza compression
- Implement decompression for bzip2, xz and lza
- Create compression/decompression tests

### Step 10: Checksum Task
- Implement SHA256 calculation
- Implement SHA256 verification
- Add base64 encoding/decoding
- Write output file (default extension .checksum)
- Test checksum generation and validation

### Step 11: Command Task
- Implement command execution
- Handle working directory
- Manage environment variables (should be able to be loaded from a .env file)
- Capture stdout/stderr
- Support saving stdout/stderr to a path.

### Step 12: S3 Upload/Download Task
- Integrate AWS SDK
- Handle authentication methods
- Implement upload with tags
- Add ACL support

### Step 13: Output Formatter
- Create JSON output structure
- Implement task result collection
- Add workflow status aggregation
- Format timestamps

### Step 14: Output Handlers
- Implement file:// output
- Implement s3:// output
- Add output templating
- Handle output errors

### Step 15: Error & Retry Logic
- Implement retry with backoff
- Handle required vs optional
- Add detailed error messages
- Log failure information

### Step 16: Integration
- Wire all components together
- Implement full workflow execution
- Add integration tests
- Test real workflows

### Step 17: Examples & Testing
- Create example workflows
- Add comprehensive unit tests
- Implement integration tests
- Performance testing

### Step 18: Documentation
- Write user documentation
- Add code documentation
- Create workflow examples
- Document best practices


## Testing Strategy

1. **Unit Tests**: Each component tested in isolation
2. **Integration Tests**: Full workflow execution tests
3. **Example Workflows**: Real-world usage scenarios
4. **Error Cases**: Test all failure modes
5. **Performance**: Benchmark critical paths

## Incremental Development Approach

Each step builds upon the previous one:
- Start with minimal viable implementation
- Add features incrementally
- Maintain working state after each step
- Test continuously
- Refactor as needed

## Success Criteria

- All native task types implemented
- Dependency resolution working correctly
- Template system fully functional
- Comprehensive error handling
- Well-tested and documented
- Example workflows provided
