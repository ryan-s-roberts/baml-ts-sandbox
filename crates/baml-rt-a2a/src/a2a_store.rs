use crate::a2a_types::{
    Artifact, ListTasksRequest, ListTasksResponse, Message, Task, TaskArtifactUpdateEvent,
    TaskState, TaskStatus, TaskStatusUpdateEvent, TASK_STATE_CANCELED,
};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum TaskUpdateEvent {
    Status(TaskStatusUpdateEvent),
    Artifact(TaskArtifactUpdateEvent),
}

impl TaskUpdateEvent {
    pub fn task_id(&self) -> Option<&str> {
        match self {
            TaskUpdateEvent::Status(event) => event.task_id.as_deref(),
            TaskUpdateEvent::Artifact(event) => event.task_id.as_deref(),
        }
    }
}

#[derive(Debug, Default)]
pub struct TaskStore {
    tasks: HashMap<String, Task>,
    order: Vec<String>,
    updates: HashMap<String, Vec<TaskUpdateEvent>>,
}

impl TaskStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert(&mut self, task: Task) -> Option<Task> {
        let id = task.id.clone()?;
        if !self.tasks.contains_key(&id) {
            self.order.push(id.clone());
        }
        self.tasks.insert(id.clone(), task.clone());
        Some(task)
    }

    pub fn get(&self, id: &str, history_length: Option<usize>) -> Option<Task> {
        let mut task = self.tasks.get(id).cloned()?;
        if let Some(limit) = history_length {
            truncate_history(&mut task, limit);
        }
        Some(task)
    }

    pub fn list(&self, request: &ListTasksRequest) -> ListTasksResponse {
        let mut tasks: Vec<Task> = self
            .order
            .iter()
            .filter_map(|id| self.tasks.get(id).cloned())
            .collect();

        if let Some(context_id) = &request.context_id {
            tasks.retain(|task| task.context_id.as_deref() == Some(context_id.as_str()));
        }

        if let Some(status) = &request.status {
            tasks.retain(|task| matches_task_state(task, status));
        }

        let include_artifacts = request.include_artifacts.unwrap_or(false);
        if !include_artifacts {
            for task in &mut tasks {
                task.artifacts.clear();
            }
        }

        if let Some(limit) = request.history_length.as_ref().and_then(|value| value.as_usize()) {
            for task in &mut tasks {
                truncate_history(task, limit);
            }
        }

        let total_size = tasks.len() as u64;
        let page_size = request
            .page_size
            .as_ref()
            .and_then(|value| value.as_usize())
            .unwrap_or(50);
        let start = request
            .page_token
            .as_ref()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        let end = usize::min(start + page_size, tasks.len());

        let page_tasks = if start < tasks.len() {
            tasks[start..end].to_vec()
        } else {
            Vec::new()
        };

        let next_page_token = if end < tasks.len() {
            Some(end.to_string())
        } else {
            None
        };

        ListTasksResponse {
            tasks: page_tasks,
            next_page_token,
            total_size: Some(total_size),
            page_size: Some(page_size as u64),
            extra: HashMap::new(),
        }
    }

    pub fn cancel(&mut self, id: &str) -> Option<Task> {
        let task = self.tasks.get_mut(id)?;
        let status = task.status.get_or_insert_with(TaskStatus::default);
        status.state = Some(TaskState::String(TASK_STATE_CANCELED.to_string()));
        Some(task.clone())
    }

    pub fn insert_message(&mut self, message: &Message) {
        if let Some(task_id) = &message.task_id
            && let Some(task) = self.tasks.get_mut(task_id)
        {
            task.history.push(message.clone());
        }
    }

    pub fn record_status_update(
        &mut self,
        task_id: Option<String>,
        context_id: Option<String>,
        status: TaskStatus,
    ) -> Option<TaskUpdateEvent> {
        if let Some(task_id) = task_id {
            let update = TaskStatusUpdateEvent {
                context_id,
                task_id: Some(task_id.clone()),
                status: Some(status),
                metadata: None,
                extra: HashMap::new(),
            };
            let event = TaskUpdateEvent::Status(update.clone());
            self.updates
                .entry(task_id)
                .or_default()
                .push(event.clone());
            return Some(event);
        }
        None
    }

    pub fn record_artifact_update(
        &mut self,
        task_id: Option<String>,
        context_id: Option<String>,
        artifact: Artifact,
        append: Option<bool>,
        last_chunk: Option<bool>,
    ) -> Option<TaskUpdateEvent> {
        if let Some(task_id) = task_id {
            let update = TaskArtifactUpdateEvent {
                context_id,
                task_id: Some(task_id.clone()),
                last_chunk,
                append,
                artifact: Some(artifact),
                metadata: None,
                extra: HashMap::new(),
            };
            let event = TaskUpdateEvent::Artifact(update.clone());
            self.updates
                .entry(task_id)
                .or_default()
                .push(event.clone());
            return Some(event);
        }
        None
    }

    pub fn drain_updates(&mut self, task_id: &str) -> Vec<TaskUpdateEvent> {
        self.updates.remove(task_id).unwrap_or_default()
    }
}

fn truncate_history(task: &mut Task, limit: usize) {
    if limit == 0 {
        task.history.clear();
        return;
    }
    if task.history.len() > limit {
        let start = task.history.len() - limit;
        task.history = task.history.split_off(start);
    }
}

fn matches_task_state(task: &Task, desired: &TaskState) -> bool {
    let Some(status) = &task.status else {
        return false;
    };
    let Some(state) = &status.state else {
        return false;
    };
    match (state, desired) {
        (TaskState::String(current), TaskState::String(target)) => current == target,
        (TaskState::Integer(current), TaskState::Integer(target)) => current == target,
        _ => false,
    }
}
