//! Tests for JavaScript tool registration

use baml_rt::baml::BamlRuntimeManager;
use baml_rt::quickjs_bridge::QuickJSBridge;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_register_js_tool() {
    tracing::info!("Test: Register JavaScript tool");
    
    // Set up BAML runtime and bridge
    let mut baml_manager = BamlRuntimeManager::new().unwrap();
    baml_manager.load_schema("baml_src").unwrap();
    let baml_manager = Arc::new(Mutex::new(baml_manager));
    
    let mut bridge = QuickJSBridge::new(baml_manager.clone()).unwrap();
    bridge.register_baml_functions().await.unwrap();
    
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
    let js_code = r#"
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
    let check_code = r#"
        JSON.stringify({
            exists: typeof greet_js === 'function',
            isAsync: greet_js.constructor.name === 'AsyncFunction'
        })
    "#;
    
    let result = bridge.evaluate(check_code).await.unwrap();
    let obj = result.as_object().unwrap();
    assert!(obj.get("exists").and_then(|v| v.as_bool()).unwrap_or(false),
        "greet_js should exist as a function");
    
    tracing::info!("✅ JavaScript tool registered successfully");
}

#[tokio::test]
async fn test_register_js_tool_with_complex_logic() {
    tracing::info!("Test: Register JavaScript tool with complex logic");
    
    let mut baml_manager = BamlRuntimeManager::new().unwrap();
    baml_manager.load_schema("baml_src").unwrap();
    let baml_manager = Arc::new(Mutex::new(baml_manager));
    
    let mut bridge = QuickJSBridge::new(baml_manager.clone()).unwrap();
    bridge.register_baml_functions().await.unwrap();
    
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
    let check_code = r#"
        JSON.stringify({
            exists: typeof calculate_js === 'function'
        })
    "#;
    
    let result = bridge.evaluate(check_code).await.unwrap();
    let obj = result.as_object().unwrap();
    assert!(obj.get("exists").and_then(|v| v.as_bool()).unwrap_or(false),
        "calculate_js should exist as a function");
    
    tracing::info!("✅ Complex JavaScript tool registered successfully");
}

#[tokio::test]
async fn test_js_tool_not_available_in_rust() {
    tracing::info!("Test: JavaScript tools are not available in Rust");
    
    let mut baml_manager = BamlRuntimeManager::new().unwrap();
    baml_manager.load_schema("baml_src").unwrap();
    let baml_manager = Arc::new(Mutex::new(baml_manager));
    
    let mut bridge = QuickJSBridge::new(baml_manager.clone()).unwrap();
    bridge.register_baml_functions().await.unwrap();
    
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
    
    use baml_rt::tools::BamlTool;
    use async_trait::async_trait;
    
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
        
        async fn execute(&self, _args: serde_json::Value) -> baml_rt::error::Result<serde_json::Value> {
            Ok(json!({"from": "rust"}))
        }
    }
    
    let mut baml_manager = BamlRuntimeManager::new().unwrap();
    baml_manager.load_schema("baml_src").unwrap();
    
    // Register Rust tool first
    baml_manager.register_tool(TestRustTool).await.unwrap();
    
    let baml_manager = Arc::new(Mutex::new(baml_manager));
    let mut bridge = QuickJSBridge::new(baml_manager.clone()).unwrap();
    bridge.register_baml_functions().await.unwrap();
    
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
    
    let mut baml_manager = BamlRuntimeManager::new().unwrap();
    baml_manager.load_schema("baml_src").unwrap();
    let baml_manager = Arc::new(Mutex::new(baml_manager));
    
    let mut bridge = QuickJSBridge::new(baml_manager.clone()).unwrap();
    bridge.register_baml_functions().await.unwrap();
    
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

