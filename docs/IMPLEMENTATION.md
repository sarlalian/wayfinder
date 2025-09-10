# ABOUTME: Implementation plan for all modules in the wayfinder workflow engine
# ABOUTME: Provides detailed breakdown of each module's structure, dependencies, and implementation approach

# Wayfinder Implementation Plan

## Module Overview

Based on the architecture defined in `PLAN.md`, the wayfinder workflow engine consists of 7 core modules:

1. **CLI Application** (`src/cli/`)
2. **Workflow Parser** (`src/parser/`)  
3. **Template Engine** (`src/template/`)
4. **Task Engine** (`src/engine/`)
5. **Native Tasks** (`src/tasks/`)
6. **Output Handler** (`src/output/`)
7. **Reporting Engine** (`src/reporting/`)

---

## 1. CLI Application Module (`src/cli/`)

### Purpose
Main entry point using Clap for command-line argument parsing and application orchestration.

### Dependencies
```toml
clap = { version = "4.4", features = ["derive", "env"] }
tokio = { version = "1.0", features = ["full"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1.0"
```

### File Structure
```
src/cli/
├── mod.rs           # Module exports
├── args.rs          # Clap argument definitions and parsing
├── commands.rs      # Command implementations (run, validate, etc.)
├── config.rs        # Configuration loading and validation
└── app.rs           # Main application orchestration
```

### Key Components

#### `args.rs` - Command Line Interface
```rust
#[derive(Parser)]
#[command(name = "wayfinder")]
#[command(about = "A CLI-based workflow engine")]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
    
    #[arg(short, long, global = true)]
    pub verbose: bool,
    
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Commands {
    Run {
        #[arg(help = "Path to workflow YAML file")]
        workflow: PathBuf,
        
        #[arg(short, long, help = "Override template variables")]
        vars: Vec<String>,
        
        #[arg(long, help = "Dry run - validate without executing")]
        dry_run: bool,
    },
    Validate {
        #[arg(help = "Path to workflow YAML file")]
        workflow: PathBuf,
    },
    Init {
        #[arg(help = "Name of the workflow to create")]
        name: String,
    },
}
```

#### `config.rs` - Configuration Management
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub default_output_dir: PathBuf,
    pub aws: AwsConfig,
    pub smtp: SmtpConfig,
    pub slack: SlackConfig,
    pub template_vars: HashMap<String, String>,
}

pub fn load_config(path: Option<PathBuf>) -> Result<Config> {
    // Load from ~/.wayfinder/config.yaml or specified path
    // Merge with environment variables
    // Validate configuration
}
```

#### `app.rs` - Application Orchestration
```rust
pub struct App {
    config: Config,
    engine: TaskEngine,
}

impl App {
    pub async fn run(&self, workflow_path: PathBuf, vars: Vec<String>) -> Result<()> {
        // 1. Parse workflow YAML
        // 2. Resolve templates with variables
        // 3. Create execution plan
        // 4. Execute workflow
        // 5. Generate output
    }
    
    pub async fn validate(&self, workflow_path: PathBuf) -> Result<()> {
        // Parse and validate workflow without execution
    }
}
```

---

## 2. Workflow Parser Module (`src/parser/`)

### Purpose
YAML parsing, validation, and workflow model definition.

### Dependencies
```toml
serde = { version = "1.0", features = ["derive"] }
serde_yaml = "0.9"
indexmap = "2.0"  # Preserves order for dependencies
url = "2.4"
regex = "1.10"
```

### File Structure
```
src/parser/
├── mod.rs           # Module exports
├── workflow.rs      # Workflow data structures
├── task.rs          # Task configuration structures
├── validation.rs    # Validation logic and rules
└── error.rs         # Parser-specific error types
```

### Key Components

#### `workflow.rs` - Core Data Structures
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub name: String,
    pub description: Option<String>,
    pub variables: HashMap<String, String>,
    pub tasks: IndexMap<String, TaskConfig>,
    pub output: OutputConfig,
    pub on_error: Option<ErrorConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskConfig {
    pub name: String,
    pub task_type: TaskType,
    pub depends_on: Vec<String>,
    pub required: bool,
    pub retry: Option<RetryConfig>,
    pub when: Option<String>,  // Conditional execution
    pub timeout: Option<Duration>,
    pub config: TaskParameters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TaskType {
    Command,
    Compress,
    Decompress,
    Checksum,
    ValidateChecksum,
    S3Upload,
    S3Download,
    Email,
    Slack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub backoff_multiplier: f64,
    pub initial_delay: Duration,
}
```

#### `validation.rs` - Workflow Validation
```rust
pub struct WorkflowValidator;

impl WorkflowValidator {
    pub fn validate(&self, workflow: &Workflow) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();
        
        // Check for circular dependencies
        self.validate_dependencies(&workflow.tasks, &mut report)?;
        
        // Validate task configurations
        self.validate_task_configs(&workflow.tasks, &mut report)?;
        
        // Validate template syntax
        self.validate_templates(&workflow, &mut report)?;
        
        // Validate output configuration
        self.validate_output_config(&workflow.output, &mut report)?;
        
        Ok(report)
    }
    
    fn validate_dependencies(&self, tasks: &IndexMap<String, TaskConfig>, report: &mut ValidationReport) -> Result<()> {
        // Topological sort to detect cycles
        // Ensure all dependencies exist
    }
}
```

---

## 3. Template Engine Module (`src/template/`)

### Purpose
Text templating with built-in functions for hostname, timestamps, environment variables, etc.

### Dependencies
```toml
handlebars = "4.4"
chrono = { version = "0.4", features = ["serde"] }
hostname = "0.3"
uuid = { version = "1.0", features = ["v4"] }
```

### File Structure
```
src/template/
├── mod.rs           # Module exports
├── engine.rs        # Main template engine
├── helpers.rs       # Built-in template helper functions
├── context.rs       # Template context management
└── functions.rs     # Custom template functions
```

### Key Components

#### `engine.rs` - Template Engine
```rust
pub struct TemplateEngine {
    handlebars: Handlebars<'static>,
}

impl TemplateEngine {
    pub fn new() -> Self {
        let mut handlebars = Handlebars::new();
        
        // Register built-in helpers
        handlebars.register_helper("hostname", Box::new(hostname_helper));
        handlebars.register_helper("timestamp", Box::new(timestamp_helper));
        handlebars.register_helper("uuid", Box::new(uuid_helper));
        handlebars.register_helper("env", Box::new(env_helper));
        handlebars.register_helper("file_exists", Box::new(file_exists_helper));
        
        Self { handlebars }
    }
    
    pub fn render(&self, template: &str, context: &TemplateContext) -> Result<String> {
        self.handlebars.render_template(template, context)
            .map_err(|e| TemplateError::RenderError(e.to_string()))
    }
    
    pub fn resolve_workflow(&self, workflow: &mut Workflow, variables: &HashMap<String, String>) -> Result<()> {
        let context = TemplateContext::new(variables);
        
        // Resolve all template strings in workflow
        for (_, task) in &mut workflow.tasks {
            self.resolve_task_config(task, &context)?;
        }
        
        Ok(())
    }
}
```

#### `helpers.rs` - Built-in Template Functions
```rust
pub fn hostname_helper(
    _h: &Helper,
    _: &Handlebars,
    _: &Context,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let hostname = hostname::get()
        .map_err(|_| RenderError::new("Failed to get hostname"))?
        .to_string_lossy();
    out.write(&hostname)?;
    Ok(())
}

pub fn timestamp_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let format = h.param(0)
        .and_then(|v| v.value().as_str())
        .unwrap_or("%Y-%m-%d %H:%M:%S");
    
    let now = chrono::Utc::now();
    let formatted = now.format(format).to_string();
    out.write(&formatted)?;
    Ok(())
}
```

#### `context.rs` - Template Context
```rust
#[derive(Debug, Clone, Serialize)]
pub struct TemplateContext {
    pub variables: HashMap<String, String>,
    pub env: HashMap<String, String>,
    pub system: SystemInfo,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub user: String,
    pub pwd: String,
}

impl TemplateContext {
    pub fn new(variables: &HashMap<String, String>) -> Self {
        let env = std::env::vars().collect();
        let system = SystemInfo::collect();
        
        Self {
            variables: variables.clone(),
            env,
            system,
        }
    }
}
```

---

## 4. Task Engine Module (`src/engine/`)

### Purpose
Execution framework with dependency resolution, parallel execution, and lifecycle management.

### Dependencies
```toml
tokio = { version = "1.0", features = ["full"] }
petgraph = "0.6"  # For dependency graphs
futures = "0.3"
tracing = "0.1"
```

### File Structure
```
src/engine/
├── mod.rs           # Module exports
├── executor.rs      # Main task executor
├── dependency.rs    # Dependency resolution and graph management
├── scheduler.rs     # Task scheduling and parallel execution
├── context.rs       # Execution context and state management
├── result.rs        # Task result handling
└── retry.rs         # Retry logic with exponential backoff
```

### Key Components

#### `executor.rs` - Main Executor
```rust
pub struct TaskExecutor {
    scheduler: TaskScheduler,
    context: ExecutionContext,
    registry: TaskRegistry,
}

impl TaskExecutor {
    pub async fn execute_workflow(&self, workflow: &Workflow) -> Result<WorkflowResult> {
        // 1. Build dependency graph
        let graph = DependencyGraph::from_workflow(workflow)?;
        
        // 2. Create execution plan
        let plan = graph.create_execution_plan()?;
        
        // 3. Execute tasks according to plan
        let mut results = WorkflowResult::new();
        
        for batch in plan.batches {
            let batch_results = self.execute_batch(batch, &workflow.tasks).await?;
            results.merge(batch_results);
            
            // Stop execution if any required task failed
            if results.has_critical_failure() {
                break;
            }
        }
        
        Ok(results)
    }
    
    async fn execute_batch(&self, batch: Vec<String>, tasks: &IndexMap<String, TaskConfig>) -> Result<BatchResult> {
        let futures = batch.into_iter().map(|task_id| {
            let task_config = &tasks[&task_id];
            let task = self.registry.create_task(&task_config.task_type)?;
            self.execute_task_with_retry(task_id.clone(), task, task_config)
        });
        
        let results = futures::future::join_all(futures).await;
        Ok(BatchResult::from_results(results))
    }
}
```

#### `dependency.rs` - Dependency Management
```rust
pub struct DependencyGraph {
    graph: petgraph::Graph<String, ()>,
    task_ids: HashMap<String, NodeIndex>,
}

impl DependencyGraph {
    pub fn from_workflow(workflow: &Workflow) -> Result<Self> {
        let mut graph = Graph::new();
        let mut task_ids = HashMap::new();
        
        // Add all tasks as nodes
        for (task_id, _) in &workflow.tasks {
            let node_id = graph.add_node(task_id.clone());
            task_ids.insert(task_id.clone(), node_id);
        }
        
        // Add dependency edges
        for (task_id, task_config) in &workflow.tasks {
            let task_node = task_ids[task_id];
            
            for dep in &task_config.depends_on {
                if let Some(&dep_node) = task_ids.get(dep) {
                    graph.add_edge(dep_node, task_node, ());
                } else {
                    return Err(DependencyError::UnknownDependency {
                        task: task_id.clone(),
                        dependency: dep.clone(),
                    }.into());
                }
            }
        }
        
        Ok(Self { graph, task_ids })
    }
    
    pub fn create_execution_plan(&self) -> Result<ExecutionPlan> {
        // Topological sort with batching for parallel execution
        let topo_order = petgraph::algo::toposort(&self.graph, None)
            .map_err(|_| DependencyError::CircularDependency)?;
        
        let batches = self.create_execution_batches(topo_order);
        Ok(ExecutionPlan { batches })
    }
}
```

#### `scheduler.rs` - Task Scheduling
```rust
pub struct TaskScheduler {
    max_concurrent: usize,
    semaphore: tokio::sync::Semaphore,
}

impl TaskScheduler {
    pub async fn execute_tasks<T>(&self, tasks: Vec<T>) -> Vec<T::Output>
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        let futures = tasks.into_iter().map(|task| {
            let permit = self.semaphore.clone();
            tokio::spawn(async move {
                let _permit = permit.acquire().await;
                task.await
            })
        });
        
        let results = futures::future::join_all(futures).await;
        results.into_iter().map(|r| r.unwrap()).collect()
    }
}
```

---

## 5. Native Tasks Module (`src/tasks/`)

### Purpose
Implementations for all built-in task types (command, compress, checksum, S3, email, Slack).

### Dependencies
```toml
tokio = { version = "1.0", features = ["full"] }
async-trait = "0.1"
aws-config = "0.55"
aws-sdk-s3 = "0.28"
aws-sdk-ses = "0.28"
reqwest = { version = "0.11", features = ["json"] }
lettre = "0.10"
flate2 = "1.0"
xz2 = "0.1"
lzma-rs = "0.3"
sha2 = "0.10"
base64 = "0.21"
```

### File Structure
```
src/tasks/
├── mod.rs           # Module exports and task registry
├── base.rs          # Task trait and common functionality
├── command.rs       # Command execution task
├── compress.rs      # Compression tasks (bzip2, xz, lza)
├── checksum.rs      # Checksum calculation and validation
├── s3.rs            # S3 upload/download tasks
├── email.rs         # SMTP and SES email tasks
├── slack.rs         # Slack messaging task
└── registry.rs      # Task factory and registration
```

### Key Components

#### `base.rs` - Task Interface
```rust
#[async_trait]
pub trait Task: Send + Sync {
    async fn execute(&self, context: &ExecutionContext) -> Result<TaskResult>;
    fn task_type(&self) -> TaskType;
    fn validate_config(&self, config: &TaskParameters) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct TaskResult {
    pub task_id: String,
    pub status: TaskStatus,
    pub output: Option<String>,
    pub error: Option<String>,
    pub duration: Duration,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Success,
    Failed,
    Skipped,
    Timeout,
}
```

#### `command.rs` - Command Task
```rust
pub struct CommandTask {
    config: CommandConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandConfig {
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: Option<PathBuf>,
    pub env_file: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub capture_output: bool,
    pub stdout_file: Option<PathBuf>,
    pub stderr_file: Option<PathBuf>,
    pub timeout: Option<Duration>,
}

#[async_trait]
impl Task for CommandTask {
    async fn execute(&self, context: &ExecutionContext) -> Result<TaskResult> {
        let mut cmd = tokio::process::Command::new(&self.config.command);
        cmd.args(&self.config.args);
        
        // Set working directory
        if let Some(ref wd) = self.config.working_dir {
            cmd.current_dir(wd);
        }
        
        // Load environment from .env file if specified
        if let Some(ref env_file) = self.config.env_file {
            self.load_env_file(env_file, &mut cmd)?;
        }
        
        // Set additional environment variables
        cmd.envs(&self.config.env);
        
        // Configure output capture
        if self.config.capture_output {
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());
        }
        
        let start = Instant::now();
        let output = tokio::time::timeout(
            self.config.timeout.unwrap_or(Duration::from_secs(3600)),
            cmd.output()
        ).await??;
        
        let duration = start.elapsed();
        
        // Save output to files if specified
        self.save_output(&output).await?;
        
        let status = if output.status.success() {
            TaskStatus::Success
        } else {
            TaskStatus::Failed
        };
        
        Ok(TaskResult {
            task_id: context.task_id.clone(),
            status,
            output: Some(String::from_utf8_lossy(&output.stdout).to_string()),
            error: if output.stderr.is_empty() { None } else { 
                Some(String::from_utf8_lossy(&output.stderr).to_string())
            },
            duration,
            metadata: HashMap::new(),
        })
    }
}
```

#### `compress.rs` - Compression Tasks
```rust
pub struct CompressTask {
    config: CompressConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressConfig {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub compression_type: CompressionType,
    pub compression_level: Option<u32>,
    pub preserve_original: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionType {
    Bzip2,
    Xz,
    Lza,
}

#[async_trait]
impl Task for CompressTask {
    async fn execute(&self, context: &ExecutionContext) -> Result<TaskResult> {
        let start = Instant::now();
        
        match self.config.compression_type {
            CompressionType::Bzip2 => self.compress_bzip2().await?,
            CompressionType::Xz => self.compress_xz().await?,
            CompressionType::Lza => self.compress_lza().await?,
        }
        
        let duration = start.elapsed();
        
        // Remove original file if not preserving
        if !self.config.preserve_original {
            tokio::fs::remove_file(&self.config.input_path).await?;
        }
        
        Ok(TaskResult {
            task_id: context.task_id.clone(),
            status: TaskStatus::Success,
            output: Some(format!("Compressed {} to {}", 
                self.config.input_path.display(),
                self.config.output_path.display()
            )),
            error: None,
            duration,
            metadata: HashMap::new(),
        })
    }
}
```

#### `s3.rs` - S3 Tasks
```rust
pub struct S3UploadTask {
    config: S3UploadConfig,
    client: aws_sdk_s3::Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3UploadConfig {
    pub local_path: PathBuf,
    pub bucket: String,
    pub key: String,
    pub acl: Option<String>,
    pub tags: HashMap<String, String>,
    pub content_type: Option<String>,
    pub metadata: HashMap<String, String>,
}

#[async_trait]
impl Task for S3UploadTask {
    async fn execute(&self, context: &ExecutionContext) -> Result<TaskResult> {
        let start = Instant::now();
        
        let body = ByteStream::from_path(&self.config.local_path).await?;
        
        let mut request = self.client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&self.config.key)
            .body(body);
        
        // Set ACL if specified
        if let Some(ref acl) = self.config.acl {
            request = request.acl(acl.parse()?);
        }
        
        // Set content type
        if let Some(ref content_type) = self.config.content_type {
            request = request.content_type(content_type);
        }
        
        // Set metadata
        for (key, value) in &self.config.metadata {
            request = request.metadata(key, value);
        }
        
        let result = request.send().await?;
        let duration = start.elapsed();
        
        // Apply tags if specified
        if !self.config.tags.is_empty() {
            self.apply_tags(&result).await?;
        }
        
        Ok(TaskResult {
            task_id: context.task_id.clone(),
            status: TaskStatus::Success,
            output: Some(format!("Uploaded {} to s3://{}/{}", 
                self.config.local_path.display(),
                self.config.bucket,
                self.config.key
            )),
            error: None,
            duration,
            metadata: HashMap::new(),
        })
    }
}
```

---

## 6. Output Handler Module (`src/output/`)

### Purpose
JSON output formatting and delivery to file:// or s3:// destinations.

### Dependencies
```toml
serde_json = "1.0"
tokio = { version = "1.0", features = ["fs"] }
aws-sdk-s3 = "0.28"
url = "2.4"
```

### File Structure
```
src/output/
├── mod.rs           # Module exports
├── formatter.rs     # JSON output formatting
├── handler.rs       # Output destination handling
├── file.rs          # File output implementation
├── s3.rs            # S3 output implementation
└── models.rs        # Output data structures
```

### Key Components

#### `formatter.rs` - Output Formatting
```rust
pub struct OutputFormatter;

impl OutputFormatter {
    pub fn format_workflow_result(&self, result: &WorkflowResult) -> Result<String> {
        let output = WorkflowOutput {
            workflow_name: result.workflow_name.clone(),
            status: result.overall_status(),
            start_time: result.start_time,
            end_time: result.end_time,
            duration: result.duration(),
            tasks: result.tasks.iter().map(|t| self.format_task_result(t)).collect(),
            summary: self.create_summary(&result),
        };
        
        serde_json::to_string_pretty(&output)
            .map_err(|e| OutputError::SerializationError(e.to_string()))
    }
    
    fn format_task_result(&self, task: &TaskResult) -> TaskOutput {
        TaskOutput {
            task_id: task.task_id.clone(),
            task_type: task.task_type.to_string(),
            status: task.status.clone(),
            duration: task.duration,
            output: task.output.clone(),
            error: task.error.clone(),
            metadata: task.metadata.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct WorkflowOutput {
    pub workflow_name: String,
    pub status: WorkflowStatus,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration: Option<Duration>,
    pub tasks: Vec<TaskOutput>,
    pub summary: WorkflowSummary,
}
```

#### `handler.rs` - Output Destination Handler
```rust
pub struct OutputHandler {
    file_handler: FileOutputHandler,
    s3_handler: S3OutputHandler,
}

impl OutputHandler {
    pub async fn write_output(&self, output: &str, destination: &str) -> Result<()> {
        let url = Url::parse(destination)?;
        
        match url.scheme() {
            "file" => {
                let path = url.to_file_path()
                    .map_err(|_| OutputError::InvalidDestination(destination.to_string()))?;
                self.file_handler.write(output, &path).await
            },
            "s3" => {
                let bucket = url.host_str()
                    .ok_or_else(|| OutputError::InvalidDestination(destination.to_string()))?;
                let key = url.path().trim_start_matches('/');
                self.s3_handler.write(output, bucket, key).await
            },
            _ => Err(OutputError::UnsupportedScheme(url.scheme().to_string()))
        }
    }
}
```

---

## 7. Reporting Engine Module (`src/reporting/`)

### Purpose
Reads workflow results from various sources and generates status reports via email, Slack, or terminal.

### Dependencies
```toml
lettre = "0.10"
reqwest = { version = "0.11", features = ["json"] }
aws-sdk-s3 = "0.28"
serde_json = "1.0"
handlebars = "4.4"
```

### File Structure
```
src/reporting/
├── mod.rs           # Module exports  
├── engine.rs        # Main reporting engine
├── readers/         # Input source readers
│   ├── file.rs      # File source reader
│   └── s3.rs        # S3 source reader
├── generators/      # Report generators
│   ├── email.rs     # Email report generation
│   ├── slack.rs     # Slack report generation
│   └── terminal.rs  # Terminal report generation
├── templates/       # Report templates
│   ├── email.hbs    # Email HTML template
│   ├── slack.json   # Slack message template
│   └── terminal.txt # Terminal output template
└── models.rs        # Reporting data structures
```

### Key Components

#### `engine.rs` - Main Reporting Engine
```rust
pub struct ReportingEngine {
    readers: HashMap<String, Box<dyn ResultReader>>,
    generators: HashMap<String, Box<dyn ReportGenerator>>,
    template_engine: Handlebars<'static>,
}

impl ReportingEngine {
    pub async fn generate_report(&self, config: &ReportConfig) -> Result<()> {
        // Read workflow results from sources
        let mut results = Vec::new();
        for source in &config.sources {
            let reader = self.get_reader(&source.scheme())?;
            let source_results = reader.read_results(source).await?;
            results.extend(source_results);
        }
        
        // Generate aggregate report
        let report_data = self.aggregate_results(results)?;
        
        // Send reports to configured destinations
        for destination in &config.destinations {
            let generator = self.get_generator(&destination.destination_type)?;
            generator.generate_and_send(&report_data, destination).await?;
        }
        
        Ok(())
    }
    
    fn aggregate_results(&self, results: Vec<WorkflowResult>) -> Result<AggregateReport> {
        let mut report = AggregateReport::new();
        
        for result in results {
            report.add_workflow_result(result);
        }
        
        report.calculate_statistics();
        Ok(report)
    }
}
```

#### `generators/email.rs` - Email Report Generation
```rust
pub struct EmailReportGenerator {
    smtp_client: Option<SmtpTransport>,
    ses_client: Option<aws_sdk_ses::Client>,
}

#[async_trait]
impl ReportGenerator for EmailReportGenerator {
    async fn generate_and_send(&self, report: &AggregateReport, destination: &ReportDestination) -> Result<()> {
        let email_config = destination.config.as_email()?;
        
        // Render email template
        let html_content = self.render_email_template(report)?;
        let text_content = self.render_text_template(report)?;
        
        // Create email message
        let email = Message::builder()
            .from(email_config.from.parse()?)
            .to(email_config.to.parse()?)
            .subject(&email_config.subject)
            .multipart(
                MultiPart::alternative()
                    .singlepart(SinglePart::plain(text_content))
                    .singlepart(SinglePart::html(html_content))
            )?;
        
        // Send via configured method
        match (&self.smtp_client, &self.ses_client) {
            (Some(smtp), _) => smtp.send(&email)?,
            (_, Some(ses)) => self.send_via_ses(email, ses).await?,
            _ => return Err(ReportError::NoEmailTransport),
        }
        
        Ok(())
    }
}
```

---

## Implementation Phases and Dependencies

### Phase 1: Foundation (Modules 1-2)
1. **CLI Application** - Basic structure and argument parsing
2. **Workflow Parser** - YAML parsing and validation

**Dependencies**: cli → parser

### Phase 2: Core Engine (Modules 3-4)  
3. **Template Engine** - Template resolution and helper functions
4. **Task Engine** - Execution framework and dependency resolution

**Dependencies**: engine → (parser, template)

### Phase 3: Task Implementations (Module 5)
5. **Native Tasks** - All task type implementations

**Dependencies**: tasks → (engine, template)

### Phase 4: Output & Reporting (Modules 6-7)
6. **Output Handler** - Result formatting and destination handling  
7. **Reporting Engine** - Report generation and delivery

**Dependencies**: 
- output → engine
- reporting → (output, tasks)

### Integration Dependencies
```
cli → (parser, template, engine, tasks, output, reporting)
engine → (parser, template, tasks, output)
tasks → (engine, template)
reporting → (output, readers)
```

---

## Testing Strategy

Each module should include:

1. **Unit Tests** - Test individual functions and components
2. **Integration Tests** - Test module interactions
3. **Example Fixtures** - Sample workflows and configurations
4. **Error Case Tests** - Validate error handling
5. **Performance Tests** - Benchmark critical paths

### Test Structure
```
tests/
├── unit/            # Unit tests for each module
├── integration/     # Cross-module integration tests  
├── fixtures/        # Test workflows and data
├── examples/        # Example workflows
└── benchmarks/      # Performance tests
```

This implementation plan provides a comprehensive roadmap for building each module of the wayfinder workflow engine, with clear dependencies, interfaces, and testing strategies.