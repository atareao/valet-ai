use crate::tools::permission::Permission;
use crate::tools::registry::ToolRegistry;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::oneshot;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
}

#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub reason: String,
    pub status: ApprovalStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub enum GuardrailResult {
    Allowed { notify: bool },
    RequiresApproval { request_id: String },
}

/// Outcome of a turn waiting on a human-in-the-loop approval decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalOutcome {
    /// The user approved the call.
    Approved,
    /// The user explicitly denied the call.
    Denied,
    /// The timeout elapsed before a decision was made.
    TimedOut,
    /// The waiting turn disappeared (receiver dropped) or no channel existed.
    Cancelled,
}

/// A pending approval: the public request plus the channel used to signal the
/// waiting turn. `responder` is consumed by `resolve_approval`; `receiver` is
/// consumed by `await_approval`.
struct PendingApproval {
    request: ApprovalRequest,
    responder: Option<oneshot::Sender<bool>>,
    receiver: Option<oneshot::Receiver<bool>>,
}

/// RAII cleanup for a pending approval: removes the entry from the registry when
/// dropped. Created inside `await_approval` so the entry is released on every
/// exit path — including when the waiting future is dropped (cancelled/aborted)
/// before it completes.
struct EntryGuard {
    store: Arc<Mutex<HashMap<String, PendingApproval>>>,
    id: String,
}

impl Drop for EntryGuard {
    fn drop(&mut self) {
        if let Ok(mut map) = self.store.lock() {
            map.remove(&self.id);
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GuardrailError {
    #[error("Tool not found: {0}")]
    ToolNotFound(String),
    #[error("Approval request not found: {0}")]
    RequestNotFound(String),
    #[error("Already resolved")]
    AlreadyResolved,
}

pub struct Guardrails {
    registry: Arc<ToolRegistry>,
    pending_approvals: Arc<Mutex<HashMap<String, PendingApproval>>>,
}

impl Guardrails {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self {
            registry,
            pending_approvals: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn check(
        &self,
        tool_name: &str,
        args: &serde_json::Value,
    ) -> Result<GuardrailResult, GuardrailError> {
        let perm = self
            .registry
            .permission(tool_name, args)
            .ok_or_else(|| GuardrailError::ToolNotFound(tool_name.to_string()))?;
        match perm {
            Permission::NoConfirm => Ok(GuardrailResult::Allowed { notify: false }),
            Permission::Notify => Ok(GuardrailResult::Allowed { notify: true }),
            Permission::ExplicitApproval => {
                let request = self.create_approval_request(tool_name, args.clone());
                Ok(GuardrailResult::RequiresApproval {
                    request_id: request.id.clone(),
                })
            }
        }
    }

    fn create_approval_request(&self, tool_name: &str, args: serde_json::Value) -> ApprovalRequest {
        let request = ApprovalRequest {
            id: uuid::Uuid::new_v4().to_string(),
            tool_name: tool_name.to_string(),
            arguments: args,
            reason: format!("Tool '{}' requires explicit approval", tool_name),
            status: ApprovalStatus::Pending,
            created_at: Utc::now(),
        };
        let (responder, receiver) = oneshot::channel();
        let entry = PendingApproval {
            request: request.clone(),
            responder: Some(responder),
            receiver: Some(receiver),
        };
        let mut pending = self.pending_approvals.lock().unwrap();
        pending.insert(request.id.clone(), entry);
        request
    }

    pub fn resolve_approval(&self, request_id: &str, approved: bool) -> Result<(), GuardrailError> {
        let mut pending = self.pending_approvals.lock().unwrap();
        let entry = pending
            .get_mut(request_id)
            .ok_or_else(|| GuardrailError::RequestNotFound(request_id.to_string()))?;
        if !matches!(entry.request.status, ApprovalStatus::Pending) {
            return Err(GuardrailError::AlreadyResolved);
        }
        entry.request.status = if approved {
            ApprovalStatus::Approved
        } else {
            ApprovalStatus::Denied
        };
        // Signal the waiting turn. If the receiver is already gone (client
        // disconnected), there is nothing to notify — the entry is kept so a
        // second resolution still reports `AlreadyResolved` (409).
        if let Some(responder) = entry.responder.take() {
            let _ = responder.send(approved);
        }
        Ok(())
    }

    pub fn get_pending_approval(&self, request_id: &str) -> Option<ApprovalRequest> {
        let pending = self.pending_approvals.lock().unwrap();
        pending.get(request_id).map(|entry| entry.request.clone())
    }

    /// Wait for the human decision on `request_id`, up to `timeout`.
    ///
    /// Takes the `oneshot::Receiver` out of the registry, releases the lock and
    /// awaits it. Missing entry or already-consumed receiver yield `Cancelled`.
    /// The entry is removed on every exit path (decision or timeout), and also
    /// if this future is dropped before completing, so no pending request is
    /// ever leaked.
    pub async fn await_approval(&self, request_id: &str, timeout: Duration) -> ApprovalOutcome {
        let receiver = {
            let mut pending = self.pending_approvals.lock().unwrap();
            pending
                .get_mut(request_id)
                .and_then(|entry| entry.receiver.take())
        };

        // Remove the entry on every exit path. Constructed *after* taking the
        // receiver (and the lock has been released) so it never holds the
        // `MutexGuard` across the `await`, yet still fires if this future is
        // dropped mid-await. The removal is idempotent.
        let _guard = EntryGuard {
            store: self.pending_approvals.clone(),
            id: request_id.to_string(),
        };

        match receiver {
            None => ApprovalOutcome::Cancelled,
            Some(rx) => match tokio::time::timeout(timeout, rx).await {
                Ok(Ok(true)) => ApprovalOutcome::Approved,
                Ok(Ok(false)) => ApprovalOutcome::Denied,
                Ok(Err(_)) => ApprovalOutcome::Cancelled,
                Err(_) => ApprovalOutcome::TimedOut,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::calendar::CalendarTool;
    use crate::tools::permission::Permission;
    use crate::tools::r#trait::{Tool, ToolError as ToolErr, ToolResult};
    use crate::tools::tasks::TasksTool;
    use async_trait::async_trait;
    use sqlx::SqlitePool;

    struct NoConfirmTool;
    #[async_trait]
    impl Tool for NoConfirmTool {
        fn name(&self) -> &'static str {
            "read_tool"
        }
        fn description(&self) -> &'static str {
            "Read only"
        }
        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }
        fn permission(&self, _args: &serde_json::Value) -> Permission {
            Permission::NoConfirm
        }
        async fn execute(&self, _args: serde_json::Value) -> Result<ToolResult, ToolErr> {
            Ok(ToolResult {
                success: true,
                data: serde_json::json!({}),
                message: None,
            })
        }
    }

    struct ApproveTool;
    #[async_trait]
    impl Tool for ApproveTool {
        fn name(&self) -> &'static str {
            "delete_tool"
        }
        fn description(&self) -> &'static str {
            "Deletes things"
        }
        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }
        fn permission(&self, _args: &serde_json::Value) -> Permission {
            Permission::ExplicitApproval
        }
        async fn execute(&self, _args: serde_json::Value) -> Result<ToolResult, ToolErr> {
            Ok(ToolResult {
                success: true,
                data: serde_json::json!({}),
                message: None,
            })
        }
    }

    fn make_registry() -> Arc<ToolRegistry> {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(NoConfirmTool));
        reg.register(Box::new(ApproveTool));
        Arc::new(reg)
    }

    #[test]
    fn test_no_confirm_passes() {
        let g = Guardrails::new(make_registry());
        let result = g.check("read_tool", &serde_json::json!({})).unwrap();
        match result {
            GuardrailResult::Allowed { notify } => assert!(!notify),
            _ => panic!("Expected Allowed"),
        }
    }

    #[test]
    fn test_explicit_approval_requires_approval() {
        let g = Guardrails::new(make_registry());
        let result = g.check("delete_tool", &serde_json::json!({})).unwrap();
        match result {
            GuardrailResult::RequiresApproval { request_id } => {
                assert!(!request_id.is_empty());
                let req = g.get_pending_approval(&request_id).unwrap();
                assert_eq!(req.tool_name, "delete_tool");
                assert!(matches!(req.status, ApprovalStatus::Pending));
            }
            _ => panic!("Expected RequiresApproval"),
        }
    }

    #[test]
    fn test_resolve_approval_approved() {
        let g = Guardrails::new(make_registry());
        let result = g.check("delete_tool", &serde_json::json!({})).unwrap();
        if let GuardrailResult::RequiresApproval { request_id } = result {
            g.resolve_approval(&request_id, true).unwrap();
            let req = g.get_pending_approval(&request_id).unwrap();
            assert!(matches!(req.status, ApprovalStatus::Approved));
        }
    }

    #[test]
    fn test_resolve_approval_denied() {
        let g = Guardrails::new(make_registry());
        let result = g.check("delete_tool", &serde_json::json!({})).unwrap();
        if let GuardrailResult::RequiresApproval { request_id } = result {
            g.resolve_approval(&request_id, false).unwrap();
            let req = g.get_pending_approval(&request_id).unwrap();
            assert!(matches!(req.status, ApprovalStatus::Denied));
        }
    }

    #[test]
    fn test_unknown_tool_returns_error() {
        let g = Guardrails::new(make_registry());
        let result = g.check("nonexistent", &serde_json::json!({}));
        assert!(matches!(result, Err(GuardrailError::ToolNotFound(_))));
    }

    #[test]
    fn test_double_resolve_returns_error() {
        let g = Guardrails::new(make_registry());
        let result = g.check("delete_tool", &serde_json::json!({})).unwrap();
        if let GuardrailResult::RequiresApproval { request_id } = result {
            g.resolve_approval(&request_id, true).unwrap();
            let err = g.resolve_approval(&request_id, true).unwrap_err();
            assert!(matches!(err, GuardrailError::AlreadyResolved));
        }
    }

    #[test]
    fn test_notify_allowed() {
        let mut reg = ToolRegistry::new();
        struct NotifyTool;
        #[async_trait]
        impl Tool for NotifyTool {
            fn name(&self) -> &'static str {
                "notify_tool"
            }
            fn description(&self) -> &'static str {
                "Notify tool"
            }
            fn parameters(&self) -> serde_json::Value {
                serde_json::json!({"type": "object"})
            }
            fn permission(&self, _args: &serde_json::Value) -> Permission {
                Permission::Notify
            }
            async fn execute(&self, _args: serde_json::Value) -> Result<ToolResult, ToolErr> {
                Ok(ToolResult {
                    success: true,
                    data: serde_json::json!({}),
                    message: None,
                })
            }
        }
        reg.register(Box::new(NotifyTool));
        let g = Guardrails::new(Arc::new(reg));
        let result = g.check("notify_tool", &serde_json::json!({})).unwrap();
        match result {
            GuardrailResult::Allowed { notify } => assert!(notify),
            _ => panic!("Expected Allowed with notify"),
        }
    }

    #[tokio::test]
    async fn test_calendar_delete_event_requires_approval() {
        let pool = SqlitePool::connect_lazy("sqlite::memory:").expect("lazy pool");
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(CalendarTool::new(pool)));
        let g = Guardrails::new(Arc::new(reg));
        let result = g
            .check(
                "calendar",
                &serde_json::json!({"operation": "delete_event", "id": "evt-1"}),
            )
            .unwrap();
        assert!(matches!(result, GuardrailResult::RequiresApproval { .. }));
    }

    #[tokio::test]
    async fn test_tasks_delete_task_requires_approval() {
        let pool = SqlitePool::connect_lazy("sqlite::memory:").expect("lazy pool");
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(TasksTool::new(pool)));
        let g = Guardrails::new(Arc::new(reg));
        let result = g
            .check(
                "tasks",
                &serde_json::json!({"operation": "delete_task", "id": "task-1"}),
            )
            .unwrap();
        assert!(matches!(result, GuardrailResult::RequiresApproval { .. }));
    }

    // ------------------------------------------------------------------
    // await_approval — pause/resume with timeout
    // ------------------------------------------------------------------

    /// Helper: create a `delete_tool` approval, returning its `request_id`.
    fn pending_delete_request(g: &Guardrails) -> String {
        match g.check("delete_tool", &serde_json::json!({})).unwrap() {
            GuardrailResult::RequiresApproval { request_id } => request_id,
            _ => panic!("Expected RequiresApproval"),
        }
    }

    #[tokio::test]
    async fn test_await_approval_approved() {
        let g = Arc::new(Guardrails::new(make_registry()));
        let request_id = pending_delete_request(&g);

        // Resolve concurrently, from a separate task.
        let resolver = g.clone();
        let id = request_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            resolver.resolve_approval(&id, true).unwrap();
        });

        let outcome = g.await_approval(&request_id, Duration::from_secs(2)).await;
        assert_eq!(outcome, ApprovalOutcome::Approved);
    }

    #[tokio::test]
    async fn test_await_approval_denied() {
        let g = Arc::new(Guardrails::new(make_registry()));
        let request_id = pending_delete_request(&g);

        let resolver = g.clone();
        let id = request_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            resolver.resolve_approval(&id, false).unwrap();
        });

        let outcome = g.await_approval(&request_id, Duration::from_secs(2)).await;
        assert_eq!(outcome, ApprovalOutcome::Denied);
    }

    #[tokio::test]
    async fn test_await_approval_timed_out() {
        let g = Guardrails::new(make_registry());
        let request_id = pending_delete_request(&g);

        let outcome = g
            .await_approval(&request_id, Duration::from_millis(20))
            .await;
        assert_eq!(outcome, ApprovalOutcome::TimedOut);
    }

    #[tokio::test]
    async fn test_await_approval_cleans_up_entry() {
        let g = Arc::new(Guardrails::new(make_registry()));
        let request_id = pending_delete_request(&g);
        assert!(g.get_pending_approval(&request_id).is_some());

        let resolver = g.clone();
        let id = request_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            resolver.resolve_approval(&id, true).unwrap();
        });

        let outcome = g.await_approval(&request_id, Duration::from_secs(2)).await;
        assert_eq!(outcome, ApprovalOutcome::Approved);
        assert!(
            g.get_pending_approval(&request_id).is_none(),
            "the entry must be removed after the turn consumes the decision"
        );
    }

    #[tokio::test]
    async fn test_dropped_await_cleans_up_entry() {
        let g = Arc::new(Guardrails::new(make_registry()));
        let request_id = pending_delete_request(&g);
        assert!(g.get_pending_approval(&request_id).is_some());

        // The outer timeout expires first and drops the inner `await_approval`
        // future mid-await. The RAII guard must still remove the registry entry.
        let expired = tokio::time::timeout(
            Duration::from_millis(10),
            g.await_approval(&request_id, Duration::from_secs(60)),
        )
        .await;
        assert!(expired.is_err(), "the outer timeout must have fired");

        assert!(
            g.get_pending_approval(&request_id).is_none(),
            "a dropped await_approval must clean up its entry"
        );
    }

    #[tokio::test]
    async fn test_await_approval_timeout_cleans_up_entry() {
        let g = Guardrails::new(make_registry());
        let request_id = pending_delete_request(&g);

        let outcome = g
            .await_approval(&request_id, Duration::from_millis(20))
            .await;
        assert_eq!(outcome, ApprovalOutcome::TimedOut);
        assert!(
            g.get_pending_approval(&request_id).is_none(),
            "the entry must be removed after a timeout"
        );

        // A second wait on the now-removed id resolves as `Cancelled`.
        let outcome = g
            .await_approval(&request_id, Duration::from_millis(20))
            .await;
        assert_eq!(outcome, ApprovalOutcome::Cancelled);
    }
}
