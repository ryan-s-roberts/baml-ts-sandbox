//! Trait definitions for runtime components
//!
//! This module provides trait-based abstractions for runtime operations,
//! enabling better testability and flexibility.
//!
//! These traits define the core interfaces for:
//! - Executing BAML functions
//! - Loading BAML schemas
//! - Managing tool registries

use crate::error::Result;
use crate::tools::BamlTool;
use serde_json::Value;
use async_trait::async_trait;

/// Trait for executing BAML functions
#[async_trait]
pub trait BamlFunctionExecutor: Send + Sync {
    /// Execute a BAML function by name with given arguments
    async fn execute_function(
        &self,
        function_name: &str,
        args: Value,
    ) -> Result<Value>;

    /// List all available function names
    fn list_functions(&self) -> Vec<String>;
}

/// Trait for schema loading operations
pub trait SchemaLoader: Send + Sync {
    /// Load a BAML schema from the given path
    fn load_schema(&mut self, schema_path: &str) -> Result<()>;
    
    /// Check if a schema is loaded
    fn is_schema_loaded(&self) -> bool;
}

/// Trait for tool registry operations
#[async_trait]
pub trait ToolRegistryTrait: Send + Sync {
    /// Register a tool
    async fn register_tool<T: BamlTool>(&mut self, tool: T) -> Result<()>;
    
    /// Execute a tool by name
    async fn execute_tool(&self, name: &str, args: Value) -> Result<Value>;
    
    /// List all registered tool names
    async fn list_tools(&self) -> Vec<String>;
}

