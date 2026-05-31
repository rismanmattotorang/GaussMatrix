mod list_joined_members;
mod view_room_topic;

use clap::Subcommand;
use ruma::OwnedRoomId;
use tuwunel_core::Result;

use crate::admin_command_dispatch;

#[admin_command_dispatch]
#[derive(Debug, Subcommand)]
pub(crate) enum RoomInfoCommand {
	/// - List joined members in a room
	ListJoinedMembers {
		room_id: OwnedRoomId,

		/// Lists only our local users in the specified room
		#[arg(long)]
		local_only: bool,
	},

	/// - Displays room topic
	///
	/// Room topics can be huge, so this is in its
	/// own separate command
	ViewRoomTopic {
		room_id: OwnedRoomId,
	},
}
