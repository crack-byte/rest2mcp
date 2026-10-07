use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::mcp::{RiskClassification, ToolDefinition};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum Persona {
    #[default]
    Human,
    ServiceAccount,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct CallerContext {
    pub persona: Persona,
    pub subject: String,
    pub scopes: Vec<String>,
}

impl CallerContext {
    pub fn can_access(&self, tool: &ToolDefinition) -> bool {
        tool.required_scopes
            .iter()
            .all(|required| self.scopes.iter().any(|scope| scope == required))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub action: String,
    pub tool_name: String,
    pub risk: RiskClassification,
    pub subject: String,
    pub persona: Persona,
    pub ts: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalToken {
    pub token: String,
    pub subject: String,
    pub tool_name: String,
    pub expires_at: DateTime<Utc>,
}

impl ApprovalToken {
    pub fn is_valid(&self, now: DateTime<Utc>) -> bool {
        self.expires_at > now
    }
}

#[derive(Debug, Clone, Default)]
pub struct SecurityGuard {
    approvals: Arc<Mutex<HashMap<String, PendingApproval>>>,
}

#[derive(Debug, Clone)]
struct PendingApproval {
    approval: ApprovalToken,
    params: Value,
}

impl SecurityGuard {
    pub fn authorize(&self, caller: &CallerContext, tool: &ToolDefinition) -> bool {
        if tool.required_scopes.is_empty() {
            return true;
        }

        caller.can_access(tool)
    }

    pub fn requires_human_confirmation(&self, tool: &ToolDefinition) -> bool {
        tool.method.eq_ignore_ascii_case("delete") || tool.risk == RiskClassification::Critical
    }

    pub fn audit_write(&self, entry: &AuditEntry) {
        tracing::info!(
            subject = %entry.subject,
            tool = %entry.tool_name,
            risk = ?entry.risk,
            persona = ?entry.persona,
            "write operation audited"
        );
    }

    pub fn create_approval_request(&self, subject: &str, tool_name: &str, params: Value) -> ApprovalToken {
        let token = Uuid::new_v4().to_string();
        let expires_at = Utc::now() + Duration::minutes(5);
        let approval = ApprovalToken {
            token: token.clone(),
            subject: subject.to_string(),
            tool_name: tool_name.to_string(),
            expires_at,
        };

        self.approvals
            .lock()
            .expect("approval store lock poisoned")
            .insert(token.clone(), PendingApproval { approval: approval.clone(), params });

        approval
    }

    pub fn consume_approval_request(&self, token: &str, subject: &str) -> Result<(ApprovalToken, Value), String> {
        let mut approvals = self.approvals.lock().expect("approval store lock poisoned");
        let pending = approvals
            .get(token)
            .cloned()
            .ok_or_else(|| "approval token is invalid or already used".to_string())?;
        if pending.approval.subject != subject {
            return Err("approval token belongs to a different caller".to_string());
        }
        if !pending.approval.is_valid(Utc::now()) {
            approvals.remove(token);
            return Err("approval token has expired".to_string());
        }
        approvals.remove(token);
        Ok((pending.approval, pending.params))
    }

    pub fn detect_fragmentation(&self, messages: &[String]) -> bool {
        let joined = messages.join(" ");
        let fragments = ["role", "system", "admin", "sensitive", "ignore previous", "tool schema"];
        for token in fragments {
            if joined.contains(token)
                && messages
                    .iter()
                    .filter(|part| part.to_ascii_lowercase().contains(token))
                    .count()
                    > 1
            {
                return true;
            }
        }

        false
    }
}
