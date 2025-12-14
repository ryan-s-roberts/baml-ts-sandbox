//! BAML function execution engine
//!
//! This module executes BAML functions using the compiled IL (Intermediate Language)
//! from the BAML compiler.

use crate::error::{BamlRtError, Result};
use baml_runtime::{BamlRuntime, FunctionResult, FunctionResultStream, RuntimeContextManager};
use baml_types::BamlValue;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// BAML execution engine that executes BAML IL
pub struct BamlExecutor {
    runtime: Arc<BamlRuntime>,
    ctx_manager: RuntimeContextManager,
    pub(crate) functions: HashMap<String, String>, // function name -> placeholder (for discovery)
}

impl BamlExecutor {
    /// Load BAML IL from the compiled output
    /// 
    /// This loads the BAML runtime from the baml_src directory using from_directory
    pub fn load_il(_baml_client_dir: &Path, baml_src_dir: &Path) -> Result<Self> {
        tracing::info!(?baml_src_dir, "Loading BAML runtime from directory");
        
        // Use from_directory which handles feature flags internally
        let env_vars: HashMap<String, String> = HashMap::new(); // TODO: Load from environment
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
        })
    }

    /// Execute a BAML function using the compiled IL
    pub async fn execute_function(
        &self,
        function_name: &str,
        args: Value,
    ) -> Result<Value> {
        tracing::debug!(
            function = function_name,
            args = ?args,
            "Executing BAML function from IL"
        );

        // Convert JSON args to BamlValue map
        let params = self.json_to_baml_map(&args)?;
        
        // Call the function
        let env_vars = HashMap::new();
        let tags = None;
        let cancel_tripwire = baml_runtime::TripWire::new(None);
        
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
        let env_vars = HashMap::new();
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
                    Err(BamlRtError::InvalidArgument("Invalid number".to_string()))
                }
            }
            Value::Bool(b) => Ok(BamlValue::Bool(*b)),
            Value::Null => Ok(BamlValue::Null),
            Value::Array(arr) => {
                let vec: Result<Vec<BamlValue>> = arr.iter()
                    .map(|v| self.json_to_baml_value(v))
                    .collect();
                Ok(BamlValue::List(vec?))
            }
            Value::Object(obj) => {
                let mut map = baml_types::BamlMap::new();
                for (k, v) in obj {
                    map.insert(k.clone(), self.json_to_baml_value(v)?);
                }
                Ok(BamlValue::Map(map))
            }
        }
    }
    
    /// Convert BamlValue to JSON Value
    fn baml_value_to_json(&self, value: &BamlValue) -> Result<Value> {
        match value {
            BamlValue::String(s) => Ok(Value::String(s.clone())),
            BamlValue::Int(i) => Ok(Value::Number((*i).into())),
            BamlValue::Float(f) => Ok(Value::Number(
                serde_json::Number::from_f64(*f)
                    .ok_or_else(|| BamlRtError::TypeConversion("Invalid float".to_string()))?
            )),
            BamlValue::Bool(b) => Ok(Value::Bool(*b)),
            BamlValue::Null => Ok(Value::Null),
            BamlValue::List(list) => {
                let vec: Result<Vec<Value>> = list.iter()
                    .map(|v| self.baml_value_to_json(v))
                    .collect();
                Ok(Value::Array(vec?))
            }
            BamlValue::Map(map) => {
                let mut obj = serde_json::Map::new();
                for (k, v) in map.iter() {
                    obj.insert(k.clone(), self.baml_value_to_json(v)?);
                }
                Ok(Value::Object(obj))
            }
            _ => Err(BamlRtError::TypeConversion(
                format!("Unsupported BamlValue type: {:?}", value)
            )),
        }
    }

    pub fn list_functions(&self) -> Vec<String> {
        self.functions.keys().cloned().collect()
    }
}
