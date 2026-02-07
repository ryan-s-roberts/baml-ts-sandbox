//! Runtime scope attribute helpers for OpenTelemetry spans and provenance.
//!
//! This module provides shared utilities for extracting runtime scope context
//! (context_id, message_id, task_id) for both OTEL spans and provenance events,
//! ensuring semantic alignment between tracing and provenance.

use baml_rt_core::context::{current_context_id, current_message_id, current_task_id};

/// Extract runtime scope attributes for OpenTelemetry spans.
///
/// Returns a tuple of (context_id, message_id, task_id) as strings suitable
/// for span attributes. Uses structured fields following OTEL conventions.
#[inline]
pub fn scope_attributes() -> (Option<String>, Option<String>, Option<String>) {
    (
        current_context_id().map(|id| id.as_str().to_string()),
        current_message_id().map(|id| id.as_str().to_string()),
        current_task_id().map(|id| id.as_str().to_string()),
    )
}

/// Format scope attributes for structured logging.
///
/// Returns a formatted string suitable for log messages, showing
/// which scope identifiers are present.
#[inline]
pub fn scope_summary() -> String {
    let (ctx_id, msg_id, task_id) = scope_attributes();
    let mut parts = Vec::new();
    if let Some(id) = ctx_id {
        parts.push(format!("context_id={}", id));
    }
    if let Some(id) = msg_id {
        parts.push(format!("message_id={}", id));
    }
    if let Some(id) = task_id {
        parts.push(format!("task_id={}", id));
    }
    if parts.is_empty() {
        "no_scope".to_string()
    } else {
        parts.join(", ")
    }
}
