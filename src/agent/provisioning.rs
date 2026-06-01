//! Agent provisioning model (§IV-A).
//!
//! An agent is a Matrix principal provisioned through the Application Service
//! API as a user in a controlled namespace — not a privileged side channel.
//! This models the namespace check; cross-signing and device provisioning build
//! on it at the integration layer.

/// The default agent-id namespace prefix. A deployment may configure its own.
pub const DEFAULT_AGENT_NAMESPACE: &str = "@gauss.agent.";

/// Whether `user_id` belongs to the agent namespace `prefix`.
#[must_use]
pub fn is_agent_id(user_id: &str, prefix: &str) -> bool { user_id.starts_with(prefix) }
