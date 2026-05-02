//! Task lifecycle handlers: get, list, result, cancel.

use crate::protocol::{CancelTaskParams, GetTaskParams, ListTasksParams, TaskResultParams};
use anyhow::{Context, Result};
use serde_json::json;
use tracing::info;

/// Handle tasks/get - retrieve task status.
pub async fn handle_tasks_get(
    server: &crate::server::McpServer,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    let get_params: GetTaskParams =
        serde_json::from_value(params.clone()).context("Failed to parse tasks/get parameters")?;

    let task = server
        .state
        .task_manager
        .get_task(&get_params.task_id)
        .ok_or_else(|| anyhow::anyhow!("Task '{}' not found", get_params.task_id))?;

    info!(
        "tasks/get: task={} state={:?}",
        get_params.task_id, task.state
    );
    Ok(json!({ "task": task }))
}

/// Handle tasks/list - list all tasks.
pub async fn handle_tasks_list(
    server: &crate::server::McpServer,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    let list_params: ListTasksParams = serde_json::from_value(params.clone()).unwrap_or_default();

    let tasks = server
        .state
        .task_manager
        .list_tasks(list_params.states.clone());

    info!(
        "tasks/list: returned {} tasks (filter={:?})",
        tasks.len(),
        list_params.states
    );
    Ok(json!({
        "tasks": tasks,
    }))
}

/// Handle tasks/result - block until task reaches terminal state.
pub async fn handle_tasks_result(
    server: &crate::server::McpServer,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    let result_params: TaskResultParams = serde_json::from_value(params.clone())
        .context("Failed to parse tasks/result parameters")?;

    // Verify task exists
    server
        .state
        .task_manager
        .get_task(&result_params.task_id)
        .ok_or_else(|| anyhow::anyhow!("Task '{}' not found", result_params.task_id))?;

    info!(
        "tasks/result: waiting for task={} (timeout={}s)",
        result_params.task_id, result_params.timeout
    );

    // Try to register a waiter — if task is already terminal, this returns None
    // and the result is delivered via the channel immediately.
    let rx = server
        .state
        .task_manager
        .wait_for_result(&result_params.task_id);

    let result_value = if let Some(receiver) = rx {
        // Block with timeout
        match tokio::time::timeout(
            std::time::Duration::from_secs(result_params.timeout),
            receiver,
        )
        .await
        {
            Ok(Ok(value)) => value,
            Ok(Err(_)) => {
                return Err(anyhow::anyhow!(
                    "Task '{}' result channel closed",
                    result_params.task_id
                ));
            }
            Err(_) => {
                return Err(anyhow::anyhow!(
                    "tasks/result timed out after {} seconds for task '{}'",
                    result_params.timeout,
                    result_params.task_id
                ));
            }
        }
    } else {
        // Already terminal — wait_for_result sent the result directly.
        // We need to get it from the stored result instead.
        let task = server
            .state
            .task_manager
            .get_task(&result_params.task_id)
            .ok_or_else(|| anyhow::anyhow!("Task '{}' not found", result_params.task_id))?;

        // For terminal tasks, return the stored result content.
        if task.state == crate::protocol::TaskState::Completed {
            json!({
                "content": [{ "type": "text", "text": "Task completed" }],
                "isError": false
            })
        } else {
            json!({
                "content": [{ "type": "text", "text": "Task did not complete successfully" }],
                "isError": true
            })
        }
    };

    info!(
        "tasks/result: task={} result returned",
        result_params.task_id
    );
    Ok(result_value)
}

/// Handle tasks/cancel - cancel a running task.
pub async fn handle_tasks_cancel(
    server: &crate::server::McpServer,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    let cancel_params: CancelTaskParams = serde_json::from_value(params.clone())
        .context("Failed to parse tasks/cancel parameters")?;

    // Check task exists and get current state
    let task = server
        .state
        .task_manager
        .get_task(&cancel_params.task_id)
        .ok_or_else(|| anyhow::anyhow!("Task '{}' not found", cancel_params.task_id))?;

    // Reject cancellation for terminal tasks
    if task.state.is_terminal() {
        return Err(anyhow::anyhow!(
            "Cannot cancel task '{}': already in terminal state '{:?}'",
            cancel_params.task_id,
            task.state
        ));
    }

    server
        .state
        .task_manager
        .cancel_task(&cancel_params.task_id);

    let task = server
        .state
        .task_manager
        .get_task(&cancel_params.task_id)
        .ok_or_else(|| anyhow::anyhow!("Task '{}' not found", cancel_params.task_id))?;

    info!("tasks/cancel: task={}", cancel_params.task_id);
    Ok(json!({ "task": task }))
}
