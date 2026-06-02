//! Agent provisioning model (§IV-A).
//!
//! An agent is a Matrix principal provisioned through the Application Service
//! API as a user in a controlled namespace — not a privileged side channel. A
//! provisioned agent is bound to a **cross-signing public key** so clients can
//! verify it as a first-class, cross-signed identity rather than trusting it
//! implicitly.
//!
//! This module models the provisioning record and its validation. Verifying
//! signatures against the bound key is the crypto layer's job; this is the
//! identity contract it builds on.

use serde_json::{Map, Value};

/// The default agent-id namespace prefix. A deployment may configure its own.
pub const DEFAULT_AGENT_NAMESPACE: &str = "@gauss.agent.";

/// Whether `user_id` belongs to the agent namespace `prefix`.
#[must_use]
pub fn is_agent_id(user_id: &str, prefix: &str) -> bool { user_id.starts_with(prefix) }

/// Why provisioning an agent was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProvisionError {
	/// The user id is not in the agent namespace.
	NotInNamespace,
	/// No cross-signing public key was supplied.
	MissingSigningKey,
}

impl ProvisionError {
	/// A stable, human-readable label.
	#[must_use]
	pub const fn label(self) -> &'static str {
		match self {
			| Self::NotInNamespace => "agent id is not in the agent namespace",
			| Self::MissingSigningKey => "a cross-signing public key is required",
		}
	}
}

/// A provisioned agent identity (§IV-A).
///
/// A Matrix principal in the agent namespace, provisioned by an operator (the
/// Application Service that owns the namespace) and bound to a cross-signing
/// public key. The record is the server's statement that this identity is a
/// governed agent, not an unverified user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentProfile {
	/// The agent's Matrix user id (in the agent namespace).
	pub agent_id: String,
	/// The operator that provisioned it — the owning appservice id.
	pub operator: String,
	/// The bound cross-signing master public key (opaque, base64).
	pub signing_key: String,
	/// An optional human-readable display name.
	pub display_name: Option<String>,
}

impl AgentProfile {
	/// Provision an agent, validating that `agent_id` is in the `namespace` and
	/// that a non-empty cross-signing `signing_key` was supplied.
	///
	/// # Errors
	///
	/// Returns [`ProvisionError`] if the id is outside the namespace or no
	/// signing key is given.
	pub fn provision(
		agent_id: &str,
		namespace: &str,
		operator: &str,
		signing_key: &str,
		display_name: Option<&str>,
	) -> Result<Self, ProvisionError> {
		if !is_agent_id(agent_id, namespace) {
			return Err(ProvisionError::NotInNamespace);
		}
		if signing_key.trim().is_empty() {
			return Err(ProvisionError::MissingSigningKey);
		}

		Ok(Self {
			agent_id: agent_id.to_owned(),
			operator: operator.to_owned(),
			signing_key: signing_key.to_owned(),
			display_name: display_name.map(ToOwned::to_owned),
		})
	}

	/// The provisioning record as a JSON value, for persistence.
	#[must_use]
	pub fn to_content(&self) -> Value {
		let mut body = Map::new();
		body.insert("agent_id".to_owned(), Value::from(self.agent_id.clone()));
		body.insert("operator".to_owned(), Value::from(self.operator.clone()));
		body.insert("signing_key".to_owned(), Value::from(self.signing_key.clone()));
		if let Some(display_name) = &self.display_name {
			body.insert("display_name".to_owned(), Value::from(display_name.clone()));
		}

		Value::Object(body)
	}

	/// Parse a provisioning record; `agent_id`, `operator`, and `signing_key`
	/// are required. The inverse of [`to_content`](Self::to_content).
	#[must_use]
	pub fn from_content(content: &Value) -> Option<Self> {
		let agent_id = content.get("agent_id").and_then(Value::as_str)?;
		let operator = content.get("operator").and_then(Value::as_str)?;
		let signing_key = content.get("signing_key").and_then(Value::as_str)?;
		let display_name =
			content.get("display_name").and_then(Value::as_str).map(ToOwned::to_owned);

		Some(Self {
			agent_id: agent_id.to_owned(),
			operator: operator.to_owned(),
			signing_key: signing_key.to_owned(),
			display_name,
		})
	}
}
