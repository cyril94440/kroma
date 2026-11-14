//! The AllDebrid engine as a standalone process (its `.kmod` entrypoint).
//!
//! It answers the `download-client` point under the instance `alldebrid`, and
//! nothing else: no admin routes, no database, no lifecycle.

use kroma_module_runtime::RemoteHost;

kroma_module_runtime::module_main!(
    |_host| kroma_alldebrid::port::routes(),
    vec![kroma_alldebrid::server_module::<RemoteHost>()],
);
