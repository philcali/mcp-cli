//! Task state management for MCP tasks.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;
use tracing::debug;

use crate::protocol::{Task, TaskResult, TaskState};

/// Internal entry holding task metadata, result, and waiter.
struct TaskEntry {
    task: Task,
    result: Option<TaskResult>,
    waiter: Option<tokio::sync::oneshot::Sender<serde_json::Value>>,
}

/// In-memory manager for task lifecycle.
pub struct TaskManager {
    tasks: Mutex<HashMap<String, TaskEntry>>,
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            tasks: Mutex::new(HashMap::new()),
        }
    }

    fn generate_id() -> String {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("task_{}", ts)
    }

    fn now_epoch_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// Convert epoch seconds to ISO 8601 (UTC).
    fn epoch_to_iso8601(secs: u64) -> String {
        // Days since Unix epoch
        let days = secs / 86400;
        let mut remainder = secs % 86400;
        let hours = remainder / 3600;
        remainder %= 3600;
        let minutes = remainder / 60;
        let seconds = remainder % 60;

        // Calculate year, month, day from days since epoch (simplified)
        let (year, month, day) = days_to_ymd(days);

        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            year, month, day, hours, minutes, seconds
        )
    }

    fn now_iso8601() -> String {
        Self::epoch_to_iso8601(Self::now_epoch_secs())
    }

    /// Create a new task and return its ID.
    pub fn create_task(&self, method: &str, ttl: Option<u64>) -> String {
        let id = Self::generate_id();
        let now = Self::now_iso8601();
        let task = Task {
            task_id: id.clone(),
            state: TaskState::Working,
            status_message: Some("Task started".to_string()),
            created_at: now.clone(),
            last_updated_at: now,
            ttl,
            poll_interval: Some(1000),
        };
        let entry = TaskEntry {
            task,
            result: None,
            waiter: None,
        };
        self.tasks.lock().unwrap().insert(id.clone(), entry);
        debug!("Created task {} for method {}", id, method);
        id
    }

    /// Get a task by ID.
    pub fn get_task(&self, task_id: &str) -> Option<Task> {
        self.tasks
            .lock()
            .unwrap()
            .get(task_id)
            .map(|e| e.task.clone())
    }

    /// List tasks, optionally filtered by state.
    pub fn list_tasks(&self, states: Option<Vec<TaskState>>) -> Vec<Task> {
        let tasks = self.tasks.lock().unwrap();
        let mut result: Vec<Task> = tasks.values().map(|e| e.task.clone()).collect();
        if let Some(filter_states) = states {
            result.retain(|t| filter_states.contains(&t.state));
        }
        result
    }

    /// Mark a task as completed and store the result.
    pub fn complete_task(&self, task_id: &str, result: serde_json::Value) {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(entry) = tasks.get_mut(task_id) {
            entry.task.state = TaskState::Completed;
            entry.task.status_message = Some("Task completed successfully".to_string());
            entry.task.last_updated_at = Self::now_iso8601();
            entry.result = Some(TaskResult {
                result: result.clone(),
                error: None,
            });
            if let Some(tx) = entry.waiter.take() {
                let _ = tx.send(result);
            }
        }
        drop(tasks);
        debug!("Task {} completed", task_id);
    }

    /// Mark a task as failed.
    pub fn fail_task(&self, task_id: &str, error: serde_json::Value) {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(entry) = tasks.get_mut(task_id) {
            entry.task.state = TaskState::Failed;
            entry.task.status_message = error
                .get("error")
                .and_then(|v| v.as_str())
                .map(String::from)
                .or_else(|| Some("Task failed".to_string()));
            entry.task.last_updated_at = Self::now_iso8601();
            entry.result = Some(TaskResult {
                result: serde_json::Value::Null,
                error: Some(error.clone()),
            });
            if let Some(tx) = entry.waiter.take() {
                let _ = tx.send(error);
            }
        }
        drop(tasks);
        debug!("Task {} failed", task_id);
    }

    /// Cancel a task. Returns true if the task was successfully cancelled.
    pub fn cancel_task(&self, task_id: &str) -> bool {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(entry) = tasks.get_mut(task_id)
            && entry.task.state == TaskState::Working
        {
            entry.task.state = TaskState::Cancelled;
            entry.task.status_message = Some("Task cancelled by request".to_string());
            entry.task.last_updated_at = Self::now_iso8601();
            if let Some(tx) = entry.waiter.take() {
                let _ = tx.send(json!({ "cancelled": true }));
            }
            debug!("Task {} cancelled", task_id);
            return true;
        }
        false
    }

    /// Register a waiter for tasks/result.
    /// Returns None if task already terminal (result delivered via channel).
    /// Returns Some(rx) if task still working — caller blocks on rx.
    pub fn wait_for_result(
        &self,
        task_id: &str,
    ) -> Option<tokio::sync::oneshot::Receiver<serde_json::Value>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let mut tasks = self.tasks.lock().unwrap();

        let entry = tasks.get_mut(task_id)?;

        // If already terminal, resolve immediately
        if entry.task.state.is_terminal() {
            if let Some(ref task_result) = entry.result {
                if task_result.error.is_some() {
                    let _ = tx.send(task_result.error.clone().unwrap());
                } else {
                    let _ = tx.send(task_result.result.clone());
                }
            }
            return None;
        }

        // Register the waiter
        entry.waiter = Some(tx);
        Some(rx)
    }

    /// Clean up tasks whose TTL has elapsed.
    /// Returns the number of cleaned tasks.
    pub fn cleanup_expired(&self) -> usize {
        let now = Self::now_epoch_secs();
        let mut tasks = self.tasks.lock().unwrap();
        let expired_ids: Vec<String> = tasks
            .iter()
            .filter_map(|(id, entry)| {
                if let Some(ttl_ms) = entry.task.ttl {
                    let ttl_secs = ttl_ms / 1000;
                    let created_secs = iso8601_to_epoch(&entry.task.created_at);
                    if now.saturating_sub(created_secs) > ttl_secs {
                        return Some(id.clone());
                    }
                }
                None
            })
            .collect();

        let count = expired_ids.len();
        for id in expired_ids {
            tasks.remove(&id);
        }
        if count > 0 {
            debug!("Cleaned up {} expired tasks", count);
        }
        count
    }
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}
fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    let mut y: i32 = 1970;
    let mut d = days as i32;

    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if d < days_in_year {
            break;
        }
        d -= days_in_year;
        y += 1;
    }

    let months = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut m = 1;
    for &dim in &months {
        if d < dim {
            break;
        }
        d -= dim;
        m += 1;
    }

    (y as u64, m as u64, (d + 1) as u64)
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

/// Parse an ISO 8601 timestamp like "2025-01-12T15:00:58Z" to epoch seconds (best effort).
fn iso8601_to_epoch(ts: &str) -> u64 {
    // Try to parse YYYY-MM-DDTHH:MM:SSZ
    if let Some(idx) = ts.find('T') {
        let date_part = &ts[..idx];
        let time_part = &ts[idx + 1..];

        let date: Vec<i32> = date_part
            .split('-')
            .filter_map(|s| s.parse().ok())
            .collect();
        let time: Vec<i32> = time_part
            .replace('Z', "")
            .split(':')
            .filter_map(|s| s.parse().ok())
            .collect();

        if date.len() >= 3 && time.len() >= 3 {
            let (y, m, d) = (date[0], date[1], date[2]);
            let (hh, mm, ss) = (time[0], time[1], time[2]);

            let days = date_to_days(y, m, d);
            days as u64 * 86400 + hh as u64 * 3600 + mm as u64 * 60 + ss as u64
        } else {
            0
        }
    } else {
        0
    }
}

/// Convert a date to days since Unix epoch.
fn date_to_days(y: i32, m: i32, d: i32) -> i64 {
    let mut total = 0i64;
    for year in 1970..y {
        total += if is_leap(year) { 366 } else { 365 };
    }
    let months = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    for month in 1..m {
        let idx = (month - 1) as usize;
        if idx < months.len() {
            total += months[idx] as i64;
        }
    }
    total += (d - 1) as i64;
    total
}

/// Convert days since Unix epoch (1970-01-01) to (year, month, day).
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_state_is_terminal() {
        assert!(!TaskState::Working.is_terminal());
        assert!(!TaskState::InputRequired.is_terminal());
        assert!(TaskState::Completed.is_terminal());
        assert!(TaskState::Failed.is_terminal());
        assert!(TaskState::Cancelled.is_terminal());
    }

    #[test]
    fn test_create_and_get_task() {
        let mgr = TaskManager::new();
        let id = mgr.create_task("tools/call", None);

        assert!(id.starts_with("task_"));

        let task = mgr.get_task(&id).expect("task should exist");
        assert_eq!(task.task_id, id);
        assert_eq!(task.state, TaskState::Working);
        assert!(!task.created_at.is_empty());
        assert!(!task.last_updated_at.is_empty());
        assert_eq!(task.ttl, None);
        assert_eq!(task.poll_interval, Some(1000));
    }

    #[test]
    fn test_create_task_with_ttl() {
        let mgr = TaskManager::new();
        let id = mgr.create_task("tools/call", Some(5000));

        let task = mgr.get_task(&id).expect("task should exist");
        assert_eq!(task.ttl, Some(5000));
    }

    #[test]
    fn test_get_nonexistent_task() {
        let mgr = TaskManager::new();
        assert!(mgr.get_task("nonexistent").is_none());
    }

    #[test]
    fn test_list_tasks_empty() {
        let mgr = TaskManager::new();
        let tasks = mgr.list_tasks(None);
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_list_tasks_with_filter() {
        let mgr = TaskManager::new();
        mgr.create_task("m1", None);
        mgr.create_task("m2", None);

        // Complete one
        let id3 = mgr.create_task("m3", None);
        mgr.complete_task(&id3, json!({ "ok": true }));

        let all = mgr.list_tasks(None);
        assert_eq!(all.len(), 3);

        let working: Vec<Task> = mgr.list_tasks(Some(vec![TaskState::Working]));
        assert_eq!(working.len(), 2);
        assert!(working.iter().all(|t| t.state == TaskState::Working));

        let completed: Vec<Task> = mgr.list_tasks(Some(vec![TaskState::Completed]));
        assert_eq!(completed.len(), 1);
    }

    #[test]
    fn test_complete_task() {
        let mgr = TaskManager::new();
        let id = mgr.create_task("tools/call", None);

        mgr.complete_task(&id, json!({ "data": "result" }));

        let task = mgr.get_task(&id).expect("task should exist");
        assert_eq!(task.state, TaskState::Completed);
        assert!(task.status_message.as_ref().unwrap().contains("completed"));
    }

    #[test]
    fn test_fail_task() {
        let mgr = TaskManager::new();
        let id = mgr.create_task("tools/call", None);

        mgr.fail_task(&id, json!({ "error": "something broke" }));

        let task = mgr.get_task(&id).expect("task should exist");
        assert_eq!(task.state, TaskState::Failed);
        assert_eq!(
            task.status_message.as_ref().unwrap().as_str(),
            "something broke"
        );
    }

    #[test]
    fn test_fail_task_without_error_message() {
        let mgr = TaskManager::new();
        let id = mgr.create_task("tools/call", None);

        mgr.fail_task(&id, json!({ "code": 500 }));

        let task = mgr.get_task(&id).expect("task should exist");
        assert_eq!(task.state, TaskState::Failed);
        assert_eq!(
            task.status_message.as_ref().unwrap().as_str(),
            "Task failed"
        );
    }

    #[test]
    fn test_cancel_working_task() {
        let mgr = TaskManager::new();
        let id = mgr.create_task("tools/call", None);

        let result = mgr.cancel_task(&id);
        assert!(result);

        let task = mgr.get_task(&id).expect("task should exist");
        assert_eq!(task.state, TaskState::Cancelled);
    }

    #[test]
    fn test_cancel_terminal_task_fails() {
        let mgr = TaskManager::new();
        let id = mgr.create_task("tools/call", None);
        mgr.complete_task(&id, json!({ "ok": true }));

        let result = mgr.cancel_task(&id);
        assert!(!result);

        let task = mgr.get_task(&id).expect("task should exist");
        assert_eq!(task.state, TaskState::Completed);
    }

    #[test]
    fn test_cancel_nonexistent_task() {
        let mgr = TaskManager::new();
        assert!(!mgr.cancel_task("nonexistent"));
    }

    #[tokio::test]
    async fn test_wait_for_result_task_still_working() {
        let mgr = TaskManager::new();
        let id = mgr.create_task("tools/call", None);

        let rx = mgr.wait_for_result(&id).expect("should return receiver");

        // Complete from another reference
        mgr.complete_task(&id, json!({ "answer": 42 }));

        let result = rx.await.expect("channel should not be closed");
        assert_eq!(result, json!({ "answer": 42 }));
    }

    #[tokio::test]
    async fn test_wait_for_result_already_completed() {
        let mgr = TaskManager::new();
        let id = mgr.create_task("tools/call", None);
        mgr.complete_task(&id, json!({ "done": true }));

        // Should return None for terminal tasks
        let rx = mgr.wait_for_result(&id);
        assert!(rx.is_none());
    }

    #[tokio::test]
    async fn test_wait_for_result_nonexistent_task() {
        let mgr = TaskManager::new();
        let rx = mgr.wait_for_result("nonexistent");
        assert!(rx.is_none());
    }

    #[test]
    fn test_cleanup_expired_tasks() {
        let mgr = TaskManager::new();

        // Task with no TTL should never expire
        mgr.create_task("m1", None);

        // Task with very large TTL should not expire
        mgr.create_task("m2", Some(u64::MAX));

        let cleaned = mgr.cleanup_expired();
        assert_eq!(cleaned, 0);

        let tasks = mgr.list_tasks(None);
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn test_cleanup_no_tasks() {
        let mgr = TaskManager::new();
        let cleaned = mgr.cleanup_expired();
        assert_eq!(cleaned, 0);
    }

    #[test]
    fn test_complete_nonexistent_task_no_panic() {
        let mgr = TaskManager::new();
        mgr.complete_task("nonexistent", json!({ "ok": true }));
        // Should not panic, just silently ignore
    }

    #[test]
    fn test_fail_nonexistent_task_no_panic() {
        let mgr = TaskManager::new();
        mgr.fail_task("nonexistent", json!({ "error": "nope" }));
        // Should not panic, just silently ignore
    }
}
