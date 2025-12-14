//! Tests for dynamic tool registration and invocation

use baml_rt::baml::BamlRuntimeManager;
use baml_rt::quickjs_bridge::QuickJSBridge;
use baml_rt::tools::BamlTool;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use async_trait::async_trait;

// Simple test tools
struct AddNumbersTool;

#[async_trait]
impl BamlTool for AddNumbersTool {
    const NAME: &'static str = "add_numbers";
    
    fn description(&self) -> &'static str {
        "Adds two numbers together"
    }
    
    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "a": {"type": "number", "description": "First number"},
                "b": {"type": "number", "description": "Second number"}
            },
            "required": ["a", "b"]
        })
    }
    
    async fn execute(&self, args: serde_json::Value) -> baml_rt::error::Result<serde_json::Value> {
        let obj = args.as_object().expect("Expected object");
        let a = obj.get("a").and_then(|v| v.as_f64()).expect("Expected 'a' number");
        let b = obj.get("b").and_then(|v| v.as_f64()).expect("Expected 'b' number");
        Ok(json!({"result": a + b}))
    }
}

struct GreetTool;

#[async_trait]
impl BamlTool for GreetTool {
    const NAME: &'static str = "greet";
    
    fn description(&self) -> &'static str {
        "Returns a greeting message"
    }
    
    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Name to greet"}
            },
            "required": ["name"]
        })
    }
    
    async fn execute(&self, args: serde_json::Value) -> baml_rt::error::Result<serde_json::Value> {
        let obj = args.as_object().expect("Expected object");
        let name = obj.get("name").and_then(|v| v.as_str()).expect("Expected 'name' string");
        Ok(json!({"greeting": format!("Hello, {}!", name)}))
    }
}

struct StreamLettersTool;

#[async_trait]
impl BamlTool for StreamLettersTool {
    const NAME: &'static str = "stream_letters";
    
    fn description(&self) -> &'static str {
        "Streams letters of a word one by one"
    }
    
    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "word": {"type": "string", "description": "Word to stream"}
            },
            "required": ["word"]
        })
    }
    
    async fn execute(&self, args: serde_json::Value) -> baml_rt::error::Result<serde_json::Value> {
        use tokio::time::{sleep, Duration};
        
        let obj = args.as_object().expect("Expected object");
        let word = obj.get("word").and_then(|v| v.as_str()).expect("Expected 'word' string");
        
        // Simulate streaming by waiting a bit
        sleep(Duration::from_millis(10)).await;
        
        // Return all letters as an array (in a real streaming scenario,
        // this would be a stream, but for now we return the result)
        let letters: Vec<String> = word.chars().map(|c| c.to_string()).collect();
        Ok(json!({"letters": letters, "count": letters.len()}))
    }
}

#[tokio::test]
async fn test_register_and_execute_tool_rust() {
    // Create BAML runtime manager
    let mut baml_manager = BamlRuntimeManager::new().unwrap();
    baml_manager.load_schema("baml_src").unwrap();
    let baml_manager = Arc::new(Mutex::new(baml_manager));

    // Register a simple calculator tool using the trait
    {
        let mut manager = baml_manager.lock().await;
        manager.register_tool(AddNumbersTool).await.unwrap();
    }

    // Test executing the tool directly from Rust
    {
        let manager = baml_manager.lock().await;
        let result = manager.execute_tool("add_numbers", json!({"a": 5, "b": 3})).await.unwrap();
        
        let result_obj = result.as_object().expect("Expected object");
        let sum = result_obj.get("result").and_then(|v| v.as_f64()).expect("Expected 'result' number");
        
        assert_eq!(sum, 8.0, "5 + 3 should equal 8");
    }

    // Test listing tools
    {
        let manager = baml_manager.lock().await;
        let tools = manager.list_tools().await;
        assert!(tools.contains(&"add_numbers".to_string()), "Should list registered tool");
    }
}

#[tokio::test]
async fn test_register_and_execute_tool_js() {
    // Create BAML runtime manager
    let mut baml_manager = BamlRuntimeManager::new().unwrap();
    baml_manager.load_schema("baml_src").unwrap();
    let baml_manager = Arc::new(Mutex::new(baml_manager));

    // Register a tool using the trait
    {
        let mut manager = baml_manager.lock().await;
        manager.register_tool(GreetTool).await.unwrap();
    }

    // Create QuickJS bridge and register functions
    let mut bridge = QuickJSBridge::new(baml_manager.clone()).unwrap();
    bridge.register_baml_functions().await.unwrap();

    // Test that tool is registered in QuickJS
    // Since eval() can't await promises, we verify registration and test execution via Rust
    let js_code = r#"
        JSON.stringify({ 
            toolExists: typeof greet === 'function',
            test: "Tool is registered in QuickJS"
        })
    "#;

    let result = bridge.evaluate(js_code).await.expect("Should check tool registration");
    
    // Verify tool is registered
    let obj = result.as_object().expect("Expected object");
    let tool_exists = obj.get("toolExists").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(tool_exists, "Tool 'greet' should be registered in QuickJS");

    // Test executing the tool directly from Rust to verify it works end-to-end
    {
        let manager = baml_manager.lock().await;
        let result = manager.execute_tool("greet", json!({"name": "World"})).await.unwrap();
        
        let result_obj = result.as_object().expect("Expected object");
        let greeting = result_obj.get("greeting").and_then(|g| g.as_str()).unwrap();
        assert_eq!(greeting, "Hello, World!", "Should return correct greeting");
    }
}

#[tokio::test]
async fn test_async_streaming_tool() {
    // Create BAML runtime manager
    let mut baml_manager = BamlRuntimeManager::new().unwrap();
    baml_manager.load_schema("baml_src").unwrap();
    let baml_manager = Arc::new(Mutex::new(baml_manager));

    // Register an async streaming tool using the trait
    {
        let mut manager = baml_manager.lock().await;
        manager.register_tool(StreamLettersTool).await.unwrap();
    }

    // Test executing the streaming tool
    {
        let manager = baml_manager.lock().await;
        let result = manager.execute_tool("stream_letters", json!({"word": "test"})).await.unwrap();
        
        let result_obj = result.as_object().expect("Expected object");
        let letters = result_obj.get("letters").and_then(|v| v.as_array()).expect("Expected 'letters' array");
        let count = result_obj.get("count").and_then(|v| v.as_u64()).expect("Expected 'count' number");
        
        assert_eq!(count, 4, "Word 'test' has 4 letters");
        assert_eq!(letters.len(), 4, "Should return 4 letters");
    }
}

