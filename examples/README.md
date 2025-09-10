# Wayfinder Example Workflows

This directory contains example workflows that demonstrate various features and patterns of the Wayfinder workflow engine.

## Available Examples

### 1. Simple Workflow (`simple_workflow.yaml`)

**Purpose**: Demonstrates basic sequential task execution with variable substitution.

**Features**:
- Variable substitution
- Sequential task dependencies
- Basic command execution
- Timeout configuration

**Usage**:
```bash
wayfinder execute examples/simple_workflow.yaml
```

**What it demonstrates**:
- How to define workflow metadata
- Using variables in task configurations
- Creating task dependencies with `depends_on`
- Basic command execution with the `command` task type

---

### 2. Parallel Workflow (`parallel_workflow.yaml`)

**Purpose**: Shows how to execute tasks in parallel while managing dependencies.

**Features**:
- Concurrent task execution
- Complex dependency relationships
- Batch processing pattern
- Environment setup and cleanup

**Usage**:
```bash
wayfinder execute examples/parallel_workflow.yaml
```

**What it demonstrates**:
- Parallel execution of independent tasks
- Fan-out/fan-in patterns (multiple parallel tasks feeding into aggregation)
- Proper resource setup and cleanup
- Variable usage for configuration

---

### 3. Retry Workflow (`retry_workflow.yaml`)

**Purpose**: Demonstrates retry mechanisms for handling flaky operations.

**Features**:
- Retry configuration with exponential backoff
- Different retry strategies for different task types
- Failure simulation and handling
- Resource cleanup

**Usage**:
```bash
wayfinder execute examples/retry_workflow.yaml
```

**What it demonstrates**:
- Configuring retry attempts with `max_attempts`
- Setting up backoff strategies with `delay` and `backoff_multiplier`
- Handling different types of failures
- When to use retries vs. when to let tasks fail

---

### 4. Complex Data Pipeline (`complex_workflow.yaml`)

**Purpose**: A comprehensive example showing a real-world data processing pipeline.

**Features**:
- Multiple task types (command, http, s3, email)
- Complex dependency graph
- Data validation and quality checks
- Multi-phase processing (extract, transform, load)
- Error handling and notifications
- Resource cleanup and archiving

**Usage**:
```bash
wayfinder execute examples/complex_workflow.yaml
```

**What it demonstrates**:
- Real-world workflow patterns
- Using different task types effectively
- Data pipeline best practices
- Comprehensive error handling
- Notification and monitoring patterns

## Common Patterns Demonstrated

### Variable Substitution
All examples show how to use variables in workflow definitions:
```yaml
variables:
  environment: "production"
  database_host: "db.example.com"

tasks:
  - id: connect_db
    config:
      command: psql
      args: ["-h", "${database_host}", "-d", "${environment}_db"]
```

### Task Dependencies
Examples show various dependency patterns:
```yaml
# Simple dependency
- id: task_b
  depends_on:
    - task_a

# Multiple dependencies
- id: final_task
  depends_on:
    - task_x
    - task_y
    - task_z
```

### Retry Configuration
Multiple examples demonstrate retry mechanisms:
```yaml
retry:
  max_attempts: 3
  delay: 5s
  backoff_multiplier: 2.0
  max_delay: 30s
```

### Error Handling
Examples show how to structure workflows for resilience:
- Use retries for transient failures
- Create cleanup tasks that run regardless of failures
- Structure dependencies to minimize cascade failures

## Running the Examples

1. **Basic execution**:
   ```bash
   wayfinder execute examples/simple_workflow.yaml
   ```

2. **With custom variables**:
   ```bash
   wayfinder execute examples/simple_workflow.yaml --variable greeting="Hi" --variable target="Everyone"
   ```

3. **With output formatting**:
   ```bash
   wayfinder execute examples/parallel_workflow.yaml --output results.json --format json
   ```

4. **Dry run mode**:
   ```bash
   wayfinder execute examples/complex_workflow.yaml --dry-run
   ```

5. **With verbose logging**:
   ```bash
   wayfinder execute examples/retry_workflow.yaml --verbose
   ```

## Customizing Examples

Each example can be customized by:

1. **Modifying variables**: Change the values in the `variables` section
2. **Adding tasks**: Follow the existing patterns to add new tasks
3. **Changing dependencies**: Modify the `depends_on` arrays to change execution order
4. **Adjusting timeouts**: Modify the `timeout` values based on your environment
5. **Updating retry settings**: Adjust retry parameters for your use case

## Best Practices Demonstrated

1. **Clear task naming**: Use descriptive task IDs and descriptions
2. **Proper dependency management**: Structure dependencies to enable maximum parallelism
3. **Resource cleanup**: Always include cleanup tasks
4. **Error handling**: Use retries appropriately and handle failures gracefully
5. **Variable usage**: Externalize configuration through variables
6. **Documentation**: Include clear descriptions for workflows and tasks

## Next Steps

After exploring these examples:

1. Try modifying them to suit your use cases
2. Create your own workflows using these patterns
3. Experiment with different task types and configurations
4. Set up monitoring and reporting for your workflows
5. Integrate with your existing tools and systems