use std::sync::Arc;

use kroma_module_runtime::RemoteHost;
use kroma_module_sdk::host::{HostCtx, HostStorage};
use kroma_party::{Party, PeerLive};

kroma_module_runtime::module_main!(
    |host: &RemoteHost| {
        let ctx: Arc<dyn HostCtx> = Arc::new(host.clone());
        let party = Party::new(ctx.clone(), host.store().clone(), Arc::new(PeerLive::new(ctx)));
        host.register_service(party.clone());
        kroma_party::point_routes::<RemoteHost>(party)
    },
    vec![kroma_party::server_module::<RemoteHost>()],
);
