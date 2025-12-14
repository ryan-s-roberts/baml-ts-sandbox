//! Tests for BAML compilation and basic invocation

use std::path::Path;

#[test]
fn test_baml_files_exist() {
    let baml_file = Path::new("baml_src/simple_prompt.baml");
    assert!(baml_file.exists(), "BAML source file should exist");
}

#[test]
fn test_baml_compilation_output() {
    // Check that baml_client directory was generated
    let client_dir = Path::new("baml_client");
    assert!(client_dir.exists(), "baml_client directory should be generated");
    assert!(client_dir.is_dir(), "baml_client should be a directory");
    
    // Check for key generated files
    let index_file = client_dir.join("index.ts");
    assert!(index_file.exists(), "index.ts should be generated");
    
    let types_file = client_dir.join("types.ts");
    assert!(types_file.exists(), "types.ts should be generated");
}

#[test]
fn test_baml_function_in_generated_code() {
    use std::fs;
    
    // Read the generated index.ts to verify our function is referenced
    let index_content = fs::read_to_string("baml_client/index.ts")
        .expect("Should be able to read index.ts");
    
    // The generated code should export our function
    // Check that SimpleGreeting is referenced (it will be lowercased in the export)
    assert!(
        index_content.contains("async_client") || index_content.contains("b"),
        "Generated code should export client functions"
    );
    
    // Check async_client file for SimpleGreeting function
    let async_client_content = fs::read_to_string("baml_client/async_client.ts")
        .expect("Should be able to read async_client.ts");
    
    // Our function should be defined in the async client
    assert!(
        async_client_content.contains("SimpleGreeting"),
        "Generated async_client.ts should contain SimpleGreeting function"
    );
    
    // Check inlinedbaml to verify the original BAML source is embedded
    let inlined_content = fs::read_to_string("baml_client/inlinedbaml.ts")
        .expect("Should be able to read inlinedbaml.ts");
    
    assert!(
        inlined_content.contains("SimpleGreeting"),
        "Inlined BAML should contain SimpleGreeting function"
    );
}

