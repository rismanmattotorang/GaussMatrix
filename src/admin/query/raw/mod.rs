mod clear;
mod compact;
mod count;
mod del;
mod get;
mod iter;
mod keys;
mod keys_sizes;
mod keys_total;
mod maps;
mod sequence;
mod vals_sizes;
mod vals_total;

use std::{fmt::Write, sync::Arc};

use clap::Subcommand;
use tuwunel_core::{Result, err, itertools::Itertools, utils::math::Expected};
use tuwunel_database::Map;
use tuwunel_service::Services;

use crate::admin_command_dispatch;

#[admin_command_dispatch(handler_prefix = "raw")]
#[derive(Debug, Subcommand)]
/// Query tables from database
pub(crate) enum RawCommand {
	/// - List database maps
	Maps,

	/// - Current rocksdb sequence number.
	Sequence,

	/// - Raw database query
	Get {
		/// Map name
		map: String,

		/// Key
		key: String,

		/// Encode as base64
		#[arg(long, short)]
		base64: bool,
	},

	/// - Raw database keys iteration
	Keys {
		/// Map name
		map: String,

		/// Key prefix
		prefix: Option<String>,

		/// Limit
		#[arg(short, long)]
		limit: Option<usize>,

		/// Lower bound
		#[arg(short, long)]
		from: Option<String>,

		/// Reverse iteration order
		#[arg(short, long, default_value("false"))]
		backwards: bool,
	},

	/// - Raw database items iteration
	Iter {
		/// Map name
		map: String,

		/// Key prefix
		prefix: Option<String>,

		/// Limit
		#[arg(short, long)]
		limit: Option<usize>,

		/// Lower bound
		#[arg(short, long)]
		from: Option<String>,

		/// Reverse iteration order
		#[arg(short, long, default_value("false"))]
		backwards: bool,
	},

	/// - Raw database key size breakdown
	KeysSizes {
		/// Map name
		map: Option<String>,

		/// Key prefix
		prefix: Option<String>,
	},

	/// - Raw database keys total bytes
	KeysTotal {
		/// Map name
		map: Option<String>,

		/// Key prefix
		prefix: Option<String>,
	},

	/// - Raw database values size breakdown
	ValsSizes {
		/// Map name
		map: Option<String>,

		/// Key prefix
		prefix: Option<String>,
	},

	/// - Raw database values total bytes
	ValsTotal {
		/// Map name
		map: Option<String>,

		/// Key prefix
		prefix: Option<String>,
	},

	/// - Raw database record count
	Count {
		/// Map name
		map: Option<String>,

		/// Key prefix
		prefix: Option<String>,
	},

	/// - Raw database delete (for string keys) DANGER!!!
	Del {
		/// Map name
		map: String,

		/// Key
		key: String,
	},

	/// - Clear database table DANGER!!!
	Clear {
		/// Map name
		map: String,

		/// Confirm
		#[arg(long)]
		confirm: bool,
	},

	/// - Compact database DANGER!!!
	Compact {
		#[arg(short, long, alias("column"))]
		maps: Option<Vec<String>>,

		#[arg(long)]
		start: Option<String>,

		#[arg(long)]
		stop: Option<String>,

		#[arg(long)]
		from: Option<usize>,

		#[arg(long)]
		into: Option<usize>,

		/// There is one compaction job per column; then this controls how many
		/// columns are compacted in parallel. If zero, one compaction job is
		/// still run at a time here, but in exclusive-mode blocking any other
		/// automatic compaction jobs until complete.
		#[arg(long)]
		parallelism: Option<usize>,

		#[arg(long, default_value("false"))]
		exhaustive: bool,
	},
}

fn with_map_or(map: Option<&str>, services: &Services) -> Result<Vec<Arc<Map>>> {
	with_maps_or(
		map.map(|map| [map])
			.as_ref()
			.map(<[&str; 1]>::as_slice),
		services,
	)
}

fn with_maps_or<S: AsRef<str>>(maps: Option<&[S]>, services: &Services) -> Result<Vec<Arc<Map>>> {
	Ok(if let Some(maps) = maps {
		maps.iter()
			.map(|map| {
				let map = map.as_ref();
				services
					.db
					.get(map)
					.cloned()
					.map_err(|_| err!("map {map} not found"))
			})
			.try_collect()?
	} else {
		services.db.iter().map(|x| x.1.clone()).collect()
	})
}

#[expect(clippy::as_conversions)]
fn encode(data: &[u8]) -> String {
	let mut res = String::with_capacity(data.len().expected_mul(4));

	for byte in data {
		if *byte < 0x20 || *byte > 0x7E {
			_ = write!(res, "\\x{byte:02x}");
		} else {
			res.push(*byte as char);
		}
	}

	res
}
