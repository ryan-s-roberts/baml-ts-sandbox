//! End-to-end test for LLM tool calling with registered tools

use baml_rt::baml::BamlRuntimeManager;
use baml_rt::quickjs_bridge::QuickJSBridge;
use baml_rt::tools::BamlTool;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use async_trait::async_trait;

// Test tools
struct WeatherToolTest;

#[async_trait]
impl BamlTool for WeatherToolTest {
    const NAME: &'static str = "get_weather";
    
    fn description(&self) -> &'static str {
        "Gets the current weather for a specific location. Returns temperature, condition, and humidity."
    }
    
    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "The city and state or city and country, e.g. San Francisco, CA or London, UK"
                }
            },
            "required": ["location"]
        })
    }
    
    async fn execute(&self, args: serde_json::Value) -> baml_rt::error::Result<serde_json::Value> {
        let obj = args.as_object().expect("Expected object");
        let location = obj.get("location")
            .and_then(|v| v.as_str())
            .expect("Expected 'location' string");
        
        tracing::info!(location = location, "Weather tool called");
        
        Ok(json!({
            "location": location,
            "temperature": "22°C",
            "temperature_f": 72,
            "condition": "Sunny with clear skies",
            "humidity": "65%",
            "wind_speed": "10 km/h",
            "description": format!("Current weather in {}: Sunny, 22°C, 65% humidity", location)
        }))
    }
}

struct CalculatorToolTest;

#[async_trait]
impl BamlTool for CalculatorToolTest {
    const NAME: &'static str = "calculate";
    
    fn description(&self) -> &'static str {
        "Performs mathematical calculations. Can handle addition, subtraction, multiplication, and division."
    }
    
    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "A mathematical expression to evaluate, e.g. '15 * 23' or '100 - 42'"
                }
            },
            "required": ["expression"]
        })
    }
    
    async fn execute(&self, args: serde_json::Value) -> baml_rt::error::Result<serde_json::Value> {
        let obj = args.as_object().expect("Expected object");
        let expr = obj.get("expression")
            .and_then(|v| v.as_str())
            .expect("Expected 'expression' string");
        
        tracing::info!(expression = expr, "Calculator tool called");
        
        let result = if let Some(pos) = expr.find('+') {
            let a: f64 = expr[..pos].trim().parse().unwrap_or(0.0);
            let b: f64 = expr[pos+1..].trim().parse().unwrap_or(0.0);
            a + b
        } else if let Some(pos) = expr.find('-') {
            let a: f64 = expr[..pos].trim().parse().unwrap_or(0.0);
            let b: f64 = expr[pos+1..].trim().parse().unwrap_or(0.0);
            a - b
        } else if let Some(pos) = expr.find('*') {
            let a: f64 = expr[..pos].trim().parse().unwrap_or(0.0);
            let b: f64 = expr[pos+1..].trim().parse().unwrap_or(0.0);
            a * b
        } else if let Some(pos) = expr.find('/') {
            let a: f64 = expr[..pos].trim().parse().unwrap_or(0.0);
            let b: f64 = expr[pos+1..].trim().parse().unwrap_or(0.0);
            if b != 0.0 { a / b } else { 0.0 }
        } else {
            0.0
        };
        
        Ok(json!({
            "expression": expr,
            "result": result,
            "formatted": format!("{} = {}", expr, result)
        }))
    }
}

#[tokio::test]
async fn test_llm_tool_calling_rust() {
    // This test verifies tool registration and execution
    // API key is optional - test focuses on tool registration infrastructure
    
    // Set up BAML runtime
    let mut baml_manager = BamlRuntimeManager::new().unwrap();
    baml_manager.load_schema("baml_src").unwrap();
    
    // Register tools using the trait-based approach
    baml_manager.register_tool(WeatherToolTest).await.unwrap();
    baml_manager.register_tool(CalculatorToolTest).await.unwrap();
    
    let baml_manager = Arc::new(Mutex::new(baml_manager));
    
    // Test that tools are registered and can be executed
    {
        let manager = baml_manager.lock().await;
        
        // Test weather tool
        let weather_result = manager.execute_tool("get_weather", json!({"location": "San Francisco"})).await.unwrap();
        let weather_obj = weather_result.as_object().expect("Expected object");
        assert!(weather_obj.contains_key("temperature"), "Weather result should contain temperature");
        
        // Test calculator tool
        let calc_result = manager.execute_tool("calculate", json!({"expression": "2 + 2"})).await.unwrap();
        let calc_obj = calc_result.as_object().expect("Expected object");
        let result = calc_obj.get("result").and_then(|v| v.as_f64()).unwrap();
        assert_eq!(result, 4.0, "2 + 2 should equal 4");
        
        // List tools
        let tools = manager.list_tools().await;
        assert!(tools.contains(&"get_weather".to_string()), "Should list weather tool");
        assert!(tools.contains(&"calculate".to_string()), "Should list calculator tool");
    }
    
    tracing::info!("Tool registration and execution tests passed");
    
    // Note: Actual LLM tool calling integration with BAML would require
    // passing the tool registry to BAML's call_function with client_registry.
    // This test verifies the foundation is in place.
}

#[tokio::test]
async fn test_llm_tool_calling_js() {
    // Set up BAML runtime
    let mut baml_manager = BamlRuntimeManager::new().unwrap();
    baml_manager.load_schema("baml_src").unwrap();
    
    // Register a tool using the trait
    struct ReverseStringTool;
    
    #[async_trait]
    impl BamlTool for ReverseStringTool {
        const NAME: &'static str = "reverse_string";
        
        fn description(&self) -> &'static str {
            "Reverses a string"
        }
        
        fn input_schema(&self) -> serde_json::Value {
            json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Text to reverse"}
                },
                "required": ["text"]
            })
        }
        
        async fn execute(&self, args: serde_json::Value) -> baml_rt::error::Result<serde_json::Value> {
            let obj = args.as_object().expect("Expected object");
            let text = obj.get("text").and_then(|v| v.as_str()).expect("Expected 'text' string");
            let reversed: String = text.chars().rev().collect();
            Ok(json!({"reversed": reversed, "original": text}))
        }
    }
    
    baml_manager.register_tool(ReverseStringTool).await.unwrap();
    
    let baml_manager = Arc::new(Mutex::new(baml_manager));
    
    // Create QuickJS bridge and register functions
    let mut bridge = QuickJSBridge::new(baml_manager.clone()).unwrap();
    bridge.register_baml_functions().await.unwrap();
    
    // Test that tool is registered in JavaScript
    let js_code = r#"
        JSON.stringify({ 
            toolExists: typeof reverse_string === 'function',
            test: "Tool is registered in QuickJS"
        })
    "#;

    let result = bridge.evaluate(js_code).await.expect("Should check tool registration");
    let obj = result.as_object().expect("Expected object");
    let tool_exists = obj.get("toolExists").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(tool_exists, "Tool 'reverse_string' should be registered in QuickJS");
    
    // Test executing the tool from Rust
    {
        let manager = baml_manager.lock().await;
        let result = manager.execute_tool("reverse_string", json!({"text": "hello"})).await.unwrap();
        
        let result_obj = result.as_object().expect("Expected object");
        let reversed = result_obj.get("reversed").and_then(|r| r.as_str()).unwrap();
        assert_eq!(reversed, "olleh", "Should reverse the string correctly");
    }
}

