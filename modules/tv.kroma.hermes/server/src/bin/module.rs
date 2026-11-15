use std::sync::Arc;

use kroma_module_runtime::RemoteHost;
use kroma_module_sdk::host::{HostCtx, HostStorage};

kroma_module_runtime::module_main!(
    |host: &RemoteHost| {
        let ctx: Arc<dyn HostCtx> = Arc::new(host.clone());
        host.register_service(kroma_hermes::build(ctx, host.store().clone()));
        axum::Router::new()
    },
    vec![kroma_hermes::server_module::<RemoteHost>()],
);
