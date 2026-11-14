use std::sync::Arc;

use kroma_module_runtime::RemoteHost;
use kroma_module_sdk::host::{HostCtx, HostStorage};

kroma_module_runtime::module_main!(
    |host: &RemoteHost| {
        let ctx: Arc<dyn HostCtx> = Arc::new(host.clone());
        let agent = kroma_hermes::build(ctx, host.store().clone());
        host.register_service(agent.clone());
        kroma_hermes::routes::<RemoteHost>(agent)
    },
    vec![kroma_hermes::server_module::<RemoteHost>()],
);
