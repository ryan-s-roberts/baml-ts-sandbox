//! Context ID propagation for async invocation flows.
//!
//! This module provides task-local context IDs so async boundaries
//! can retain request context without requiring JS changes.

use crate::ids::{AgentId, ContextId, MessageId, TaskId};
use crate::error::{BamlRtError, Result};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct RuntimeScope {
    pub context_id: ContextId,
    pub agent_id: AgentId,
    pub message_id: Option<MessageId>,
    pub task_id: Option<TaskId>,
}

impl RuntimeScope {
    pub fn new(
        context_id: ContextId,
        agent_id: AgentId,
        message_id: Option<MessageId>,
        task_id: Option<TaskId>,
    ) -> Self {
        Self { context_id, agent_id, message_id, task_id }
    }
}

tokio::task_local! {
    static RUNTIME_SCOPE: RuntimeScope;
}

static CONTEXT_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn generate_context_id() -> ContextId {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let counter = CONTEXT_COUNTER.fetch_add(1, Ordering::Relaxed);
    ContextId::new(millis, counter)
}

pub fn current_scope() -> Option<RuntimeScope> {
    RUNTIME_SCOPE.try_with(|scope| scope.clone()).ok()
}

pub fn current_context_id() -> Option<ContextId> {
    current_scope().map(|scope| scope.context_id)
}

pub fn current_agent_id() -> Option<AgentId> {
    current_scope().map(|scope| scope.agent_id)
}

pub fn current_message_id() -> Option<MessageId> {
    current_scope().and_then(|scope| scope.message_id)
}

pub fn current_task_id() -> Option<TaskId> {
    current_scope().and_then(|scope| scope.task_id)
}

pub fn current_or_new() -> ContextId {
    current_context_id().unwrap_or_else(generate_context_id)
}

pub async fn with_scope<F, T>(scope: RuntimeScope, fut: F) -> T
where
    F: std::future::Future<Output = T>,
{
    RUNTIME_SCOPE.scope(scope, fut).await
}

pub async fn with_context_id<F, T>(id: ContextId, fut: F) -> Result<T>
where
    F: std::future::Future<Output = T>,
{
    let scope = current_scope()
        .map(|mut scope| {
            scope.context_id = id.clone();
            scope
        })
        .ok_or_else(|| {
            BamlRtError::InvalidArgument(
                "RuntimeScope must have agent_id - cannot create scope without agent context".to_string()
            )
        })?;
    Ok(with_scope(scope, fut).await)
}

pub async fn with_message_id<F, T>(id: MessageId, fut: F) -> Result<T>
where
    F: std::future::Future<Output = T>,
{
    let scope = current_scope()
        .ok_or_else(|| {
            BamlRtError::InvalidArgument(
                "RuntimeScope must exist with agent_id - cannot create scope without agent context".to_string()
            )
        })?;
    let scope = RuntimeScope::new(scope.context_id, scope.agent_id, Some(id), scope.task_id);
    Ok(with_scope(scope, fut).await)
}

pub async fn with_task_id<F, T>(id: TaskId, fut: F) -> Result<T>
where
    F: std::future::Future<Output = T>,
{
    let scope = current_scope()
        .ok_or_else(|| {
            BamlRtError::InvalidArgument(
                "RuntimeScope must exist with agent_id - cannot create scope without agent context".to_string()
            )
        })?;
    let scope = RuntimeScope::new(scope.context_id, scope.agent_id, scope.message_id, Some(id));
    Ok(with_scope(scope, fut).await)
}

pub async fn with_agent_id<F, T>(id: AgentId, fut: F) -> T
where
    F: std::future::Future<Output = T>,
{
    let scope = current_scope()
        .map(|mut scope| {
            scope.agent_id = id.clone();
            scope
        })
        .unwrap_or_else(|| RuntimeScope::new(generate_context_id(), id, None, None));
    with_scope(scope, fut).await
}
