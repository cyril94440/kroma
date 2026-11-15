mod admin;
mod points;
mod points_ops;
mod viewer;

use std::sync::Arc;

use axum::response::Response;
use axum::Router;
use kroma_module_sdk::domain::{Permission, User};
use kroma_module_sdk::host::{service, HostCtx};

use crate::runtime::Live;

pub use points::point_routes;

pub fn routes<S: HostCtx + Clone + Send + Sync + 'static>() -> Router<S> {
    viewer::routes::<S>().merge(admin::routes::<S>())
}

pub(crate) fn live<S: HostCtx>(host: &S) -> Result<Arc<Live>, Response> {
    service::<Live>(host).ok_or_else(|| kroma_module_sdk::host::json_error(axum::http::StatusCode::SERVICE_UNAVAILABLE, "live is not running"))
}

fn permitted<S: HostCtx>(host: &S, user: &User, permission: Permission) -> Result<(), Response> {
    if !user.can(permission) {
        return Err(kroma_module_sdk::host::json_error(axum::http::StatusCode::FORBIDDEN, "forbidden"));
    }
    host.require(user, permission)
}

pub(crate) fn watcher<S: HostCtx>(host: &S, user: &User) -> Result<(), Response> {
    permitted(host, user, Permission::Playback)
}

pub(crate) fn admin_only<S: HostCtx>(host: &S, user: &User) -> Result<(), Response> {
    permitted(host, user, Permission::SettingsManage)
}
