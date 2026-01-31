//! Tests for tool registration (Rust and JavaScript)

use async_trait::async_trait;
use baml_rt::tools::BamlTool;
use serde_json::json;

use test_support::common::{
    assert_tool_registered_in_js,
    setup_baml_runtime_default,
    setup_baml_runtime_manager_default,
    setup_bridge,
};
use std::sync::Arc;
use tokio::sync::Mutex;
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
    
    async fn execute(&self, args: serde_json::Value) -> baml_rt::Result<serde_json::Value> {
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
    
    async fn execute(&self, args: serde_json::Value) -> baml_rt::Result<serde_json::Value> {
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
    
    async fn execute(&self, args: serde_json::Value) -> baml_rt::Result<serde_json::Value> {
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
    let baml_manager = setup_baml_runtime_default();

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
    let baml_manager = setup_baml_runtime_default();

    // Register a tool using the trait
    {
        let mut manager = baml_manager.lock().await;
        manager.register_tool(GreetTool).await.unwrap();
    }

    // Create QuickJS bridge and register functions
    let mut bridge = setup_bridge(baml_manager.clone()).await;

    // Test that tool is registered in QuickJS
    // Since eval() can't await promises, we verify registration and test execution via Rust
    assert_tool_registered_in_js(&mut bridge, "greet").await;

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
    let baml_manager = setup_baml_runtime_default();

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

#[tokio::test]
async fn test_register_js_tool() {
    tracing::info!("Test: Register JavaScript tool");
    
    // Set up BAML runtime and bridge
    let baml_manager = setup_baml_runtime_default();
    
    let mut bridge = setup_bridge(baml_manager.clone()).await;
    
    // Register a simple JavaScript tool
    bridge.register_js_tool("greet_js", r#"
        async function(name) {
            return { greeting: `Hello, ${name}!` };
        }
    "#).await.unwrap();
    
    // Verify it's listed
    let js_tools = bridge.list_js_tools();
    assert!(js_tools.contains(&"greet_js".to_string()), "Should list greet_js tool");
    
    // Verify it's callable from JavaScript
    let _js_code = r#"
        (async () => {
            try {
                const result = await greet_js("World");
                return JSON.stringify({
                    success: true,
                    greeting: result.greeting
                });
            } catch (e) {
                return JSON.stringify({
                    success: false,
                    error: e.toString()
                });
            }
        })()
    "#;
    
    // Note: We can't easily await this in eval(), but we can check it exists
    assert_tool_registered_in_js(&mut bridge, "greet_js").await;

    let check_code = r#"
        (() => JSON.stringify({
            isAsync: greet_js.constructor.name === 'AsyncFunction'
        }))()
    "#;

    let result = bridge.evaluate(check_code).await.unwrap();
    let obj = result.as_object().unwrap();
    assert!(
        obj.get("isAsync").and_then(|v| v.as_bool()).unwrap_or(false),
        "greet_js should be async"
    );
    
    tracing::info!("✅ JavaScript tool registered successfully");
}

#[tokio::test]
async fn test_register_js_tool_with_complex_logic() {
    tracing::info!("Test: Register JavaScript tool with complex logic");
    
    let baml_manager = setup_baml_runtime_default();
    
    let mut bridge = setup_bridge(baml_manager.clone()).await;
    
    // Register a more complex JavaScript tool
    bridge.register_js_tool("calculate_js", r#"
        async function(expression) {
            try {
                // Simple calculator using eval (for testing only - would use safer parser in production)
                const result = Function('"use strict"; return (' + expression + ')')();
                return {
                    expression: expression,
                    result: result,
                    formatted: `${expression} = ${result}`
                };
            } catch (e) {
                return {
                    expression: expression,
                    error: e.message
                };
            }
        }
    "#).await.unwrap();
    
    // Verify it exists
    let js_tools = bridge.list_js_tools();
    assert!(js_tools.contains(&"calculate_js".to_string()), "Should list calculate_js tool");
    
    // Check function exists
    assert_tool_registered_in_js(&mut bridge, "calculate_js").await;
    
    tracing::info!("✅ Complex JavaScript tool registered successfully");
}

#[tokio::test]
async fn test_js_tool_not_available_in_rust() {
    tracing::info!("Test: JavaScript tools are not available in Rust");
    
    let baml_manager = setup_baml_runtime_default();
    
    let mut bridge = setup_bridge(baml_manager.clone()).await;
    
    // Register a JavaScript tool
    bridge.register_js_tool("js_only_tool", r#"
        async function() {
            return { from: "javascript" };
        }
    "#).await.unwrap();
    
    // Verify it's NOT in the Rust tool registry
    let manager = baml_manager.lock().await;
    let rust_tools = manager.list_tools().await;
    assert!(!rust_tools.contains(&"js_only_tool".to_string()),
        "JS tool should NOT be in Rust tool registry");
    
    // Verify it IS a JS tool
    assert!(bridge.is_js_tool("js_only_tool"),
        "Should identify js_only_tool as a JavaScript tool");
    
    tracing::info!("✅ JavaScript tools correctly isolated from Rust");
}

#[tokio::test]
async fn test_js_tool_name_conflict_with_rust_tool() {
    tracing::info!("Test: JavaScript tool name conflict detection");
    
    // Create a Rust tool
    struct TestRustTool;
    
    #[async_trait]
    impl BamlTool for TestRustTool {
        const NAME: &'static str = "conflict_tool";
        
        fn description(&self) -> &'static str {
            "A Rust tool"
        }
        
        fn input_schema(&self) -> serde_json::Value {
            json!({})
        }
        
        async fn execute(&self, _args: serde_json::Value) -> baml_rt::Result<serde_json::Value> {
            Ok(json!({"from": "rust"}))
        }
    }
    
    let mut baml_manager = setup_baml_runtime_manager_default();
    
    // Register Rust tool first
    baml_manager.register_tool(TestRustTool).await.unwrap();
    
    let baml_manager = Arc::new(Mutex::new(baml_manager));
    let mut bridge = setup_bridge(baml_manager.clone()).await;
    
    // Try to register a JS tool with the same name - should fail
    let result = bridge.register_js_tool("conflict_tool", r#"
        async function() {
            return { from: "javascript" };
        }
    "#).await;
    
    assert!(result.is_err(), "Should reject JS tool with conflicting name");
    assert!(result.unwrap_err().to_string().contains("conflicts with existing Rust tool"),
        "Error should mention conflict with Rust tool");
    
    tracing::info!("✅ JavaScript tool name conflict correctly detected");
}

#[tokio::test]
async fn test_register_multiple_js_tools() {
    tracing::info!("Test: Register multiple JavaScript tools");
    
    let baml_manager = setup_baml_runtime_default();
    
    let mut bridge = setup_bridge(baml_manager.clone()).await;
    
    // Register multiple JS tools
    bridge.register_js_tool("tool1", r#"async function() { return { id: 1 }; }"#).await.unwrap();
    bridge.register_js_tool("tool2", r#"async function() { return { id: 2 }; }"#).await.unwrap();
    bridge.register_js_tool("tool3", r#"async function() { return { id: 3 }; }"#).await.unwrap();
    
    // Verify all are listed
    let js_tools = bridge.list_js_tools();
    assert_eq!(js_tools.len(), 3, "Should have 3 JS tools");
    assert!(js_tools.contains(&"tool1".to_string()));
    assert!(js_tools.contains(&"tool2".to_string()));
    assert!(js_tools.contains(&"tool3".to_string()));
    
    tracing::info!("✅ Multiple JavaScript tools registered successfully");
}
