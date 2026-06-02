//! Capability scoping and the mediation decision (§IV-C).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

/// The state event type carrying an agent's capability grant. Keyed by the
/// agent's user id, the grant is visible, versioned, and federated room state,
/// so revocation is immediate (§IV-C).
pub const CAPABILITY_GRANT_TYPE: &str = "m.gauss.agent.capability";

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

impl Action {
	/// The stable wire label (`auto` / `review` / `forbidden`).
	#[must_use]
	pub const fn label(self) -> &'static str {
		match self {
			| Self::Auto => "auto",
			| Self::Review => "review",
			| Self::Forbidden => "forbidden",
		}
	}

	/// Parse an action from its wire label.
	#[must_use]
	pub fn from_label(label: &str) -> Option<Self> {
		match label {
			| "auto" => Some(Self::Auto),
			| "review" => Some(Self::Review),
			| "forbidden" => Some(Self::Forbidden),
			| _ => None,
		}
	}
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
	/// The tool's per-window rate limit has been exceeded.
	RateLimited,
}

/// A per-tool rate limit: at most `max` invocations per `window_secs` window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RateLimit {
	/// The maximum number of invocations permitted within a window.
	pub max: u32,
	/// The window length, in seconds.
	pub window_secs: u64,
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

impl Decision {
	/// A stable label for this decision (`execute`, `requires_approval`, or
	/// `denied:<reason>`), used in the audit record and on the MCP wire.
	#[must_use]
	pub fn label(self) -> String {
		match self {
			| Self::Execute => "execute".to_owned(),
			| Self::RequiresApproval => "requires_approval".to_owned(),
			| Self::Denied(reason) => format!("denied:{}", reason_label(reason)),
		}
	}

	/// Whether the invocation was rejected.
	#[must_use]
	pub const fn is_denied(self) -> bool { matches!(self, Self::Denied(_)) }
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
	rate_limits: BTreeMap<String, RateLimit>,
	default_action: Action,
	version: u64,
}

impl CapabilityGrant {
	/// An empty grant that permits nothing.
	#[must_use]
	pub fn new() -> Self { Self::default() }

	/// Set the grant's monotonic version. Each edit to a room's grant bumps
	/// this so changes are ordered and auditable (the lifecycle invariant).
	#[must_use]
	pub const fn with_version(mut self, version: u64) -> Self {
		self.version = version;
		self
	}

	/// This grant's version.
	#[must_use]
	pub const fn version(&self) -> u64 { self.version }

	/// Set the action applied to permitted tools that carry no explicit
	/// classification.
	#[must_use]
	pub const fn with_default_action(mut self, action: Action) -> Self {
		self.default_action = action;
		self
	}

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

	/// Apply a rate limit to a tool: at most `max` invocations per `window_secs`.
	#[must_use]
	pub fn with_rate_limit(mut self, tool: &str, max: u32, window_secs: u64) -> Self {
		self.rate_limits.insert(tool.to_owned(), RateLimit { max, window_secs });
		self
	}

	/// The rate limit configured for `tool`, if any.
	#[must_use]
	pub fn rate_limit_for(&self, tool: &str) -> Option<RateLimit> {
		self.rate_limits.get(tool).copied()
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

	/// The rooms this grant makes accessible.
	pub fn rooms(&self) -> impl Iterator<Item = &str> + '_ {
		self.accessible_rooms.iter().map(String::as_str)
	}

	/// The permitted tools and their action classification.
	pub fn tools(&self) -> impl Iterator<Item = (&str, Action)> + '_ {
		self.permitted_tools.iter().map(move |tool| {
			let action = self.tool_actions.get(tool).copied().unwrap_or(self.default_action);
			(tool.as_str(), action)
		})
	}

	/// The tools that carry a rate limit, with their limits.
	pub fn rate_limits(&self) -> impl Iterator<Item = (&str, RateLimit)> + '_ {
		self.rate_limits.iter().map(|(tool, limit)| (tool.as_str(), *limit))
	}

	/// Serialise the grant into `m.gauss.agent.capability` state-event content.
	#[must_use]
	pub fn to_content(&self) -> Value {
		let rooms: Vec<Value> = self
			.accessible_rooms
			.iter()
			.map(|room| Value::from(room.clone()))
			.collect();

		let mut tools = Map::new();
		for tool in &self.permitted_tools {
			let action = self.tool_actions.get(tool).copied().unwrap_or(self.default_action);
			tools.insert(tool.clone(), Value::from(action.label()));
		}

		let mut rate_limits = Map::new();
		for (tool, limit) in &self.rate_limits {
			rate_limits.insert(
				tool.clone(),
				json!({ "max": limit.max, "window_secs": limit.window_secs }),
			);
		}

		json!({
			"rooms": rooms,
			"tools": Value::Object(tools),
			"rate_limits": Value::Object(rate_limits),
			"default_action": self.default_action.label(),
			"version": self.version,
		})
	}

	/// Parse a grant from `m.gauss.agent.capability` state-event content.
	///
	/// Unknown or malformed fields are ignored; an absent `default_action`
	/// keeps the conservative default ([`Action::Review`]).
	#[must_use]
	pub fn from_content(content: &Value) -> Self {
		let mut grant = Self::new();

		grant.version = content.get("version").and_then(Value::as_u64).unwrap_or(0);

		if let Some(default) = content
			.get("default_action")
			.and_then(Value::as_str)
			.and_then(Action::from_label)
		{
			grant.default_action = default;
		}

		if let Some(rooms) = content.get("rooms").and_then(Value::as_array) {
			for room in rooms.iter().filter_map(Value::as_str) {
				grant.accessible_rooms.insert(room.to_owned());
			}
		}

		if let Some(tools) = content.get("tools").and_then(Value::as_object) {
			for (tool, action) in tools {
				let action = action
					.as_str()
					.and_then(Action::from_label)
					.unwrap_or(grant.default_action);
				grant.permitted_tools.insert(tool.clone());
				grant.tool_actions.insert(tool.clone(), action);
			}
		}

		if let Some(limits) = content.get("rate_limits").and_then(Value::as_object) {
			for (tool, limit) in limits {
				let Some(max) = limit.get("max").and_then(Value::as_u64).and_then(|m| u32::try_from(m).ok())
				else {
					continue;
				};
				let window_secs = limit.get("window_secs").and_then(Value::as_u64).unwrap_or(0);
				grant.rate_limits.insert(tool.clone(), RateLimit { max, window_secs });
			}
		}

		grant
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
		"decision": decision.label(),
	});

	serde_json::to_vec(&body).unwrap_or_default()
}

/// A stable label for a deny reason.
fn reason_label(reason: DenyReason) -> &'static str {
	match reason {
		| DenyReason::RoomNotInScope => "room_not_in_scope",
		| DenyReason::ToolNotGranted => "tool_not_granted",
		| DenyReason::ToolForbidden => "tool_forbidden",
		| DenyReason::RateLimited => "rate_limited",
	}
}
