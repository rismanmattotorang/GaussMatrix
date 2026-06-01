//! Feature-gated `ruma` adapter (`core-bridge`).
//!
//! A blanket [`EventView`] over the server's `gaussmatrix_core::matrix::Event`,
//! so a real PDU bridges into the resolution model (via
//! [`StateEvent::from_view`](crate::StateEvent::from_view)) without a JSON
//! round-trip.

use gaussmatrix_core::matrix::Event as CoreEvent;
use serde_json::Value;

use crate::view::EventView;

impl<E: CoreEvent> EventView for E {
	fn event_id(&self) -> String { CoreEvent::event_id(self).to_string() }

	fn event_type(&self) -> String { CoreEvent::kind(self).to_string() }

	fn sender(&self) -> String { CoreEvent::sender(self).to_string() }

	fn state_key(&self) -> Option<String> {
		CoreEvent::state_key(self).map(ToOwned::to_owned)
	}

	fn origin_server_ts(&self) -> u64 {
		u64::from(CoreEvent::origin_server_ts(self).get())
	}

	fn auth_event_ids(&self) -> Vec<String> {
		CoreEvent::auth_events(self).map(ToString::to_string).collect()
	}

	fn content(&self) -> Value {
		serde_json::from_str(CoreEvent::content(self).get()).unwrap_or(Value::Null)
	}
}
