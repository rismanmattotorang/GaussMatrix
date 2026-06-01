//! Capability scoping and the mediation decision (§IV-C).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;

/// How an agent's invocation of a permitted tool is handled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
	/// Executed immediately.
	Auto,
	/// Executed only after human approval, rendered in GaussInteract.
	Review,
	/// Never executed (an explicit deny entry).
	Forbidden,
}

impl Default for Action {
	/// Permitted-but-unclassified tools default to requiring approval — the
	/// conservative, human-in-the-loop choice.
	fn default() -> Self { Self::Review }
}

/// Why a tool invocation was denied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DenyReason {
	/// The target room is not in the grant's accessible-room set.
	RoomNotInScope,
	/// The tool is not in the grant's permitted-tool set.
	ToolNotGranted,
	/// The tool is permitted but explicitly classified as forbidden.
	ToolForbidden,
}

/// The outcome of mediating a tool invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
	/// Execute the invocation immediately.
	Execute,
	/// Execute only after human approval.
	RequiresApproval,
	/// Reject the invocation.
	Denied(DenyReason),
}

/// An agent's capability grant: an explicit, least-privilege set of permitted
/// tools, accessible rooms, and per-tool action classification.
///
/// A default grant permits nothing — every invocation is denied until tools and
/// rooms are explicitly allowed, enforcing the least-privilege invariant.
#[derive(Clone, Debug, Default)]
pub struct CapabilityGrant {
	permitted_tools: BTreeSet<String>,
	accessible_rooms: BTreeSet<String>,
	tool_actions: BTreeMap<String, Action>,
	default_action: Action,
}

impl CapabilityGrant {
	/// An empty grant that permits nothing.
	#[must_use]
	pub fn new() -> Self { Self::default() }

	/// Grant access to a room.
	#[must_use]
	pub fn allow_room(mut self, room: &str) -> Self {
		self.accessible_rooms.insert(room.to_owned());
		self
	}

	/// Permit a tool, with the given action classification.
	#[must_use]
	pub fn allow_tool(mut self, tool: &str, action: Action) -> Self {
		self.permitted_tools.insert(tool.to_owned());
		self.tool_actions.insert(tool.to_owned(), action);
		self
	}

	/// Mediate an invocation of `tool` in `room` against this grant.
	///
	/// The room must be in scope and the tool permitted; otherwise the
	/// invocation is denied. A permitted tool's classification then determines
	/// whether it executes immediately, requires approval, or is forbidden.
	#[must_use]
	pub fn mediate(&self, tool: &str, room: &str) -> Decision {
		if !self.accessible_rooms.contains(room) {
			return Decision::Denied(DenyReason::RoomNotInScope);
		}
		if !self.permitted_tools.contains(tool) {
			return Decision::Denied(DenyReason::ToolNotGranted);
		}

		match self.tool_actions.get(tool).copied().unwrap_or(self.default_action) {
			| Action::Auto => Decision::Execute,
			| Action::Review => Decision::RequiresApproval,
			| Action::Forbidden => Decision::Denied(DenyReason::ToolForbidden),
		}
	}
}

/// Serialise a gateway mediation decision into an audit-log record (§IV-D): the
/// agent, the tool, the room, and the resulting decision.
#[must_use]
pub fn mediation_record(agent: &str, tool: &str, room: &str, decision: Decision) -> Vec<u8> {
	let body = json!({
		"agent": agent,
		"tool": tool,
		"room": room,
		"decision": decision_label(decision),
	});

	serde_json::to_vec(&body).unwrap_or_default()
}

/// A stable label for a decision, used in the audit record.
fn decision_label(decision: Decision) -> String {
	match decision {
		| Decision::Execute => "execute".to_owned(),
		| Decision::RequiresApproval => "requires_approval".to_owned(),
		| Decision::Denied(reason) => format!("denied:{}", reason_label(reason)),
	}
}

/// A stable label for a deny reason.
fn reason_label(reason: DenyReason) -> &'static str {
	match reason {
		| DenyReason::RoomNotInScope => "room_not_in_scope",
		| DenyReason::ToolNotGranted => "tool_not_granted",
		| DenyReason::ToolForbidden => "tool_forbidden",
	}
}
