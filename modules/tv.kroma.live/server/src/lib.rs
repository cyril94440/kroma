#![allow(clippy::result_large_err)]

#[cfg(target_family = "wasm")]
extern crate kroma_sqlite_guest as rusqlite;


mod catalogue;
mod clipbuf;
#[cfg(test)]
mod clipbuf_tests;
mod cmd;
mod config;
#[cfg(test)]
mod catalogue_tests;
mod db;
mod directory;
mod epg_parse;
mod epg_refresh;
mod epg_search;
#[cfg(all(test, not(target_family = "wasm")))]
mod fake_portal;
mod failover;
mod health;
#[cfg(test)]
mod health_tests;
mod loader;
mod model;
mod norm;
mod portal;
mod presence;
mod probe;
mod providers;
mod publicpath;
mod module;
#[cfg(all(test, not(target_family = "wasm")))]
mod it_clips;
#[cfg(all(test, not(target_family = "wasm")))]
mod it_failover;
#[cfg(all(test, not(target_family = "wasm")))]
mod it_files;
#[cfg(all(test, not(target_family = "wasm")))]
mod it_frame;
#[cfg(all(test, not(target_family = "wasm")))]
mod it_live;
#[cfg(all(test, not(target_family = "wasm")))]
mod it_ops;
#[cfg(all(test, not(target_family = "wasm")))]
mod it_proposal_wire;
#[cfg(all(test, not(target_family = "wasm")))]
mod it_robust;
#[cfg(all(test, not(target_family = "wasm")))]
mod it_scan;
#[cfg(all(test, not(target_family = "wasm")))]
mod it_stream;
#[cfg(all(test, not(target_family = "wasm")))]
mod it_points;
#[cfg(all(test, not(target_family = "wasm")))]
mod it_routes;
mod routes;
mod rules;
mod runtime;
mod status;
mod scanframe;
mod score;
mod trial;

pub const MODULE_ID: &str = "tv.kroma.live";
pub const DATA_DIR: &str = "live";

use kroma_module_sdk::EmbeddedModule;
pub const MODULE: EmbeddedModule = kroma_module_sdk::embedded_module!();

pub use module::{server_module, LiveModule};
pub use routes::{point_routes, routes};
pub use runtime::{Live, Programs};
