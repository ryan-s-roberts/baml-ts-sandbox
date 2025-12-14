//! QuickJS integration bridge
//!
//! This module maps BAML function calls (executed in Rust) to QuickJS,
//! allowing JavaScript code to invoke BAML functions.

use crate::baml::BamlRuntimeManager;
use crate::error::{BamlRtError, Result};
use crate::js_value_converter::value_to_js_value_facade;
use quickjs_runtime::builder::QuickJsRuntimeBuilder;
use quickjs_runtime::facades::QuickJsRuntimeFacade;
use quickjs_runtime::jsutils::Script;
use quickjs_runtime::quickjsrealmadapter::QuickJsRealmAdapter;
use quickjs_runtime::values::{JsValueConvertable, JsValueFacade};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Bridge between QuickJS JavaScript runtime and BAML functions
/// 
/// BAML functions execute in Rust. This bridge exposes them to QuickJS
/// so JavaScript code can call them.
pub struct QuickJSBridge {
    runtime: QuickJsRuntimeFacade,
    baml_manager: Arc<Mutex<BamlRuntimeManager>>,
}

impl QuickJSBridge {
    /// Create a new QuickJS bridge
    pub fn new(baml_manager: Arc<Mutex<BamlRuntimeManager>>) -> Result<Self> {
        tracing::info!("Initializing QuickJS bridge");

        // Initialize QuickJS runtime using builder
        let runtime = QuickJsRuntimeBuilder::new()
            .build();

        Ok(Self {
            runtime,
            baml_manager,
        })
    }

    /// Register all BAML functions with the QuickJS context
    /// 
    /// This maps Rust BAML functions to JavaScript callables.
    /// When JS calls the function, it will invoke the Rust BAML execution.
    pub async fn register_baml_functions(&mut self) -> Result<()> {
        tracing::info!("Registering BAML functions with QuickJS");

        let manager = self.baml_manager.lock().await;
        let functions = manager.list_functions();
        drop(manager); // Release lock before async operation

        // First, register helper functions that JavaScript can call to invoke BAML functions
        self.register_baml_invoke_helper().await?;
        self.register_baml_stream_helper().await?;

        for function_name in functions {
            self.register_single_function(&function_name).await?;
            self.register_single_stream_function(&function_name).await?;
        }

        Ok(())
    }

    /// Register a helper function that JavaScript can call to invoke BAML functions
    async fn register_baml_invoke_helper(&mut self) -> Result<()> {
        let manager_clone = self.baml_manager.clone();
        
        // Register a native Rust function that JavaScript can call
        // This function will handle the async BAML execution using promises
        self.runtime.set_function(
            &[],
            "__baml_invoke",
            move |_realm: &QuickJsRealmAdapter, args: Vec<JsValueFacade>| -> std::result::Result<JsValueFacade, quickjs_runtime::jsutils::JsError> {
                if args.len() < 2 {
                    return Err(quickjs_runtime::jsutils::JsError::new_str("Expected 2 arguments: function_name and args"));
                }

                // Extract function name (first arg should be a string)
                let func_name_js = &args[0];
                let func_name = if func_name_js.is_string() {
                    func_name_js.get_str().to_string()
                } else {
                    return Err(quickjs_runtime::jsutils::JsError::new_str("First argument must be a string (function name)"));
                };

                // Extract args (second arg) - for complex objects, we still use JSON.stringify
                // but we can optimize this in the future
                let args_js = &args[1];
                // For now, convert to string and parse back - we can optimize this later
                // The issue is that JsValueFacade doesn't expose direct access to object properties
                let args_json_str = if args_js.is_string() {
                    args_js.get_str().to_string()
                } else {
                    // For non-strings, try to convert via debug format (fallback)
                    // In practice, JavaScript should pass JSON.stringify'd values
                    return Err(quickjs_runtime::jsutils::JsError::new_str("Args must be a JSON string - use JSON.stringify in JavaScript"));
                };

                // Parse JSON string to Value
                let args_json: Value = serde_json::from_str(&args_json_str)
                    .map_err(|e| quickjs_runtime::jsutils::JsError::new_str(&format!("Failed to parse JSON args: {}", e)))?;

                // Create a promise that will execute the BAML call asynchronously
                let manager = manager_clone.clone();
                let func_name_clone = func_name.clone();

                // Use JsValueFacade::new_promise to create a non-blocking promise
                // The producer is a Future that will be executed asynchronously
                // Type parameters: R is the result type (JsValueFacade), P is the Future, M is unused/mapper
                Ok(JsValueFacade::new_promise::<JsValueFacade, _, ()>(async move {
                    // Execute the BAML function asynchronously
                    let manager = manager.lock().await;
                    let result = manager.invoke_function(&func_name_clone, args_json).await;

                    match result {
                        Ok(json_value) => {
                            // Convert JSON value to JsValueFacade directly (no stringify needed)
                            Ok(value_to_js_value_facade(json_value))
                        }
                        Err(e) => {
                            Err(quickjs_runtime::jsutils::JsError::new_str(&format!("BAML execution error: {}", e)))
                        }
                    }
                }))
            },
        ).map_err(|e| BamlRtError::QuickJs(format!("Failed to register helper function: {}", e)))?;

        tracing::debug!("Registered __baml_invoke helper function with async promise support");
        Ok(())
    }

    /// Register a helper function for streaming BAML function execution
    async fn register_baml_stream_helper(&mut self) -> Result<()> {
        let manager_clone = self.baml_manager.clone();
        
        // Register a native Rust function that JavaScript can call for streaming
        self.runtime.set_function(
            &[],
            "__baml_stream",
            move |_realm: &QuickJsRealmAdapter, args: Vec<JsValueFacade>| -> std::result::Result<JsValueFacade, quickjs_runtime::jsutils::JsError> {
                if args.len() < 2 {
                    return Err(quickjs_runtime::jsutils::JsError::new_str("Expected 2 arguments: function_name and args"));
                }

                // Extract function name
                let func_name_js = &args[0];
                let func_name = if func_name_js.is_string() {
                    func_name_js.get_str().to_string()
                } else {
                    return Err(quickjs_runtime::jsutils::JsError::new_str("First argument must be a string (function name)"));
                };

                // Extract args (second arg) - JSON string from JavaScript
                let args_js = &args[1];
                let args_json_str = if args_js.is_string() {
                    args_js.get_str().to_string()
                } else {
                    return Err(quickjs_runtime::jsutils::JsError::new_str("Second argument must be a JSON string"));
                };

                // Parse JSON string to Value
                let args_json: Value = match serde_json::from_str(&args_json_str) {
                    Ok(v) => v,
                    Err(e) => return Err(quickjs_runtime::jsutils::JsError::new_str(&format!("Failed to parse JSON args: {}", e))),
                };

                let manager = manager_clone.clone();
                let func_name_clone = func_name.clone();

                // Create a promise that will execute the streaming BAML call
                let manager_for_stream = manager_clone.clone();
                Ok(JsValueFacade::new_promise::<JsValueFacade, _, ()>(async move {
                    use tokio::sync::mpsc;
                    let (tx, mut rx) = mpsc::channel::<serde_json::Value>(100);
                    
                    let func_name_stream = func_name_clone.clone();
                    let args_json_stream = args_json.clone();
                    
                    // Spawn a task to run the stream and send incremental results
                    tokio::spawn(async move {
                        // Create the stream
                        let manager = manager_for_stream.lock().await;
                        let stream_result = manager.invoke_function_stream(&func_name_stream, args_json_stream);
                        
                        // Get context manager reference while we have the lock
                        let executor_ref = manager.executor.as_ref().unwrap();
                        let ctx_manager = executor_ref.ctx_manager();
                        
                        // Create the stream
                        let mut stream = match stream_result {
                            Ok(s) => s,
                            Err(e) => {
                                drop(manager); // Release lock
                                let error_value = serde_json::json!({"error": format!("Failed to create stream: {}", e)});
                                let _ = tx.send(error_value).await;
                                return;
                            }
                        };
                        
                        // We need to keep the manager lock during stream execution
                        // because ctx_manager is a reference. For now, we'll collect all results
                        // in the callback and then drop the lock.
                        let env_vars = HashMap::new();
                        let (final_result, _call_id) = {
                            stream.run(
                                None::<fn()>, // on_tick
                                Some(|result: baml_runtime::FunctionResult| {
                                    // Extract incremental result and send it
                                    // parsed() returns Option<Result<ResponseBamlValue, Error>>
                                    if let Some(Ok(parsed)) = result.parsed() {
                                        if let Ok(parsed_value) = serde_json::to_value(parsed.serialize_partial()) {
                                            let _ = tx.try_send(parsed_value);
                                        }
                                    }
                                }),
                                ctx_manager,
                                None, // type_builder
                                None, // client_registry
                                env_vars,
                            ).await
                        };
                        drop(manager); // Release lock after stream completes

                        // Send final result
                        match final_result {
                            Ok(result) => {
                                // parsed() returns Option<Result<ResponseBamlValue, Error>>
                                if let Some(Ok(parsed)) = result.parsed() {
                                    if let Ok(final_value) = serde_json::to_value(parsed.serialize_partial()) {
                                        let _ = tx.send(final_value).await;
                                    }
                                }
                            }
                            Err(e) => {
                                let error_value = serde_json::json!({"error": format!("{}", e)});
                                let _ = tx.send(error_value).await;
                            }
                        }
                    });

                    // Collect results from the channel into an array
                    let mut results = Vec::new();
                    while let Some(value) = rx.recv().await {
                        results.push(value);
                    }

                    // Convert results array to JsValueFacade directly
                    Ok(value_to_js_value_facade(serde_json::Value::Array(results)))
                }))
            },
        ).map_err(|e| BamlRtError::QuickJs(format!("Failed to register streaming helper function: {}", e)))?;

        tracing::debug!("Registered __baml_stream helper function");
        Ok(())
    }

    /// Register a single BAML function with QuickJS
    async fn register_single_function(&mut self, function_name: &str) -> Result<()> {
        // Register a JavaScript wrapper function that calls the Rust helper
        // Use JSON.stringify to convert arguments to JSON
        // Note: For now, we're using a synchronous approach, but the JS function is async
        // to match the expected interface
        let js_code = format!(
            r#"
            globalThis.{} = async function(...args) {{
                // Convert arguments to a JSON object
                const argObj = {{}};
                // For now, handle simple cases - can be enhanced later
                if (args.length === 1 && typeof args[0] === 'object') {{
                    Object.assign(argObj, args[0]);
                }} else {{
                    // Try to map positional args to object properties
                    // This is a simplified mapping - could be improved with function signatures
                    args.forEach((arg, idx) => {{
                        argObj[`arg${{idx}}`] = arg;
                    }});
                }}
                
                // Call the Rust helper function - JSON.stringify once here is efficient
                // The helper returns a promise that will resolve asynchronously
                return await __baml_invoke("{}", JSON.stringify(argObj));
            }};
            "#,
            function_name, function_name
        );

        let script = Script::new("register_function.js", &js_code);
        let _result = self.runtime
            .eval(None, script)
            .await
            .map_err(|e| BamlRtError::QuickJs(format!("Failed to register function: {}", e)))?;
        
        tracing::debug!(function = function_name, "Registered function with QuickJS");
        
        Ok(())
    }

    /// Register a streaming version of a single BAML function with QuickJS
    async fn register_single_stream_function(&mut self, function_name: &str) -> Result<()> {
        // Register a JavaScript wrapper function for streaming
        let stream_function_name = format!("{}Stream", function_name);
        let js_code = format!(
            r#"
            globalThis.{} = async function(...args) {{
                // Convert arguments to a JSON object
                const argObj = {{}};
                if (args.length === 1 && typeof args[0] === 'object') {{
                    Object.assign(argObj, args[0]);
                }} else {{
                    args.forEach((arg, idx) => {{
                        argObj[`arg${{idx}}`] = arg;
                    }});
                }}
                
                // Call the Rust streaming helper function - JSON.stringify once here
                // This returns an array of incremental results
                const results = await __baml_stream("{}", JSON.stringify(argObj));
                
                // Return the array directly - JavaScript can iterate over it
                return results;
            }};
            "#,
            stream_function_name, function_name
        );

        let script = Script::new("register_stream_function.js", &js_code);
        let _result = self.runtime
            .eval(None, script)
            .await
            .map_err(|e| BamlRtError::QuickJs(format!("Failed to register stream function: {}", e)))?;
        
        tracing::debug!(function = function_name, stream_function = stream_function_name, "Registered streaming function with QuickJS");
        
        Ok(())
    }

    /// Execute JavaScript code in the QuickJS context
    /// 
    /// The code should return a JSON string (use JSON.stringify in JavaScript).
    /// This avoids double stringification and is more efficient.
    pub async fn evaluate(&mut self, code: &str) -> Result<Value> {
        tracing::debug!(code = code, "Executing JavaScript code");
        
        // Execute code directly - it should return a JSON string
        let script = Script::new("eval.js", code);
        
        let js_result = self.runtime
            .eval(None, script)
            .await
            .map_err(|e| BamlRtError::QuickJs(format!("Failed to execute JavaScript: {}", e)))?;

        // Result should be a JSON string - parse it
        if js_result.is_string() {
            let json_str = js_result.get_str();
            serde_json::from_str(json_str)
                .map_err(|e| BamlRtError::TypeConversion(format!("Failed to parse JSON result: {}", e)))
        } else {
            // If it's not a string, it might be a promise - this is an error
            // Code should properly await and stringify
            Err(BamlRtError::QuickJs(format!(
                "JavaScript code should return a JSON string. Got: {:?}. Ensure code uses JSON.stringify on the final result.",
                js_result
            )))
        }
    }
}

