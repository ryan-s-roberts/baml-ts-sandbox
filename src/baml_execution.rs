//! BAML function execution engine
//!
//! This module executes BAML functions using the compiled IL (Intermediate Language)
//! from the BAML compiler.

use crate::error::{BamlRtError, Result};
use crate::tools::ToolRegistry;
use baml_runtime::{BamlRuntime, FunctionResultStream, RuntimeContextManager};
use baml_types::BamlValue;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

/// BAML execution engine that executes BAML IL
pub struct BamlExecutor {
    runtime: Arc<BamlRuntime>,
    ctx_manager: RuntimeContextManager,
    pub(crate) functions: HashMap<String, String>, // function name -> placeholder (for discovery)
    tool_registry: Arc<Mutex<ToolRegistry>>,
}

impl BamlExecutor {
    /// Load BAML IL from the compiled output
    /// 
    /// This loads the BAML runtime from the baml_src directory using from_directory
    pub fn load_il(
        _baml_client_dir: &Path,
        baml_src_dir: &Path,
        tool_registry: Arc<Mutex<ToolRegistry>>,
    ) -> Result<Self> {
        tracing::info!(?baml_src_dir, "Loading BAML runtime from directory");
        
        // Use from_directory which handles feature flags internally
        // Load environment variables - BAML uses these for API keys
        let mut env_vars: HashMap<String, String> = HashMap::new();
        
        // Load OPENROUTER_API_KEY from environment if present
        if let Ok(api_key) = std::env::var("OPENROUTER_API_KEY") {
            env_vars.insert("OPENROUTER_API_KEY".to_string(), api_key);
            tracing::debug!("Loaded OPENROUTER_API_KEY from environment");
        }
        
        // Load other common API key environment variables
        for key in &["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "GOOGLE_API_KEY"] {
            if let Ok(value) = std::env::var(key) {
                env_vars.insert(key.to_string(), value);
                tracing::debug!(api_key = key, "Loaded API key from environment");
            }
        }
        
        let feature_flags = internal_baml_core::feature_flags::FeatureFlags::default();
        
        let runtime = BamlRuntime::from_directory(baml_src_dir, env_vars, feature_flags)
            .map_err(|e| BamlRtError::BamlRuntime(format!("Failed to load BAML runtime: {}", e)))?;
        
        // Create context manager
        let ctx_manager = runtime.create_ctx_manager(
            BamlValue::String("rust".to_string()),
            None, // baml_src_reader
        );
        
        // Function discovery will be done from the runtime's IR when needed
        let function_map = HashMap::new();
        
        Ok(Self {
            runtime: Arc::new(runtime),
            ctx_manager,
            functions: function_map,
            tool_registry,
        })
    }

    /// Execute a BAML function using the compiled IL
    pub async fn execute_function(
        &self,
        function_name: &str,
        args: Value,
        tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
    ) -> Result<Value> {
        tracing::debug!(
            function = function_name,
            args = ?args,
            "Executing BAML function from IL"
        );

        // Convert JSON args to BamlValue map
        let params = self.json_to_baml_map(&args)?;
        
        // Call the function
        // Load environment variables for API keys
        let mut env_vars = HashMap::new();
        if let Ok(api_key) = std::env::var("OPENROUTER_API_KEY") {
            env_vars.insert("OPENROUTER_API_KEY".to_string(), api_key);
        }
        for key in &["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "GOOGLE_API_KEY"] {
            if let Ok(value) = std::env::var(key) {
                env_vars.insert(key.to_string(), value);
            }
        }
        let tags = None;
        let cancel_tripwire = baml_runtime::TripWire::new(None);
        
        // TODO: Integrate tool registry with BAML's function calling mechanism
        // For now, tools are registered but not passed to the LLM
        // This requires understanding BAML's native tool calling support
        let (result, _call_id) = self.runtime.call_function(
            function_name.to_string(),
            &params,
            &self.ctx_manager,
            None, // type_builder
            None, // client_registry
            None, // collectors
            env_vars,
            tags,
            cancel_tripwire,
        ).await;
        
        let function_result = result
            .map_err(|e| BamlRtError::BamlRuntime(format!("Function execution failed: {}", e)))?;
        
        // Extract the parsed value
        let parsed = function_result.parsed()
            .as_ref()
            .ok_or_else(|| BamlRtError::BamlRuntime("Function returned no parsed result".to_string()))?
            .as_ref()
            .map_err(|e| BamlRtError::BamlRuntime(format!("Parsing failed: {}", e)))?;
        
        // Convert ResponseBamlValue to JSON using serialize_partial
        let json_value = serde_json::to_value(parsed.serialize_partial())
            .map_err(|e| BamlRtError::TypeConversion(format!("Failed to serialize: {}", e)))?;
        
        Ok(json_value)
    }

    /// Execute a BAML function with streaming support
    /// 
    /// Returns a stream of incremental results as the function executes.
    pub fn execute_function_stream(
        &self,
        function_name: &str,
        args: Value,
    ) -> Result<FunctionResultStream> {
        tracing::debug!(
            function = function_name,
            args = ?args,
            "Starting streaming execution of BAML function"
        );

        // Convert JSON args to BamlValue map
        let params = self.json_to_baml_map(&args)?;
        
        // Create stream function call
        // Load environment variables for API keys
        let mut env_vars = HashMap::new();
        if let Ok(api_key) = std::env::var("OPENROUTER_API_KEY") {
            env_vars.insert("OPENROUTER_API_KEY".to_string(), api_key);
        }
        for key in &["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "GOOGLE_API_KEY"] {
            if let Ok(value) = std::env::var(key) {
                env_vars.insert(key.to_string(), value);
            }
        }
        let tags = None;
        let cancel_tripwire = baml_runtime::TripWire::new(None);
        
        let stream = self.runtime.stream_function(
            function_name.to_string(),
            &params,
            &self.ctx_manager,
            None, // type_builder
            None, // client_registry
            None, // collectors
            env_vars,
            cancel_tripwire,
            tags,
        )
        .map_err(|e| BamlRtError::BamlRuntime(format!("Failed to create stream: {}", e)))?;

        Ok(stream)
    }

    /// Get a reference to the context manager (needed for streaming)
    pub fn ctx_manager(&self) -> &RuntimeContextManager {
        &self.ctx_manager
    }
    
    /// Convert JSON Value to BamlMap<String, BamlValue>
    fn json_to_baml_map(&self, value: &Value) -> Result<baml_types::BamlMap<String, BamlValue>> {
        let obj = value.as_object()
            .ok_or_else(|| BamlRtError::InvalidArgument("Expected JSON object".to_string()))?;
        
        let mut map = baml_types::BamlMap::new();
        for (k, v) in obj {
            map.insert(k.clone(), self.json_to_baml_value(v)?);
        }
        Ok(map)
    }
    
    /// Convert JSON Value to BamlValue
    fn json_to_baml_value(&self, value: &Value) -> Result<BamlValue> {
        match value {
            Value::String(s) => Ok(BamlValue::String(s.clone())),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(BamlValue::Int(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(BamlValue::Float(f))
                } else {
                    Err(BamlRtError::TypeConversion(format!("Invalid number: {}", n)))
                }
            }
            Value::Bool(b) => Ok(BamlValue::Bool(*b)),
            Value::Array(arr) => {
                let mut vec = Vec::new();
                for item in arr {
                    vec.push(self.json_to_baml_value(item)?);
                }
                Ok(BamlValue::List(vec))
            }
            Value::Object(obj) => {
                let mut map = baml_types::BamlMap::new();
                for (k, v) in obj {
                    map.insert(k.clone(), self.json_to_baml_value(v)?);
                }
                Ok(BamlValue::Map(map))
            }
            Value::Null => Ok(BamlValue::Null),
        }
    }

    #[allow(dead_code)]
    fn baml_value_to_json(&self, value: &BamlValue) -> Result<Value> {
        match value {
            BamlValue::String(s) => Ok(Value::String(s.clone())),
            BamlValue::Int(i) => Ok(Value::Number((*i).into())),
            BamlValue::Float(f) => Ok(serde_json::Number::from_f64(*f)
                .ok_or_else(|| BamlRtError::TypeConversion("Invalid float".to_string()))?
                .into()),
            BamlValue::Bool(b) => Ok(Value::Bool(*b)),
            BamlValue::List(list) => {
                let mut arr = Vec::new();
                for item in list {
                    arr.push(self.baml_value_to_json(item)?);
                }
                Ok(Value::Array(arr))
            }
            BamlValue::Map(map) => {
                let mut obj = serde_json::Map::new();
                for (k, v) in map.iter() {
                    obj.insert(k.clone(), self.baml_value_to_json(v)?);
                }
                Ok(Value::Object(obj))
            }
            BamlValue::Null => Ok(Value::Null),
            BamlValue::Media(_) => Err(BamlRtError::TypeConversion("Media not supported in JSON conversion".to_string())),
            BamlValue::Enum(name, value) => {
                // Enums are represented as objects with the enum name
                let mut obj = serde_json::Map::new();
                obj.insert("__type".to_string(), Value::String(name.clone()));
                obj.insert("value".to_string(), Value::String(value.clone()));
                Ok(Value::Object(obj))
            }
            BamlValue::Class(name, fields) => {
                // Classes are represented as objects
                let mut obj = serde_json::Map::new();
                obj.insert("__type".to_string(), Value::String(name.clone()));
                for (k, v) in fields.iter() {
                    obj.insert(k.clone(), self.baml_value_to_json(v)?);
                }
                Ok(Value::Object(obj))
            }
        }
    }
}
