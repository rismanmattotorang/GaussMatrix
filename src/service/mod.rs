#![expect(refining_impl_trait)]

mod manager;
mod migrations;
mod once_services;
mod service;
pub mod services;
pub mod gm_resolve;
mod store_provider;

pub mod account_data;
pub mod admin;
pub mod agent;
pub mod appservice;
pub mod audit;
pub mod client;
pub mod config;
pub mod deactivate;
pub mod emergency;
pub mod fed;
pub mod federation;
pub mod globals;
pub mod key_backups;
pub mod media;
pub mod membership;
pub mod oauth;
pub mod presence;
pub mod pusher;
pub mod registration_tokens;
pub mod resolver;
pub mod rooms;
pub mod sending;
pub mod server_keys;
pub mod storage;
pub mod sync;
pub mod transaction_ids;
pub mod uiaa;
pub mod users;

pub(crate) use once_services::OnceServices;
pub(crate) use service::{Args, Service};

pub(crate) type SelfServices = std::sync::Arc<OnceServices>;

use log as _;

pub use crate::services::Services;

gaussmatrix_core::mod_ctor! {}
gaussmatrix_core::mod_dtor! {}
gaussmatrix_core::rustc_flags_capture! {}
