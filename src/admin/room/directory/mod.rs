mod list;
mod publish;
mod unpublish;

use clap::Subcommand;
use ruma::OwnedRoomId;
use gaussmatrix_core::Result;

use crate::admin_command_dispatch;

#[admin_command_dispatch(handler_prefix = "directory")]
#[derive(Debug, Subcommand)]
pub(crate) enum RoomDirectoryCommand {
	/// - Publish a room to the room directory
	Publish {
		/// The room id of the room to publish
		room_id: OwnedRoomId,
	},

	/// - Unpublish a room to the room directory
	Unpublish {
		/// The room id of the room to unpublish
		room_id: OwnedRoomId,
	},

	/// - List rooms that are published
	List {
		page: Option<usize>,
	},
}
