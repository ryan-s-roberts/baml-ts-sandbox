//! Error types for the BAML runtime integration

use thiserror::Error;

#[derive(Error, Debug)]
pub enum BamlRtError {
    #[error("BAML runtime error: {0}")]
    BamlRuntime(String),

    #[error("QuickJS error: {0}")]
    QuickJs(String),

    #[error("Type conversion error: {0}")]
    TypeConversion(String),

    #[error("Function not found: {0}")]
    FunctionNotFound(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, BamlRtError>;

