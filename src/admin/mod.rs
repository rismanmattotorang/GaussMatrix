#![expect(clippy::too_many_arguments)]

pub(crate) mod admin;
#[macro_use]
pub(crate) mod context;
mod tests;
pub(crate) mod utils;

pub(crate) mod agent;
pub(crate) mod appservice;
pub(crate) mod debug;
pub(crate) mod federation;
pub(crate) mod media;
pub(crate) mod query;
pub(crate) mod room;
pub(crate) mod server;
pub(crate) mod token;
pub(crate) mod user;

use std::sync::Arc;

use log as _;
pub(crate) use gaussmatrix_macros::{admin_command, admin_command_dispatch};

pub(crate) use crate::{context::Context, utils::get_room_info};

pub(crate) const PAGE_SIZE: usize = 100;

gaussmatrix_core::mod_ctor! {}
gaussmatrix_core::mod_dtor! {}
gaussmatrix_core::rustc_flags_capture! {}

/// Install the admin command root.
pub fn init(admin_service: &gaussmatrix_service::admin::Service) {
	let root: Arc<dyn gaussmatrix_service::admin::Command> = Arc::new(admin::Root);
	_ = admin_service
		.command
		.write()
		.expect("locked for writing")
		.insert(root);
}

/// Uninstall the admin command root.
pub fn fini(admin_service: &gaussmatrix_service::admin::Service) {
	_ = admin_service
		.command
		.write()
		.expect("locked for writing")
		.take();
}
