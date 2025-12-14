//! Error types for the BAML runtime integration
//!
//! Provides a comprehensive error hierarchy using `thiserror` for proper error handling
//! and error chaining throughout the codebase.

use thiserror::Error;

/// Main error type for the BAML runtime integration
#[derive(Error, Debug)]
pub enum BamlRtError {
    /// BAML runtime execution error
    #[error("BAML runtime error: {0}")]
    BamlRuntime(String),

    /// QuickJS JavaScript engine error
    #[error("QuickJS error: {0}")]
    QuickJs(String),

    /// Type conversion error between Rust and JavaScript types
    #[error("Type conversion error: {0}")]
    TypeConversion(String),

    /// Function not found in registry
    #[error("Function not found: {0}")]
    FunctionNotFound(String),

    /// Invalid argument provided to a function
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    /// I/O error (file operations, etc.)
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Tool execution error
    #[error("Tool execution error: {0}")]
    ToolExecution(String),

    /// Tool registration error
    #[error("Tool registration error: {0}")]
    ToolRegistration(String),

    /// Schema loading error
    #[error("Schema loading error: {0}")]
    SchemaLoading(String),

    /// Runtime configuration error
    #[error("Runtime configuration error: {0}")]
    Configuration(String),

    /// Runtime initialization error
    #[error("Runtime initialization error: {0}")]
    Initialization(String),
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, BamlRtError>;
