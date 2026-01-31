//! BAML runtime wrapper and function execution

use crate::baml_execution::BamlExecutor;
use baml_rt_core::{BamlRtError, Result};
use baml_rt_core::types::FunctionSignature;
use baml_rt_tools::{ToolRegistry as ConcreteToolRegistry, ToolMetadata, ToolMapper};
use crate::traits::{BamlFunctionExecutor, SchemaLoader};
use baml_rt_interceptor::InterceptorRegistry;
use baml_rt_core::correlation::current_correlation_id;
use baml_rt_observability::metrics;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Mutex as TokioMutex;

// BAML executes in Rust. We will implement execution of BAML functions
// in Rust, then map those function calls to QuickJS so JavaScript can invoke them.
// use baml;

/// Manages the BAML runtime and function registry
pub struct BamlRuntimeManager {
    function_registry: HashMap<String, FunctionSignature>,
    pub(crate) executor: Option<BamlExecutor>,
    tool_registry: Arc<TokioMutex<ConcreteToolRegistry>>,
    tool_mapper: Arc<StdMutex<ToolMapper>>,
    interceptor_registry: Arc<TokioMutex<InterceptorRegistry>>,
}

impl BamlRuntimeManager {
    /// Create a new BAML runtime manager
    pub fn new() -> Result<Self> {
        tracing::info!("Initializing BAML runtime manager");

        Ok(Self {
            function_registry: HashMap::new(),
            executor: None,
            tool_registry: Arc::new(TokioMutex::new(ConcreteToolRegistry::new())),
            tool_mapper: Arc::new(StdMutex::new(ToolMapper::new())),
            interceptor_registry: Arc::new(TokioMutex::new(InterceptorRegistry::new())),
        })
    }

    /// Check if a schema is loaded
    pub fn is_schema_loaded(&self) -> bool {
        self.executor.is_some()
    }

    /// Load a compiled BAML schema/configuration
    ///
    /// This loads the BAML IL (Intermediate Language) from the baml_src directory
    /// and registers all available functions.
    ///
    /// The schema_path should point to the baml_src directory.
    pub fn load_schema(&mut self, schema_path: &str) -> Result<()> {
        tracing::info!(schema_path = schema_path, "Loading BAML IL");

        use std::path::Path;

        // Find project root
        let schema_path_obj = Path::new(schema_path);
        let project_root = if schema_path_obj.is_file() {
            schema_path_obj.parent()
                .and_then(|p| p.parent())
        } else if schema_path_obj.file_name() == Some(std::ffi::OsStr::new("baml_src")) {
            schema_path_obj.parent()
        } else {
            Some(schema_path_obj)
        }
        .ok_or_else(|| BamlRtError::InvalidArgument("Invalid schema path".to_string()))?;

        let baml_src_dir = project_root.join("baml_src");
        if !baml_src_dir.exists() {
            return Err(BamlRtError::BamlRuntime(
                "baml_src directory not found".to_string()
            ));
        }

        // Load BAML IL into executor (pass tool registry)
        let tool_registry_clone = self.tool_registry.clone();
        let tool_mapper_clone = self.tool_mapper.clone();
        let executor = BamlExecutor::load_il(&baml_src_dir, tool_registry_clone, tool_mapper_clone)?;

        // Discover functions from the BAML runtime
        let function_names = executor.list_functions();
        for func_name in function_names {
            // Register function signature
            self.function_registry.insert(
                func_name.clone(),
                FunctionSignature {
                    name: func_name.clone(),
                    input_types: vec![],
                    output_type: baml_rt_core::types::BamlType::String,
                },
            );
        }

        self.executor = Some(executor);

        tracing::info!(
            function_count = self.function_registry.len(),
            "Loaded BAML IL"
        );

        Ok(())
    }

    /// Get the signature of a function by name
    pub fn get_function_signature(&self, name: &str) -> Option<&FunctionSignature> {
        self.function_registry.get(name)
    }

    /// Execute a BAML function with the given arguments
    ///
    /// This is the main entry point for executing BAML functions.
    /// It validates the function exists and delegates to the executor.
    pub async fn invoke_function(
        &self,
        function_name: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let correlation_id = current_correlation_id();
        if let Some(correlation_id) = correlation_id.as_deref() {
            tracing::debug!(
                function = function_name,
                args = ?args,
                correlation_id = correlation_id,
                "Invoking BAML function"
            );
        } else {
            tracing::debug!(
                function = function_name,
                args = ?args,
                "Invoking BAML function"
            );
        }

        // Verify function exists
        let _signature = self
            .function_registry
            .get(function_name)
            .ok_or_else(|| BamlRtError::FunctionNotFound(function_name.to_string()))?;

        // Execute the BAML function using the executor
        let executor = self.executor.as_ref()
            .ok_or_else(|| BamlRtError::BamlRuntime("BAML runtime not loaded".to_string()))?;

        // Pass tool registry and interceptor registry to executor
        let interceptor_registry = Some(self.interceptor_registry.clone());
        executor.execute_function(function_name, args, interceptor_registry).await
    }

    /// Invoke a BAML function with streaming support
    ///
    /// Returns a stream that yields incremental results as the function executes.
    pub fn invoke_function_stream(
        &self,
        function_name: &str,
        args: serde_json::Value,
    ) -> Result<baml_runtime::FunctionResultStream> {
        tracing::debug!(
            function = function_name,
            args = ?args,
            "Invoking BAML function with streaming"
        );

        // Verify function exists
        let _signature = self
            .function_registry
            .get(function_name)
            .ok_or_else(|| BamlRtError::FunctionNotFound(function_name.to_string()))?;

        // Execute the BAML function using the executor
        let executor = self.executor.as_ref()
            .ok_or_else(|| BamlRtError::BamlRuntime("BAML runtime not loaded".to_string()))?;

        executor.execute_function_stream(function_name, args)
    }

    /// List all available BAML functions
    pub fn list_functions(&self) -> Vec<String> {
        self.function_registry.keys().cloned().collect()
    }

    /// Get the tool registry (for tool registration)
    pub fn tool_registry(&self) -> Arc<TokioMutex<ConcreteToolRegistry>> {
        self.tool_registry.clone()
    }

    /// Get the interceptor registry (for registering interceptors)
    pub fn interceptor_registry(&self) -> Arc<TokioMutex<InterceptorRegistry>> {
        self.interceptor_registry.clone()
    }

    /// Register an LLM interceptor
    pub async fn register_llm_interceptor<I: baml_rt_interceptor::LLMInterceptor>(&self, interceptor: I) {
        let mut registry = self.interceptor_registry.lock().await;
        registry.register_llm_interceptor(interceptor);
    }

    /// Register a tool interceptor
    pub async fn register_tool_interceptor<I: baml_rt_interceptor::ToolInterceptor>(&self, interceptor: I) {
        let mut registry = self.interceptor_registry.lock().await;
        registry.register_tool_interceptor(interceptor);
    }

    /// Register a tool that implements the BamlTool trait
    ///
    /// Tools can be called by LLMs during BAML function execution
    /// or directly from JavaScript via the QuickJS bridge.
    ///
    /// # Example
    /// ```rust,no_run
    /// use baml_rt::baml::BamlRuntimeManager;
    /// use baml_rt::tools::BamlTool;
    /// use serde_json::json;
    /// use async_trait::async_trait;
    ///
    /// struct MyTool;
    ///
    /// #[async_trait]
    /// impl BamlTool for MyTool {
    ///     const NAME: &'static str = "my_tool";
    ///     fn description(&self) -> &'static str { "Does something" }
    ///     fn input_schema(&self) -> serde_json::Value { json!({}) }
    ///     async fn execute(&self, _args: serde_json::Value) -> baml_rt::Result<serde_json::Value> {
    ///         Ok(json!({"result": "success"}))
    ///     }
    /// }
    ///
    /// # tokio_test::block_on(async {
    /// let mut manager = BamlRuntimeManager::new()?;
    /// manager.register_tool(MyTool).await?;
    /// # Ok::<(), baml_rt::BamlRtError>(())
    /// # }).unwrap();
    /// ```
    pub async fn register_tool<T: baml_rt_tools::BamlTool>(&mut self, tool: T) -> Result<()> {
        let mut registry = self.tool_registry.lock().await;
        registry.register(tool)
    }

    /// Execute a tool function by name
    ///
    /// This will call tool interceptors before and after execution.
    pub async fn execute_tool(&self, name: &str, args: Value) -> Result<Value> {
        use baml_rt_interceptor::ToolCallContext;
        use std::time::Instant;

        let start = Instant::now();
        let correlation_id = current_correlation_id();
        let metadata = if let Some(correlation_id) = correlation_id {
            json!({ "correlation_id": correlation_id })
        } else {
            json!({})
        };

        // Build context for interceptors
        let context = ToolCallContext {
            tool_name: name.to_string(),
            function_name: None, // Could be enhanced to track which function called this tool
            args: args.clone(),
            metadata,
        };

        // Run interceptors before execution
        let interceptor_registry = self.interceptor_registry.lock().await;
        let _decision = interceptor_registry.intercept_tool_call(&context).await?;
        drop(interceptor_registry);

        // Handle interceptor decision
        // If we get here, the decision is Allow (blocking would have returned Err)
        let final_args = args;

        // Execute the tool
        let registry = self.tool_registry.lock().await;
        let result = registry.execute(name, final_args).await;
        drop(registry);

        // Calculate duration
        let duration = start.elapsed();
        let duration_ms = duration.as_millis() as u64;

        // Notify interceptors of completion
        let interceptor_registry = self.interceptor_registry.lock().await;
        interceptor_registry.notify_tool_call_complete(&context, &result, duration_ms).await;
        drop(interceptor_registry);

        let metric_result = if result.is_ok() { "success" } else { "error" };
        metrics::record_tool_invocation(name, metric_result, duration);

        result
    }

    /// List all registered tools
    pub async fn list_tools(&self) -> Vec<String> {
        let registry = self.tool_registry.lock().await;
        registry.list_tools()
    }

    /// Get tool metadata
    pub async fn get_tool_metadata(&self, name: &str) -> Option<ToolMetadata> {
        let registry = self.tool_registry.lock().await;
        registry.get_metadata(name).cloned()
    }

    /// Map a BAML union variant to a tool function
    ///
    /// This connects BAML's structured output (union types) to our Rust tool registry.
    /// When BAML returns a union variant representing a tool choice, we can execute
    /// the corresponding Rust tool function.
    ///
    /// # Arguments
    /// * `baml_variant_name` - The name of the BAML class/union variant (e.g., "WeatherTool")
    /// * `tool_function_name` - The name of the registered Rust tool function (e.g., "get_weather")
    ///
    /// # Example
    /// ```rust,no_run
    /// // Register a tool
    /// # use baml_rt::baml::BamlRuntimeManager;
    /// # use baml_rt::tools::BamlTool;
    /// # struct WeatherTool;
    /// # #[async_trait::async_trait]
    /// # impl BamlTool for WeatherTool {
    /// #     const NAME: &'static str = "get_weather";
    /// #     fn description(&self) -> &'static str { "" }
    /// #     fn input_schema(&self) -> serde_json::Value { serde_json::json!({}) }
    /// #     async fn execute(&self, _: serde_json::Value) -> baml_rt::Result<serde_json::Value> { Ok(serde_json::json!({})) }
    /// # }
    /// # tokio_test::block_on(async {
    /// # let mut manager = BamlRuntimeManager::new()?;
    /// manager.register_tool(WeatherTool).await?;
    ///
    /// // Map BAML union variant to tool
    /// manager.map_baml_variant_to_tool("WeatherTool", "get_weather");
    ///
    /// // When BAML returns a WeatherTool variant, it will automatically map to get_weather
    /// # Ok::<(), baml_rt::BamlRtError>(())
    /// # }).unwrap();
    /// ```
    pub fn map_baml_variant_to_tool(
        &mut self,
        baml_variant_name: impl Into<String>,
        tool_function_name: impl Into<String>,
    ) {
        if let Ok(mut mapper) = self.tool_mapper.lock() {
            mapper.register_mapping(baml_variant_name, tool_function_name);
        } else {
            tracing::warn!("Failed to acquire tool mapper lock");
        }
    }

    /// Execute a tool from a BAML union type result
    ///
    /// Takes a BAML result (which should be a union variant representing a tool choice),
    /// maps it to the appropriate tool function, and executes it.
    ///
    /// # Arguments
    /// * `baml_result` - The JSON result from BAML function (union variant)
    ///
    /// # Returns
    /// The result of executing the tool function
    pub async fn execute_tool_from_baml_result(&self, baml_result: Value) -> Result<Value> {
        // Parse the BAML result to extract tool name and args
        let (variant_name, tool_args_value) = self
            .tool_mapper
            .lock()
            .map_err(|_| BamlRtError::ToolMapperLockPoisoned)?
            .parse_variant_and_args(&baml_result)?;

        // Map variant to tool name
        let tool_name = self
            .tool_mapper
            .lock()
            .map_err(|_| BamlRtError::ToolMapperLockPoisoned)?
            .variant_to_tool_name(&variant_name)?;

        // Execute via execute_tool which handles interceptors
        self.execute_tool(&tool_name, tool_args_value).await
    }
}

// Implement traits for better abstraction
#[async_trait]
impl BamlFunctionExecutor for BamlRuntimeManager {
    async fn execute_function(&self, function_name: &str, args: Value) -> Result<Value> {
        self.invoke_function(function_name, args).await
    }

    fn list_functions(&self) -> Vec<String> {
        self.function_registry.keys().cloned().collect()
    }
}

impl SchemaLoader for BamlRuntimeManager {
    fn load_schema(&mut self, schema_path: &str) -> Result<()> {
        self.load_schema(schema_path)
    }

    fn is_schema_loaded(&self) -> bool {
        self.is_schema_loaded()
    }
}

impl Default for BamlRuntimeManager {
    fn default() -> Self {
        Self {
            function_registry: HashMap::new(),
            executor: None,
            tool_registry: Arc::new(TokioMutex::new(ConcreteToolRegistry::new())),
            tool_mapper: Arc::new(StdMutex::new(ToolMapper::new())),
            interceptor_registry: Arc::new(TokioMutex::new(InterceptorRegistry::new())),
        }
    }
}
