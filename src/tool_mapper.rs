//! Tool mapper - maps BAML union types to registered Rust tool functions
//!
//! When BAML returns a union type representing a tool choice, this module
//! maps it to the corresponding Rust tool function and executes it.

use crate::error::{BamlRtError, Result};
use crate::tools::ToolRegistry;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Maps BAML union type names to Rust tool function names
pub struct ToolMapper {
    /// Mapping from BAML class/union variant name to tool function name
    /// e.g., "WeatherTool" -> "get_weather"
    variant_to_tool: HashMap<String, String>,
}

impl ToolMapper {
    /// Create a new tool mapper
    pub fn new() -> Self {
        Self {
            variant_to_tool: HashMap::new(),
        }
    }

    /// Register a mapping from a BAML union variant to a tool function
    /// 
    /// # Arguments
    /// * `baml_variant_name` - The name of the BAML class/union variant (e.g., "WeatherTool")
    /// * `tool_function_name` - The name of the registered Rust tool function (e.g., "get_weather")
    /// 
    /// # Example
    /// ```rust,no_run
    /// let mut mapper = ToolMapper::new();
    /// mapper.register_mapping("WeatherTool", "get_weather");
    /// mapper.register_mapping("CalculatorTool", "calculate");
    /// ```
    pub fn register_mapping(
        &mut self,
        baml_variant_name: impl Into<String>,
        tool_function_name: impl Into<String>,
    ) {
        let variant = baml_variant_name.into();
        let tool = tool_function_name.into();
        
        tracing::debug!(
            baml_variant = variant.as_str(),
            tool_function = tool.as_str(),
            "Registered tool mapping"
        );
        
        self.variant_to_tool.insert(variant, tool);
    }

    /// Execute a tool based on BAML union type result
    /// 
    /// This extracts the union variant type, maps it to a tool function,
    /// and executes that tool with the parameters from the BAML result.
    /// 
    /// BAML union types can be serialized in different ways:
    /// 1. Direct class fields (e.g., {"location": "SF"})
    /// 2. With __type field (e.g., {"__type": "WeatherTool", "location": "SF"})
    /// 3. Nested with variant name as key (e.g., {"WeatherTool": {"location": "SF"}})
    /// 
    /// # Arguments
    /// * `baml_result` - The JSON result from BAML (should be a class/union variant)
    /// * `tool_registry` - The tool registry containing registered tools
    /// 
    /// # Returns
    /// The result of executing the tool function
    pub async fn execute_from_baml_result(
        &self,
        baml_result: Value,
        tool_registry: &Arc<Mutex<ToolRegistry>>,
    ) -> Result<Value> {
        tracing::debug!(baml_result = ?baml_result, "Parsing BAML union result for tool execution");
        
        // Handle nested structure: {"WeatherTool": {...}}
        let (variant_name, tool_obj_value) = if let Some(obj) = baml_result.as_object() {
            // Check if it's a single-key object where the key is a variant name
            if obj.len() == 1 {
                let (key, value) = obj.iter().next()
                    .ok_or_else(|| BamlRtError::InvalidArgument(
                        "Expected non-empty object with tool variant".to_string()
                    ))?;
                // Check if this key matches a known variant
                if self.variant_to_tool.contains_key(key) {
                    (key.clone(), value.clone())
                } else {
                    // Not a nested structure, treat as direct class
                    (String::new(), baml_result.clone())
                }
            } else {
                // Multiple keys or empty, treat as direct class
                (String::new(), baml_result.clone())
            }
        } else {
            return Err(BamlRtError::InvalidArgument(
                "Expected BAML result to be an object representing a tool choice".to_string()
            ));
        };

        let tool_obj = tool_obj_value.as_object()
            .ok_or_else(|| BamlRtError::InvalidArgument(
                "Expected BAML result to be an object representing a tool choice".to_string()
            ))?;

        // Determine variant name
        let variant_name = if !variant_name.is_empty() {
            variant_name
        } else {
            // Try to get the type from __type field (how BAML serializes classes)
            tool_obj.get("__type")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| {
                    // Try to infer from field names (heuristic)
                    // If it has "location", probably WeatherTool
                    // If it has "expression", probably CalculatorTool
                    if tool_obj.contains_key("location") {
                        Some("WeatherTool".to_string())
                    } else if tool_obj.contains_key("expression") {
                        Some("CalculatorTool".to_string())
                    } else {
                        None
                    }
                })
                .ok_or_else(|| BamlRtError::InvalidArgument(
                    format!("Could not determine tool type from BAML result. Expected '__type' field, nested structure like {{'WeatherTool': {{...}}}}, or recognizable fields. Got keys: {:?}", 
                        tool_obj.keys().collect::<Vec<_>>())
                ))?
        };

        tracing::debug!(
            variant = variant_name.as_str(),
            "Detected BAML union variant"
        );

        // Map variant name to tool function name
        let tool_function_name = self.variant_to_tool.get(&variant_name)
            .ok_or_else(|| BamlRtError::FunctionNotFound(
                format!("No tool mapping found for BAML variant '{}'. Registered mappings: {:?}", 
                    variant_name, 
                    self.variant_to_tool.keys().collect::<Vec<_>>())
            ))?;

        tracing::info!(
            variant = variant_name.as_str(),
            tool = tool_function_name.as_str(),
            "Executing tool from BAML union variant"
        );

        // Extract tool arguments from the BAML result
        // The BAML class fields become the tool arguments
        let mut tool_args = serde_json::Map::new();
        
        for (key, value) in tool_obj {
            // Skip metadata fields
            if key != "__type" {
                tool_args.insert(key.clone(), value.clone());
            }
        }

        let tool_args_value = Value::Object(tool_args.clone());
        tracing::debug!(
            variant = variant_name.as_str(),
            tool = tool_function_name.as_str(),
            tool_args = ?tool_args_value,
            "Extracted tool arguments"
        );

        // Execute the tool
        let registry = tool_registry.lock().await;
        let tool_result = registry.execute(tool_function_name, Value::Object(tool_args)).await?;

        tracing::debug!(
            variant = variant_name.as_str(),
            tool = tool_function_name.as_str(),
            "Tool executed successfully"
        );

        Ok(tool_result)
    }

    /// List all registered mappings
    pub fn list_mappings(&self) -> Vec<(&String, &String)> {
        self.variant_to_tool.iter().collect()
    }
}

impl Default for ToolMapper {
    fn default() -> Self {
        Self::new()
    }
}

