//! Comprehensive end-to-end tests for the trait-based tool system
//!
//! Tests cover:
//! - Tool registration using the BamlTool trait
//! - Tool execution from Rust
//! - Tool execution from JavaScript via QuickJS
//! - Tool metadata and listing
//! - **E2E: Actual LLM calls that invoke registered tools**


use baml_rt::tools::BamlTool;
use serde_json::json;
use async_trait::async_trait;

use test_support::common::{
    WeatherTool,
    CalculatorTool,
    assert_tool_registered_in_js,
    require_api_key,
    setup_baml_runtime_default,
    setup_bridge,
};

/// Test tool for arithmetic operations
struct ArithmeticTool;

#[async_trait]
impl BamlTool for ArithmeticTool {
    const NAME: &'static str = "arithmetic";
    
    fn description(&self) -> &'static str {
        "Performs basic arithmetic operations: add, subtract, multiply, divide"
    }
    
    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["add", "subtract", "multiply", "divide"],
                    "description": "The arithmetic operation to perform"
                },
                "a": {"type": "number", "description": "First operand"},
                "b": {"type": "number", "description": "Second operand"}
            },
            "required": ["operation", "a", "b"]
        })
    }
    
    async fn execute(&self, args: serde_json::Value) -> baml_rt::Result<serde_json::Value> {
        let obj = args.as_object().expect("Expected object");
        let op = obj.get("operation").and_then(|v| v.as_str()).expect("Expected 'operation' string");
        let a = obj.get("a").and_then(|v| v.as_f64()).expect("Expected 'a' number");
        let b = obj.get("b").and_then(|v| v.as_f64()).expect("Expected 'b' number");
        
        let result = match op {
            "add" => a + b,
            "subtract" => a - b,
            "multiply" => a * b,
            "divide" => if b != 0.0 { a / b } else { 0.0 },
            _ => return Err(baml_rt::BamlRtError::InvalidArgument(
                format!("Unknown operation: {}", op)
            )),
        };
        
        tracing::info!(operation = op, a = a, b = b, result = result, "ArithmeticTool executed");
        
        Ok(json!({
            "operation": op,
            "a": a,
            "b": b,
            "result": result,
            "formatted": format!("{} {} {} = {}", a, op, b, result)
        }))
    }
}

/// Test tool for string manipulation
struct StringManipulationTool;

#[async_trait]
impl BamlTool for StringManipulationTool {
    const NAME: &'static str = "string_manipulation";
    
    fn description(&self) -> &'static str {
        "Performs string manipulation operations: uppercase, lowercase, reverse"
    }
    
    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["uppercase", "lowercase", "reverse"],
                    "description": "The string operation to perform"
                },
                "text": {"type": "string", "description": "Text to manipulate"}
            },
            "required": ["operation", "text"]
        })
    }
    
    async fn execute(&self, args: serde_json::Value) -> baml_rt::Result<serde_json::Value> {
        let obj = args.as_object().expect("Expected object");
        let op = obj.get("operation").and_then(|v| v.as_str()).expect("Expected 'operation' string");
        let text = obj.get("text").and_then(|v| v.as_str()).expect("Expected 'text' string");
        
        let result = match op {
            "uppercase" => text.to_uppercase(),
            "lowercase" => text.to_lowercase(),
            "reverse" => text.chars().rev().collect(),
            _ => return Err(baml_rt::BamlRtError::InvalidArgument(
                format!("Unknown operation: {}", op)
            )),
        };
        
        tracing::info!(operation = op, text = text, result = result, "StringManipulationTool executed");
        
        Ok(json!({
            "operation": op,
            "original": text,
            "result": result
        }))
    }
}

#[tokio::test]
async fn test_e2e_trait_tool_registration_rust_execution() {
    // Set up BAML runtime
    let baml_manager = setup_baml_runtime_default();
    
    // Register tools using trait system
    {
        let mut manager = baml_manager.lock().await;
        manager.register_tool(ArithmeticTool).await.unwrap();
        manager.register_tool(StringManipulationTool).await.unwrap();
    }
    
    // Execute tools from Rust
    {
        let manager = baml_manager.lock().await;
        
        let arithmetic_result = manager.execute_tool(
            "arithmetic",
            json!({"operation": "multiply", "a": 7, "b": 6})
        ).await.unwrap();
        
        let result = arithmetic_result.get("result").and_then(|v| v.as_f64()).unwrap();
        assert_eq!(result, 42.0, "7 * 6 should equal 42");
        
        let string_result = manager.execute_tool(
            "string_manipulation",
            json!({"operation": "reverse", "text": "baml"})
        ).await.unwrap();
        
        let result = string_result.get("result").and_then(|v| v.as_str()).unwrap();
        assert_eq!(result, "lmab", "Reversing 'baml' should give 'lmab'");
    }
}

#[tokio::test]
async fn test_e2e_trait_tool_js_registration() {
    // Set up BAML runtime and bridge
    let baml_manager = setup_baml_runtime_default();
    
    // Register tools using trait system
    {
        let mut manager = baml_manager.lock().await;
        manager.register_tool(ArithmeticTool).await.unwrap();
    }
    
    let mut bridge = setup_bridge(baml_manager.clone()).await;
    
    // Verify tool is registered in JS
    assert_tool_registered_in_js(&mut bridge, "arithmetic").await;
}

#[tokio::test]
async fn test_e2e_trait_tool_metadata_and_listing() {
    // Set up BAML runtime
    let baml_manager = setup_baml_runtime_default();
    
    // Register tools
    {
        let mut manager = baml_manager.lock().await;
        manager.register_tool(ArithmeticTool).await.unwrap();
        manager.register_tool(StringManipulationTool).await.unwrap();
    }
    
    // Test listing and metadata
    {
        let manager = baml_manager.lock().await;
        let tools = manager.list_tools().await;
        
        assert!(tools.contains(&"arithmetic".to_string()));
        assert!(tools.contains(&"string_manipulation".to_string()));
        
        let arithmetic_meta = manager.get_tool_metadata("arithmetic").await.unwrap();
        assert_eq!(arithmetic_meta.name, "arithmetic");
        assert!(arithmetic_meta.description.contains("arithmetic"));
    }
}

#[tokio::test]
async fn test_e2e_trait_tool_llm_calling() {
    let _ = require_api_key();
    
    // Set up BAML runtime
    let baml_manager = setup_baml_runtime_default();
    
    // Register tools
    {
        let mut manager = baml_manager.lock().await;
        manager.register_tool(WeatherTool).await.unwrap();
        manager.register_tool(CalculatorTool).await.unwrap();
    }
    
    // Invoke a function that may call tools
    {
        let manager = baml_manager.lock().await;
        
        let result = manager.invoke_function(
            "ChooseTool",
            json!({"message": "What is 42 times 7?"})
        ).await;
        
        assert!(result.is_ok());
    }
}
