mod admin;
mod points;
mod viewer;

use std::sync::Arc;

use axum::response::Response;
use axum::Router;
use kroma_module_sdk::domain::{Permission, User};
use kroma_module_sdk::host::{service, HostCtx};

use crate::runtime::Live;

pub use points::point_routes;

pub fn routes<S: HostCtx + Clone + Send + Sync + 'static>() -> Router<S> {
    viewer::routes::<S>().merge(admin::routes::<S>()).merge(point_routes::<S>())
}

pub(crate) fn live<S: HostCtx>(host: &S) -> Result<Arc<Live>, Response> {
    service::<Live>(host).ok_or_else(|| kroma_module_sdk::host::json_error(axum::http::StatusCode::SERVICE_UNAVAILABLE, "live is not running"))
}

pub(crate) fn watcher<S: HostCtx>(host: &S, user: &User) -> Result<(), Response> {
    host.require(user, Permission::Playback)
}

pub(crate) fn admin_only<S: HostCtx>(host: &S, user: &User) -> Result<(), Response> {
    host.require(user, Permission::SettingsManage)
}
