use std::sync::Arc;

use axum::Extension;
use kroma_module_runtime::RemoteHost;
use kroma_module_sdk::host::{HostCtx, HostStorage};

kroma_module_runtime::module_main!(
    |host: &RemoteHost| {
        let ctx: Arc<dyn HostCtx> = Arc::new(host.clone());
        let sport = kroma_sport::build(ctx, host.store().clone());
        host.register_service(sport.clone());
        kroma_sport::tool_routes::<RemoteHost>().layer(Extension(sport))
    },
    vec![kroma_sport::server_module::<RemoteHost>()],
);
