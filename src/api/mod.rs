#![expect(clippy::toplevel_ref_arg)]

pub mod agent;
pub mod client;
pub mod oidc;
pub mod router;
pub mod server;

use log as _;

pub(crate) use self::router::{ClientIp, Ruma, RumaResponse, State};

gaussmatrix_core::mod_ctor! {}
gaussmatrix_core::mod_dtor! {}
