//! Tool function registration system
//!
//! This module provides a trait-based system for registering tool functions
//! that can be called by LLMs during BAML function execution or directly from JavaScript.

use baml_rt_core::{BamlRtError, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;

/// Trait for BAML tools that can be called by LLMs or JavaScript
///
/// Tools implement this trait to provide:
/// - Name and metadata
/// - Input schema for LLM understanding
/// - Execution logic
///
/// # Example
/// ```rust,no_run
/// use baml_rt::tools::BamlTool;
/// use serde_json::{json, Value};
/// use async_trait::async_trait;
///
/// struct WeatherTool;
///
/// #[async_trait]
/// impl BamlTool for WeatherTool {
///     const NAME: &'static str = "get_weather";
///
///     fn description(&self) -> &'static str {
///         "Gets the current weather for a specific location"
///     }
///
///     fn input_schema(&self) -> Value {
///         json!({
///             "type": "object",
///             "properties": {
///                 "location": {"type": "string", "description": "Location to get weather for"}
///             },
///             "required": ["location"]
///         })
///     }
///
///     async fn execute(&self, args: Value) -> baml_rt::Result<Value> {
///         let obj = args.as_object().expect("Expected object");
///         let location = obj.get("location").and_then(|v| v.as_str()).unwrap();
///         Ok(json!({"temperature": "22°C", "location": location}))
///     }
/// }
/// ```
#[async_trait]
pub trait BamlTool: Send + Sync + 'static {
    /// The unique name of this tool
    const NAME: &'static str;

    /// Description of what this tool does (used by LLMs to understand when to call it)
    fn description(&self) -> &'static str;

    /// JSON schema describing the tool's input parameters
    fn input_schema(&self) -> Value;

    /// Execute the tool with the given arguments
    ///
    /// # Arguments
    /// * `args` - JSON object containing the tool's input parameters
    ///
    /// # Returns
    /// JSON value representing the tool's output
    async fn execute(&self, args: Value) -> Result<Value>;
}

/// Metadata describing a tool function
#[derive(Debug, Clone)]
pub struct ToolMetadata {
    /// Tool name (must be unique)
    pub name: String,
    /// Tool description (used by LLMs to understand what the tool does)
    pub description: String,
    /// JSON schema for the tool's input parameters
    pub input_schema: Value,
}

/// Registry for dynamically registered tool functions
pub struct ToolRegistry {
    tools: HashMap<String, (ToolMetadata, Arc<dyn ToolExecutor>)>,
}

/// Internal trait for executing tools (bridges trait objects to async trait)
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, args: Value) -> Result<Value>;
}

/// Wrapper that implements ToolExecutor for any BamlTool
struct ToolWrapper<T: BamlTool> {
    tool: T,
}

#[async_trait]
impl<T: BamlTool> ToolExecutor for ToolWrapper<T> {
    async fn execute(&self, args: Value) -> Result<Value> {
        self.tool.execute(args).await
    }
}

impl ToolRegistry {
    /// Create a new empty tool registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool that implements the BamlTool trait
    ///
    /// # Arguments
    /// * `tool` - An instance of a type implementing `BamlTool`
    ///
    /// # Example
    /// ```rust,no_run
    /// use baml_rt::tools::{ToolRegistry, BamlTool};
    /// use serde_json::json;
    /// use async_trait::async_trait;
    ///
    /// struct MyTool;
    ///
    /// #[async_trait]
    /// impl BamlTool for MyTool {
    ///     const NAME: &'static str = "my_tool";
    ///     fn description(&self) -> &'static str { "My tool" }
    ///     fn input_schema(&self) -> serde_json::Value { json!({}) }
    ///     async fn execute(&self, _args: serde_json::Value) -> baml_rt::Result<serde_json::Value> {
    ///         Ok(json!({}))
    ///     }
    /// }
    ///
    /// let mut registry = ToolRegistry::new();
    /// registry.register(MyTool).expect("register tool");
    /// ```
    pub fn register<T: BamlTool>(&mut self, tool: T) -> Result<()> {
        let name = T::NAME.to_string();

        if self.tools.contains_key(&name) {
            return Err(BamlRtError::InvalidArgument(format!(
                "Tool '{}' is already registered",
                name
            )));
        }

        let description_str = tool.description().to_string();
        let metadata = ToolMetadata {
            name: name.clone(),
            description: description_str.clone(),
            input_schema: tool.input_schema(),
        };

        let tool_executor: Arc<dyn ToolExecutor> = Arc::new(ToolWrapper { tool });

        self.tools.insert(name.clone(), (metadata, tool_executor));

        tracing::info!(
            tool = name.as_str(),
            description = description_str.as_str(),
            "Registered tool function"
        );

        Ok(())
    }

    /// Register a tool with dynamic metadata and executor.
    pub fn register_dynamic(
        &mut self,
        metadata: ToolMetadata,
        executor: Arc<dyn ToolExecutor>,
    ) -> Result<()> {
        if self.tools.contains_key(&metadata.name) {
            return Err(BamlRtError::InvalidArgument(format!(
                "Tool '{}' is already registered",
                metadata.name
            )));
        }

        tracing::info!(
            tool = metadata.name.as_str(),
            description = metadata.description.as_str(),
            "Registered dynamic tool function"
        );

        self.tools
            .insert(metadata.name.clone(), (metadata, executor));

        Ok(())
    }

    /// Get tool metadata by name
    pub fn get_metadata(&self, name: &str) -> Option<&ToolMetadata> {
        self.tools.get(name).map(|(metadata, _)| metadata)
    }

    /// List all registered tool names
    pub fn list_tools(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Get all tool metadata (for LLM function calling)
    pub fn all_metadata(&self) -> Vec<&ToolMetadata> {
        self.tools.values().map(|(metadata, _)| metadata).collect()
    }

    /// Execute a tool function by name
    pub async fn execute(&self, name: &str, args: Value) -> Result<Value> {
        let (_, tool_executor) = self.tools.get(name)
            .ok_or_else(|| BamlRtError::FunctionNotFound(format!("Tool '{}' not found", name)))?;

        tracing::debug!(
            tool = name,
            args = ?args,
            "Executing tool function"
        );

        tool_executor.execute(args).await
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
