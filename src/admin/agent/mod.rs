mod audit_count;
mod audit_verify;
mod deprovision;
mod grant_show;
mod list;
mod profile;
mod provision;

use clap::Subcommand;
use gaussmatrix_core::Result;
use ruma::{OwnedRoomId, OwnedUserId};

use crate::admin_command_dispatch;

#[admin_command_dispatch(handler_prefix = "agent")]
#[derive(Debug, Subcommand)]
pub(crate) enum AgentCommand {
	/// - Provision an agent identity, binding a cross-signing public key
	Provision {
		/// The agent's Matrix user id (must be in the agent namespace).
		user_id: OwnedUserId,

		/// The agent's cross-signing master public key (opaque, base64).
		#[arg(long)]
		signing_key: String,

		/// An optional human-readable display name.
		#[arg(long)]
		display_name: Option<String>,
	},

	/// - Remove an agent's provisioning record
	Deprovision {
		/// The agent's Matrix user id.
		user_id: OwnedUserId,
	},

	/// - Show an agent's provisioning record
	Profile {
		/// The agent's Matrix user id.
		user_id: OwnedUserId,
	},

	/// - List all provisioned agents
	List,

	/// - Show the effective capability grant for a room
	GrantShow {
		/// The room whose `m.gauss.agent.capability` grant to read.
		room_id: OwnedRoomId,
	},

	/// - Verify the integrity of the tamper-evident agent audit log
	AuditVerify,

	/// - Count the entries in the agent audit log
	AuditCount,
}
