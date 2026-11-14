use std::sync::Arc;

use kroma_module_runtime::RemoteHost;
use kroma_module_sdk::host::{HostCtx, HostStorage};

kroma_module_runtime::module_main!(
    |host: &RemoteHost| {
        let context: Arc<dyn HostCtx> = Arc::new(host.clone());
        let live = kroma_live::Live::new(context, host.store().clone(), host.data_dir().join(kroma_live::DATA_DIR), kroma_live::Programs::default());
        host.register_service(live);
        kroma_live::routes::<RemoteHost>()
    },
    vec![kroma_live::server_module::<RemoteHost>()],
);
