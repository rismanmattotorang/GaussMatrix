//! # gm-api — GaussMatrix typed Matrix model (event-content adapters)
//!
//! This crate is the start of the GaussMatrix typed model layer
//! ([`GaussMatrix-SPECS.pdf`], §III-B, "extending ruma"). Its first concern is
//! the **adapter** between raw Matrix event content and the state-resolution
//! engine: parsing the content of `m.room.power_levels`, `m.room.member`, and
//! `m.room.join_rules` events into the [`gm_stateres`] models the authorisation
//! rules consume, and a [`StateEvent`] that carries those parsed projections and
//! implements [`gm_stateres::Event`].
//!
//! The parsers apply the Matrix content defaults (e.g. `state_default`/`ban`/
//! `kick` default to 50, `invite`/`users_default`/`events_default` to 0 on a
//! present power-levels event) and tolerate the integer-or-string encoding of
//! power-level values that older rooms emit.
//!
//! ## Scope
//!
//! This is the event-typing foundation. The full CS/SS request/response surface
//! and a direct `gm_stateres::Event` impl over the server's ruma-backed `Pdu`
//! type build on this and land when the engine is wired into the server. The
//! sender's effective power level is *derived* during resolution (it is not
//! intrinsic to an event), so [`StateEvent`] carries it as a separately-set
//! field rather than parsing it from content.
//!
//! [`GaussMatrix-SPECS.pdf`]: ../../GaussMatrix-SPECS.pdf

#![forbid(unsafe_code)]

mod auth;
mod content;
mod endpoint;
mod error;
mod event;
#[cfg(test)]
mod tests;
mod versions;

pub use self::{
	auth::extract_access_token,
	content::{
		join_authorised_from_content, join_rule_from_content, membership_from_content,
		power_levels_from_content,
	},
	endpoint::{AuthScope, Endpoint, Method, PathParams, match_template},
	error::{ErrorCode, MatrixError},
	event::StateEvent,
	versions::Versions,
};
