// ABOUTME: Dependency graph management and execution planning
// ABOUTME: Handles topological sorting and parallel execution planning for workflow tasks

use petgraph::algo::toposort;
use petgraph::graph::NodeIndex;
use petgraph::{Direction, Graph};
use std::collections::{HashMap, HashSet, VecDeque};

use super::error::{ExecutionError, Result};
use crate::parser::Workflow;

pub struct DependencyGraph {
    graph: Graph<String, ()>,
    task_indices: HashMap<String, NodeIndex>,
}

pub struct ExecutionPlan {
    pub batches: Vec<Vec<String>>,
    pub total_tasks: usize,
}

impl DependencyGraph {
    /// Create a dependency graph from a workflow
    pub fn from_workflow(workflow: &Workflow) -> Result<Self> {
        let mut graph = Graph::new();
        let mut task_indices = HashMap::new();

        // Add all tasks as nodes
        for task_id in workflow.tasks.keys() {
            let node_index = graph.add_node(task_id.clone());
            task_indices.insert(task_id.clone(), node_index);
        }

        // Add dependency edges
        for (task_id, task_config) in &workflow.tasks {
            let task_node = task_indices[task_id];

            for dependency in &task_config.depends_on {
                if let Some(&dep_node) = task_indices.get(dependency) {
                    // Add edge from dependency to task (dependency -> task)
                    graph.add_edge(dep_node, task_node, ());
                } else {
                    return Err(ExecutionError::DependencyError {
                        message: format!(
                            "Task '{}' depends on unknown task '{}'",
                            task_id, dependency
                        ),
                    });
                }
            }
        }

        Ok(Self {
            graph,
            task_indices,
        })
    }

    /// Create an execution plan with batched parallel execution
    pub fn create_execution_plan(&self) -> Result<ExecutionPlan> {
        // Perform topological sort to detect cycles
        let sorted_nodes =
            toposort(&self.graph, None).map_err(|cycle| ExecutionError::CircularDependency {
                tasks: vec![self.graph[cycle.node_id()].clone()],
            })?;

        // Create batches for parallel execution
        let batches = self.create_execution_batches(sorted_nodes);
        let total_tasks = self.task_indices.len();

        Ok(ExecutionPlan {
            batches,
            total_tasks,
        })
    }

    /// Create batches of tasks that can be executed in parallel
    fn create_execution_batches(&self, sorted_nodes: Vec<NodeIndex>) -> Vec<Vec<String>> {
        let mut batches = Vec::new();
        let mut completed_tasks = HashSet::new();
        let mut remaining_tasks: HashSet<NodeIndex> = sorted_nodes.into_iter().collect();

        while !remaining_tasks.is_empty() {
            let mut current_batch = Vec::new();
            let mut batch_nodes = Vec::new();

            // Find tasks that can be executed (all dependencies completed)
            for &node_idx in &remaining_tasks {
                let task_id = &self.graph[node_idx];

                // Check if all dependencies are completed
                let dependencies_met = self
                    .graph
                    .neighbors_directed(node_idx, Direction::Incoming)
                    .all(|dep_node| completed_tasks.contains(&dep_node));

                if dependencies_met {
                    current_batch.push(task_id.clone());
                    batch_nodes.push(node_idx);
                }
            }

            if current_batch.is_empty() {
                // This shouldn't happen if topological sort succeeded
                break;
            }

            // Remove processed nodes from remaining tasks
            for node_idx in &batch_nodes {
                remaining_tasks.remove(node_idx);
                completed_tasks.insert(*node_idx);
            }

            batches.push(current_batch);
        }

        batches
    }

    /// Get all tasks that depend on the given task
    pub fn get_dependents(&self, task_id: &str) -> Vec<String> {
        if let Some(&node_idx) = self.task_indices.get(task_id) {
            self.graph
                .neighbors_directed(node_idx, Direction::Outgoing)
                .map(|dependent_node| self.graph[dependent_node].clone())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get all tasks that the given task depends on
    pub fn get_dependencies(&self, task_id: &str) -> Vec<String> {
        if let Some(&node_idx) = self.task_indices.get(task_id) {
            self.graph
                .neighbors_directed(node_idx, Direction::Incoming)
                .map(|dep_node| self.graph[dep_node].clone())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Check if task A depends on task B (directly or indirectly)
    pub fn has_dependency_path(&self, from_task: &str, to_task: &str) -> bool {
        if let (Some(&from_node), Some(&to_node)) = (
            self.task_indices.get(from_task),
            self.task_indices.get(to_task),
        ) {
            // Use BFS to find if there's a path from to_node to from_node
            let mut visited = HashSet::new();
            let mut queue = VecDeque::new();
            queue.push_back(to_node);
            visited.insert(to_node);

            while let Some(current) = queue.pop_front() {
                if current == from_node {
                    return true;
                }

                for neighbor in self.graph.neighbors_directed(current, Direction::Outgoing) {
                    if !visited.contains(&neighbor) {
                        visited.insert(neighbor);
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        false
    }

    /// Get tasks that have no dependencies (root tasks)
    pub fn get_root_tasks(&self) -> Vec<String> {
        self.task_indices
            .iter()
            .filter_map(|(task_id, &node_idx)| {
                let has_dependencies = self
                    .graph
                    .neighbors_directed(node_idx, Direction::Incoming)
                    .next()
                    .is_some();

                if !has_dependencies {
                    Some(task_id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get tasks that no other tasks depend on (leaf tasks)
    pub fn get_leaf_tasks(&self) -> Vec<String> {
        self.task_indices
            .iter()
            .filter_map(|(task_id, &node_idx)| {
                let has_dependents = self
                    .graph
                    .neighbors_directed(node_idx, Direction::Outgoing)
                    .next()
                    .is_some();

                if !has_dependents {
                    Some(task_id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Validate the dependency graph for common issues
    pub fn validate(&self) -> Result<()> {
        // Check for self-dependencies
        for (task_id, &node_idx) in &self.task_indices {
            if self
                .graph
                .neighbors_directed(node_idx, Direction::Incoming)
                .any(|dep_node| dep_node == node_idx)
            {
                return Err(ExecutionError::DependencyError {
                    message: format!("Task '{}' depends on itself", task_id),
                });
            }
        }

        // Check for unreachable tasks
        let root_tasks = self.get_root_tasks();
        if root_tasks.is_empty() && !self.task_indices.is_empty() {
            return Err(ExecutionError::DependencyError {
                message: "No root tasks found - all tasks have dependencies".to_string(),
            });
        }

        Ok(())
    }
}

impl ExecutionPlan {
    /// Get the maximum parallelism level (largest batch size)
    pub fn max_parallelism(&self) -> usize {
        self.batches
            .iter()
            .map(|batch| batch.len())
            .max()
            .unwrap_or(0)
    }

    /// Get the total number of execution phases
    pub fn execution_depth(&self) -> usize {
        self.batches.len()
    }

    /// Check if a task is in the execution plan
    pub fn contains_task(&self, task_id: &str) -> bool {
        self.batches
            .iter()
            .any(|batch| batch.contains(&task_id.to_string()))
    }

    /// Get the batch index for a specific task
    pub fn get_task_batch_index(&self, task_id: &str) -> Option<usize> {
        for (batch_idx, batch) in self.batches.iter().enumerate() {
            if batch.contains(&task_id.to_string()) {
                return Some(batch_idx);
            }
        }
        None
    }

    /// Get all tasks that will execute before the given task
    pub fn get_predecessors(&self, task_id: &str) -> Vec<String> {
        if let Some(batch_idx) = self.get_task_batch_index(task_id) {
            self.batches
                .iter()
                .take(batch_idx)
                .flatten()
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get all tasks that will execute after the given task
    pub fn get_successors(&self, task_id: &str) -> Vec<String> {
        if let Some(batch_idx) = self.get_task_batch_index(task_id) {
            self.batches
                .iter()
                .skip(batch_idx + 1)
                .flatten()
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::task::TaskConfig;
    use crate::parser::{TaskType, Workflow};
    use indexmap::IndexMap;

    fn create_test_workflow() -> Workflow {
        let mut tasks = IndexMap::new();

        // Task A (no dependencies)
        let task_a = TaskConfig {
            name: Some("task_a".to_string()),
            description: None,
            task_type: TaskType::Command,
            depends_on: vec![],
            required: true,
            retry_config: None,
            when: None,
            timeout: None,
            config: serde_yaml::Value::Null,
        };
        tasks.insert("task_a".to_string(), task_a);

        // Task B (depends on A)
        let task_b = TaskConfig {
            name: Some("task_b".to_string()),
            description: None,
            task_type: TaskType::Command,
            depends_on: vec!["task_a".to_string()],
            required: true,
            retry_config: None,
            when: None,
            timeout: None,
            config: serde_yaml::Value::Null,
        };
        tasks.insert("task_b".to_string(), task_b);

        // Task C (depends on A)
        let task_c = TaskConfig {
            name: Some("task_c".to_string()),
            description: None,
            task_type: TaskType::Command,
            depends_on: vec!["task_a".to_string()],
            required: true,
            retry_config: None,
            when: None,
            timeout: None,
            config: serde_yaml::Value::Null,
        };
        tasks.insert("task_c".to_string(), task_c);

        // Task D (depends on B and C)
        let task_d = TaskConfig {
            name: Some("task_d".to_string()),
            description: None,
            task_type: TaskType::Command,
            depends_on: vec!["task_b".to_string(), "task_c".to_string()],
            required: true,
            retry_config: None,
            when: None,
            timeout: None,
            config: serde_yaml::Value::Null,
        };
        tasks.insert("task_d".to_string(), task_d);

        Workflow {
            name: "test_workflow".to_string(),
            description: None,
            version: "1.0".to_string(),
            author: None,
            variables: std::collections::HashMap::new(),
            tasks,
            output: crate::parser::OutputConfig::default(),
            on_error: None,
        }
    }

    #[test]
    fn test_dependency_graph_creation() {
        let workflow = create_test_workflow();
        let graph = DependencyGraph::from_workflow(&workflow).unwrap();

        assert_eq!(graph.task_indices.len(), 4);
        assert!(graph.task_indices.contains_key("task_a"));
        assert!(graph.task_indices.contains_key("task_d"));
    }

    #[test]
    fn test_execution_plan_creation() {
        let workflow = create_test_workflow();
        let graph = DependencyGraph::from_workflow(&workflow).unwrap();
        let plan = graph.create_execution_plan().unwrap();

        assert_eq!(plan.total_tasks, 4);
        assert_eq!(plan.batches.len(), 3); // 3 execution phases

        // First batch should contain only task_a
        assert_eq!(plan.batches[0], vec!["task_a"]);

        // Second batch should contain task_b and task_c (parallel)
        assert_eq!(plan.batches[1].len(), 2);
        assert!(plan.batches[1].contains(&"task_b".to_string()));
        assert!(plan.batches[1].contains(&"task_c".to_string()));

        // Third batch should contain only task_d
        assert_eq!(plan.batches[2], vec!["task_d"]);
    }

    #[test]
    fn test_dependency_queries() {
        let workflow = create_test_workflow();
        let graph = DependencyGraph::from_workflow(&workflow).unwrap();

        // Test dependencies
        assert_eq!(graph.get_dependencies("task_a"), Vec::<String>::new());
        assert_eq!(graph.get_dependencies("task_b"), vec!["task_a"]);
        assert_eq!(graph.get_dependencies("task_d").len(), 2);

        // Test dependents
        assert_eq!(graph.get_dependents("task_d"), Vec::<String>::new());
        assert_eq!(graph.get_dependents("task_a").len(), 2); // task_b and task_c

        // Test root and leaf tasks
        assert_eq!(graph.get_root_tasks(), vec!["task_a"]);
        assert_eq!(graph.get_leaf_tasks(), vec!["task_d"]);
    }

    #[test]
    fn test_circular_dependency_detection() {
        let mut tasks = IndexMap::new();

        let task_a = TaskConfig {
            name: Some("task_a".to_string()),
            description: None,
            task_type: TaskType::Command,
            depends_on: vec!["task_b".to_string()],
            required: true,
            retry_config: None,
            when: None,
            timeout: None,
            config: serde_yaml::Value::Null,
        };
        tasks.insert("task_a".to_string(), task_a);

        let task_b = TaskConfig {
            name: Some("task_b".to_string()),
            description: None,
            task_type: TaskType::Command,
            depends_on: vec!["task_a".to_string()],
            required: true,
            retry_config: None,
            when: None,
            timeout: None,
            config: serde_yaml::Value::Null,
        };
        tasks.insert("task_b".to_string(), task_b);

        let workflow = Workflow {
            name: "circular_test".to_string(),
            description: None,
            version: "1.0".to_string(),
            author: None,
            variables: std::collections::HashMap::new(),
            tasks,
            output: crate::parser::OutputConfig::default(),
            on_error: None,
        };

        let graph = DependencyGraph::from_workflow(&workflow).unwrap();
        let result = graph.create_execution_plan();

        assert!(result.is_err());
        assert!(matches!(
            result.err().unwrap(),
            ExecutionError::CircularDependency { .. }
        ));
    }

    #[test]
    fn test_execution_plan_queries() {
        let workflow = create_test_workflow();
        let graph = DependencyGraph::from_workflow(&workflow).unwrap();
        let plan = graph.create_execution_plan().unwrap();

        assert_eq!(plan.max_parallelism(), 2); // task_b and task_c can run in parallel
        assert_eq!(plan.execution_depth(), 3);

        assert!(plan.contains_task("task_a"));
        assert!(!plan.contains_task("task_x"));

        assert_eq!(plan.get_task_batch_index("task_a"), Some(0));
        assert_eq!(plan.get_task_batch_index("task_d"), Some(2));

        let predecessors = plan.get_predecessors("task_d");
        assert_eq!(predecessors.len(), 3); // task_a, task_b, task_c
    }
}
