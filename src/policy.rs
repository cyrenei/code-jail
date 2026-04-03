//! Policy enforcement integration.
//!
//! When policy mode is active (--policy <policy.toml>), every capability
//! request is evaluated against a deny-by-default policy before WASI is configured.
//! Each sandbox run creates a session. Denied capabilities are
//! rejected. All decisions are audit-logged.

use std::path::Path;

use arbiter_audit::{AuditEntry, AuditSink, AuditSinkConfig};
use arbiter_behavior::{AnomalyConfig, AnomalyDetector, classify_operation};
use arbiter_identity::{Agent, AgentRegistry, InMemoryRegistry, TrustLevel};
use arbiter_mcp::context::McpRequest;
use arbiter_policy::{Decision, EvalContext, PolicyConfig, evaluate};
use arbiter_session::{CreateSessionRequest, SessionStore, TaskSession};
use chrono::Utc;
use uuid::Uuid;

use crate::capability::{CapGrant, FsMount, ResolvedCaps};

/// Policy gate state for a codejail session.
pub struct PolicyGate {
    policy: PolicyConfig,
    registry: InMemoryRegistry,
    sessions: SessionStore,
    audit: AuditSink,
    anomaly: AnomalyDetector,
}

/// Result of evaluating a single capability through the policy engine.
#[derive(Debug)]
pub struct CapDecision {
    pub tool_name: String,
    pub allowed: bool,
    pub reason: String,
    pub policy_id: Option<String>,
}

/// Summary of policy evaluation for an entire run.
pub struct PolicyVerdict {
    pub session: TaskSession,
    pub agent: Agent,
    pub decisions: Vec<CapDecision>,
    pub authorized_caps: ResolvedCaps,
    pub denied_count: usize,
}

impl PolicyGate {
    /// Load policy from a TOML file and initialize the gate.
    pub fn load(policy_path: &Path, audit_path: Option<&Path>) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(policy_path)?;
        let mut policy: PolicyConfig = toml::from_str(&content)?;
        policy.compile().map_err(|e| anyhow::anyhow!("{e}"))?;

        let audit_config = AuditSinkConfig {
            write_stdout: false,
            file_path: audit_path.map(|p| p.to_path_buf()),
        };

        Ok(Self {
            policy,
            registry: InMemoryRegistry::new(),
            sessions: SessionStore::new(),
            audit: AuditSink::new(audit_config),
            anomaly: AnomalyDetector::new(AnomalyConfig::default()),
        })
    }

    /// Register a WASM image as an agent.
    pub async fn register_agent(
        &self,
        image_name: &str,
        capabilities: Vec<String>,
        trust_level: TrustLevel,
    ) -> anyhow::Result<Agent> {
        let agent = self
            .registry
            .register_agent(
                "codejail".to_string(),
                image_name.to_string(),
                capabilities,
                trust_level,
                None,
            )
            .await
            .map_err(|e| anyhow::anyhow!("agent registration failed: {e}"))?;
        Ok(agent)
    }

    /// Create a session for a sandbox run.
    pub async fn create_session(
        &self,
        agent: &Agent,
        intent: &str,
        authorized_tools: Vec<String>,
        call_budget: u64,
        time_limit_secs: u64,
    ) -> TaskSession {
        self.sessions
            .create(CreateSessionRequest {
                agent_id: agent.id,
                delegation_chain_snapshot: vec![],
                declared_intent: intent.to_string(),
                authorized_tools,
                time_limit: chrono::Duration::seconds(time_limit_secs as i64),
                call_budget,
                rate_limit_per_minute: None,
                rate_limit_window_secs: 60,
                data_sensitivity_ceiling: arbiter_session::DataSensitivity::Internal,
            })
            .await
    }

    /// Evaluate all capability grants against policy.
    ///
    /// Returns a PolicyVerdict with the authorized subset of caps,
    /// plus audit logs for every decision.
    #[allow(clippy::too_many_arguments)]
    pub async fn evaluate_caps(
        &self,
        image_name: &str,
        intent: &str,
        grants: &[CapGrant],
        volumes: &[String],
        env_overrides: &[String],
        net_flag: bool,
        call_budget: u64,
        time_limit_secs: u64,
    ) -> anyhow::Result<PolicyVerdict> {
        // Register agent for this WASM image
        let cap_names: Vec<String> =
            Self::extract_cap_names(grants, volumes, env_overrides, net_flag);
        let agent = self
            .register_agent(image_name, cap_names.clone(), TrustLevel::Basic)
            .await?;

        // Create session
        let session = self
            .create_session(
                &agent,
                intent,
                cap_names.clone(),
                call_budget,
                time_limit_secs,
            )
            .await;

        // Build eval context
        let eval_ctx = EvalContext {
            agent: agent.clone(),
            delegation_chain: vec![],
            declared_intent: intent.to_string(),
            principal_sub: "codejail-operator".to_string(),
            principal_groups: vec!["sandbox-runners".to_string()],
        };

        // Evaluate each capability as an MCP tool call
        let mut decisions = Vec::new();
        let mut authorized_mounts = Vec::new();
        let mut authorized_net: Vec<String> = Vec::new();
        let mut authorized_env: Vec<(String, String)> = Vec::new();
        let mut denied_count = 0;

        // Evaluate filesystem grants
        for grant in grants {
            match grant {
                CapGrant::Fs(mount) => {
                    let tool_name = if mount.writable {
                        "fs_write".to_string()
                    } else {
                        "fs_read".to_string()
                    };
                    let args = serde_json::json!({
                        "host_path": mount.host.to_string_lossy(),
                        "guest_path": &mount.guest,
                        "writable": mount.writable,
                    });
                    let decision = self
                        .evaluate_single(&eval_ctx, &session, &tool_name, Some(args))
                        .await;
                    if decision.allowed {
                        authorized_mounts.push(mount.clone());
                    } else {
                        denied_count += 1;
                    }
                    decisions.push(decision);
                }
                CapGrant::Net(rule) => {
                    let tool_name = "net_connect".to_string();
                    let args = serde_json::json!({ "destination": rule });
                    let decision = self
                        .evaluate_single(&eval_ctx, &session, &tool_name, Some(args))
                        .await;
                    if decision.allowed {
                        authorized_net.push(rule.clone());
                    } else {
                        denied_count += 1;
                    }
                    decisions.push(decision);
                }
                CapGrant::Env(vars) => {
                    let tool_name = "env_read".to_string();
                    for var in vars {
                        let args = serde_json::json!({ "variable": var });
                        let decision = self
                            .evaluate_single(&eval_ctx, &session, &tool_name, Some(args))
                            .await;
                        if decision.allowed {
                            if let Ok(val) = std::env::var(var) {
                                authorized_env.push((var.clone(), val));
                            }
                        } else {
                            denied_count += 1;
                        }
                        decisions.push(decision);
                    }
                }
            }
        }

        // Evaluate volume mounts
        for v in volumes {
            let (host, guest) = if let Some((h, g)) = v.split_once(':') {
                (h.to_string(), g.to_string())
            } else {
                (v.clone(), v.clone())
            };
            let tool_name = "fs_write".to_string();
            let args = serde_json::json!({
                "host_path": &host,
                "guest_path": &guest,
                "writable": true,
            });
            let decision = self
                .evaluate_single(&eval_ctx, &session, &tool_name, Some(args))
                .await;
            if decision.allowed {
                authorized_mounts.push(FsMount {
                    host: host.into(),
                    guest,
                    writable: true,
                });
            } else {
                denied_count += 1;
            }
            decisions.push(decision);
        }

        // Evaluate env overrides
        for e in env_overrides {
            let (key, value) = if let Some((k, v)) = e.split_once('=') {
                (k.to_string(), v.to_string())
            } else {
                (e.to_string(), std::env::var(e).unwrap_or_default())
            };
            let tool_name = "env_read".to_string();
            let args = serde_json::json!({ "variable": &key });
            let decision = self
                .evaluate_single(&eval_ctx, &session, &tool_name, Some(args))
                .await;
            if decision.allowed {
                authorized_env.push((key, value));
            } else {
                denied_count += 1;
            }
            decisions.push(decision);
        }

        // Net flag
        if net_flag {
            let tool_name = "net_connect".to_string();
            let args = serde_json::json!({ "destination": "*" });
            let decision = self
                .evaluate_single(&eval_ctx, &session, &tool_name, Some(args))
                .await;
            if decision.allowed {
                authorized_net.push("*".to_string());
            } else {
                denied_count += 1;
            }
            decisions.push(decision);
        }

        let authorized_caps = ResolvedCaps {
            fs_mounts: authorized_mounts,
            net_rules: authorized_net,
            env_vars: authorized_env,
            inherit_stdio: true,
        };

        Ok(PolicyVerdict {
            session,
            agent,
            decisions,
            authorized_caps,
            denied_count,
        })
    }

    /// Evaluate a single capability as an MCP tool call against policy.
    async fn evaluate_single(
        &self,
        ctx: &EvalContext,
        session: &TaskSession,
        tool_name: &str,
        arguments: Option<serde_json::Value>,
    ) -> CapDecision {
        let mcp_request = McpRequest {
            id: Some(serde_json::Value::String(Uuid::new_v4().to_string())),
            method: "tools/call".to_string(),
            tool_name: Some(tool_name.to_string()),
            arguments,
            resource_uri: None,
        };

        // Check session tool whitelist
        if !session.authorized_tools.is_empty()
            && !session.authorized_tools.contains(&tool_name.to_string())
        {
            let decision = CapDecision {
                tool_name: tool_name.to_string(),
                allowed: false,
                reason: format!("tool '{tool_name}' not in session whitelist"),
                policy_id: None,
            };
            self.audit_decision(ctx, session, &mcp_request, &decision)
                .await;
            return decision;
        }

        // Evaluate policy
        let policy_decision = evaluate(&self.policy, ctx, &mcp_request);

        let decision = match policy_decision {
            Decision::Allow { policy_id } => CapDecision {
                tool_name: tool_name.to_string(),
                allowed: true,
                reason: format!("allowed by policy '{policy_id}'"),
                policy_id: Some(policy_id),
            },
            Decision::Deny { reason } => CapDecision {
                tool_name: tool_name.to_string(),
                allowed: false,
                reason,
                policy_id: None,
            },
            Decision::Escalate { reason } => CapDecision {
                tool_name: tool_name.to_string(),
                allowed: false,
                reason: format!("escalation required: {reason}"),
                policy_id: None,
            },
            Decision::Annotate { policy_id, reason } => CapDecision {
                tool_name: tool_name.to_string(),
                allowed: true,
                reason: format!("annotated by '{policy_id}': {reason}"),
                policy_id: Some(policy_id),
            },
        };

        // Behavioral drift check
        let op_type = classify_operation("tools/call", Some(tool_name));
        let anomaly_response = self
            .anomaly
            .detect(&ctx.declared_intent, op_type, tool_name);
        if !matches!(anomaly_response, arbiter_behavior::AnomalyResponse::Normal) {
            eprintln!(
                "[codejail] drift detected: {tool_name} ({op_type:?}) vs intent '{}'",
                ctx.declared_intent
            );
        }

        self.audit_decision(ctx, session, &mcp_request, &decision)
            .await;
        decision
    }

    /// Write an audit entry for a capability decision.
    async fn audit_decision(
        &self,
        ctx: &EvalContext,
        session: &TaskSession,
        mcp_request: &McpRequest,
        decision: &CapDecision,
    ) {
        let entry = AuditEntry {
            timestamp: Utc::now(),
            request_id: Uuid::new_v4(),
            agent_id: ctx.agent.id.to_string(),
            delegation_chain: String::new(),
            task_session_id: session.session_id.to_string(),
            tool_called: decision.tool_name.clone(),
            arguments: mcp_request
                .arguments
                .clone()
                .unwrap_or(serde_json::Value::Null),
            authorization_decision: if decision.allowed {
                "allow".to_string()
            } else {
                "deny".to_string()
            },
            policy_matched: decision.policy_id.clone(),
            anomaly_flags: vec![],
            failure_category: None,
            latency_ms: 0,
            upstream_status: None,
            inspection_findings: vec![],
        };

        if let Err(e) = self.audit.write(&entry).await {
            eprintln!("[codejail] audit write failed: {e}");
        }
    }

    /// Extract tool names from capability grants for session registration.
    fn extract_cap_names(
        grants: &[CapGrant],
        volumes: &[String],
        env_overrides: &[String],
        net_flag: bool,
    ) -> Vec<String> {
        let mut names = Vec::new();
        for grant in grants {
            match grant {
                CapGrant::Fs(mount) => {
                    if mount.writable {
                        names.push("fs_write".to_string());
                    } else {
                        names.push("fs_read".to_string());
                    }
                }
                CapGrant::Net(_) => names.push("net_connect".to_string()),
                CapGrant::Env(_) => names.push("env_read".to_string()),
            }
        }
        if !volumes.is_empty() {
            names.push("fs_write".to_string());
        }
        if !env_overrides.is_empty() {
            names.push("env_read".to_string());
        }
        if net_flag {
            names.push("net_connect".to_string());
        }
        names.sort();
        names.dedup();
        names
    }
}
