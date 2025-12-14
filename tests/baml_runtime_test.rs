//! Tests for BAML runtime manager

use baml_rt::baml::BamlRuntimeManager;
use baml_rt::error::BamlRtError;

#[tokio::test]
async fn test_create_runtime_manager() {
    let manager = BamlRuntimeManager::new().expect("Should create manager");
    assert!(manager.list_functions().is_empty());
}

#[tokio::test]
async fn test_invoke_nonexistent_function() {
    let manager = BamlRuntimeManager::new().expect("Should create manager");
    
    let result = manager
        .invoke_function("nonexistent", serde_json::json!({}))
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        BamlRtError::FunctionNotFound(name) => {
            assert_eq!(name, "nonexistent");
        }
        _ => panic!("Expected FunctionNotFound error"),
    }
}

#[tokio::test]
async fn test_list_functions() {
    let manager = BamlRuntimeManager::new().expect("Should create manager");
    let functions = manager.list_functions();
    assert_eq!(functions.len(), 0);
}

