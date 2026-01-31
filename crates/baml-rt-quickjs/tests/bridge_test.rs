//! Tests for QuickJS bridge integration

use baml_rt::baml::BamlRuntimeManager;
use baml_rt::quickjs_bridge::QuickJSBridge;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_quickjs_bridge_creation() {
    // Test that we can create a QuickJS bridge
    let baml_manager = Arc::new(Mutex::new(BamlRuntimeManager::new().unwrap()));
    let bridge = QuickJSBridge::new(baml_manager);
    
    let bridge = bridge.await;
    assert!(bridge.is_ok(), "Should be able to create QuickJS bridge");
}

#[tokio::test]
async fn test_quickjs_evaluate_simple_code() {
    // Test that we can execute simple JavaScript code
    let baml_manager = Arc::new(Mutex::new(BamlRuntimeManager::new().unwrap()));
    let mut bridge = QuickJSBridge::new(baml_manager).await.unwrap();
    
    // Execute a simple JavaScript expression
    let result = bridge.evaluate("2 + 2").await;
    
    // The result might be a string representation or actual JSON
    // For now, just check that it doesn't error
    assert!(result.is_ok(), "Should be able to execute JavaScript code");
}

#[tokio::test]
async fn test_quickjs_evaluate_json() {
    // Test JSON stringify/parse
    let baml_manager = Arc::new(Mutex::new(BamlRuntimeManager::new().unwrap()));
    let mut bridge = QuickJSBridge::new(baml_manager).await.unwrap();
    
    // Execute code that returns a JSON object
    let result = bridge.evaluate("({answer: 42})").await;
    
    assert!(result.is_ok(), "Should be able to execute JavaScript and get JSON");
}
