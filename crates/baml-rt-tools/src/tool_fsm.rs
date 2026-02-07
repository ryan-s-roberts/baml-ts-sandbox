//! Tool session FSM primitives.

use baml_rt_core::BamlRtError;
use async_trait::async_trait;
use serde_json::Value;
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolSessionId(String);

impl ToolSessionId {
    pub fn new(id: impl Into<String>) -> std::result::Result<Self, BamlRtError> {
        let value = id.into();
        Uuid::parse_str(&value).map_err(|_| {
            BamlRtError::InvalidArgument(format!("Invalid tool session id '{}'", value))
        })?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ToolSessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolFailureKind {
    InvalidInput,
    ExecutionFailed,
    NotAuthorized,
    RateLimited,
    Cancelled,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ToolFailure {
    pub kind: ToolFailureKind,
    pub message: String,
    pub retryable: bool,
}

impl ToolFailure {
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self {
            kind: ToolFailureKind::InvalidInput,
            message: message.into(),
            retryable: false,
        }
    }

    pub fn execution_failed(message: impl Into<String>) -> Self {
        Self {
            kind: ToolFailureKind::ExecutionFailed,
            message: message.into(),
            retryable: false,
        }
    }

    pub fn from_error(error: &BamlRtError) -> Self {
        let kind = match error {
            BamlRtError::InvalidArgument(_) | BamlRtError::InvalidArgumentWithSource { .. } => {
                ToolFailureKind::InvalidInput
            }
            BamlRtError::QuickJs(_) | BamlRtError::QuickJsWithSource { .. } => {
                ToolFailureKind::ExecutionFailed
            }
            BamlRtError::ToolExecution(_) => ToolFailureKind::ExecutionFailed,
            _ => ToolFailureKind::Unknown,
        };
        Self {
            kind,
            message: error.to_string(),
            retryable: false,
        }
    }
}

#[derive(Debug)]
pub enum ToolSessionError {
    Transport(BamlRtError),
    Tool(ToolFailure),
}

impl From<BamlRtError> for ToolSessionError {
    fn from(error: BamlRtError) -> Self {
        ToolSessionError::Transport(error)
    }
}

#[derive(Debug, Clone)]
pub enum ToolStep {
    Streaming { output: Value },
    Done { output: Option<Value> },
    Error { error: ToolFailure },
}

#[async_trait]
pub trait ToolSession: Send + Sync {
    async fn send(&mut self, input: Value) -> std::result::Result<(), ToolSessionError>;
    async fn next(&mut self) -> std::result::Result<ToolStep, ToolSessionError>;
    async fn finish(&mut self) -> std::result::Result<(), ToolSessionError>;
    async fn abort(&mut self, reason: Option<String>) -> std::result::Result<(), ToolSessionError>;
}
