use std::sync::Arc;
use std::time::{Duration, Instant};

use kroma_module_sdk::host::testing::StubHost;
use kroma_module_sdk::host::HostCtx;
use serde_json::json;

use crate::db::test_pool;
use crate::fake_portal::FakePortal;
use crate::model::ProviderId;
use crate::runtime::{now_ms, Live, Programs, SwitchRequest};
use crate::status::Phase;

pub struct Rig {
    pub live: Arc<Live>,
    pub host: StubHost,
    pub primary: FakePortal,
    pub backup: FakePortal,
    pub pool: kroma_module_sdk::db::testing::TempPool,
}

pub fn rig_with(programs: Programs, tweak: impl Fn(StubHost) -> StubHost) -> Rig {
    let primary = FakePortal::start();
    let backup = FakePortal::start();
    let host = tweak(StubHost::new().with_config("primaryUrl", json!(primary.url())).with_config("backupUrl", json!(backup.url())));
    let pool = test_pool();
    let root = host.data_dir().join("live");
    std::fs::create_dir_all(root.join("public")).unwrap();
    let live = Live::new(Arc::new(host.clone()), pool.clone(), root, programs);
    Rig { live, host, primary, backup, pool }
}

pub fn rig() -> Rig {
    rig_with(Programs::default(), |h| h)
}

pub async fn until(limit: Duration, mut check: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + limit;
    while Instant::now() < end {
        if check() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    check()
}

pub fn load(rig: &Rig) {
    let results = rig.live.refresh_catalogues();
    assert!(results.iter().all(|(_, r)| r.is_ok()), "{results:?}");
    rig.live.refresh_epg().unwrap();
}

pub fn request(id: &str) -> SwitchRequest {
    SwitchRequest { channel_id: id.into(), ..SwitchRequest::default() }
}

pub fn playlist_of(rig: &Rig) -> Option<String> {
    let path = rig.live.status().playlist?;
    std::fs::read_to_string(rig.live.public_dir().join(path)).ok()
}

#[tokio::test(flavor = "multi_thread")]
async fn both_catalogues_and_the_guide_load_from_the_portals() {
    let rig = rig();

    load(&rig);

    let primary = rig.live.providers.catalogue(ProviderId::Primary);
    assert_eq!(primary.len(), 5, "the adult channel is filtered out at load");
    assert!(primary.by_stream_id(5).is_none());
    assert_eq!(rig.live.providers.catalogue(ProviderId::Backup).len(), 5);
    let page = rig.live.list_channels(&crate::directory::ChannelQuery { q: String::new(), page: 1, per_page: 50, ..Default::default() });
    assert_eq!(page.total, 4, "the 4K channel is kept out of the listing");
    assert!(page.channels.iter().any(|c| c.name == "FR: TF1 FHD" && c.now.as_ref().is_some_and(|n| n.title == "Journal")));
    assert!(rig.live.groups(ProviderId::Primary).contains(&"General".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_guide_finds_a_programme_by_title_and_maps_it_to_a_channel() {
    let rig = rig();
    load(&rig);

    let found = rig.live.guide(Some("le match PSG Liverpool"), None, None, 5);

    assert_eq!(found.len(), 1);
    assert_eq!((found[0].title.as_str(), found[0].channel_name.as_str(), found[0].channel_id.as_str()), ("PSG Liverpool", "VIP: CANAL+ ᴿᴬᵂ", "p:2"));
    assert!(found[0].end > found[0].start && found[0].start % 1000 == 0);
    assert!(rig.live.guide(Some("nothing like this"), None, None, 5).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn resolving_a_request_applies_the_house_rules() {
    let rig = rig();
    load(&rig);

    let resolved = rig.live.resolve("canal plus vip raw", 3);
    let four_k = rig.live.resolve("arte", 3);
    let wanted_4k = rig.live.resolve("arte 4k", 3);

    assert_eq!(resolved.first().map(|c| c.name.as_str()), Some("VIP: CANAL+ ᴿᴬᵂ"));
    assert!(four_k.is_empty(), "never 4K unless asked");
    assert_eq!(wanted_4k.len(), 1);
    assert_eq!(rig.live.favourites().iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["VIP: CANAL+ ᴿᴬᵂ", "FR: TF1 FHD", "FR: M6 HD"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_refresh_keeps_the_catalogue_and_says_why_without_the_credentials() {
    let rig = rig();
    load(&rig);
    rig.host.set_config("primaryUrl", json!("http://127.0.0.1:1/get.php?username=TESTUSER&password=TESTPASS42"));

    let results = rig.live.refresh_catalogues();

    let error = results.iter().find(|(p, _)| *p == ProviderId::Primary).unwrap().1.clone().unwrap_err();
    assert!(!error.contains("TESTPASS42") && !error.contains("TESTUSER"), "{error}");
    assert_eq!(rig.live.providers.catalogue(ProviderId::Primary).len(), 5);
    let state = crate::db::catalogue_state(&rig.pool, ProviderId::Primary).unwrap();
    assert!(state.error.is_some() && state.channels == 5);
    let _ = now_ms();
    let _ = Phase::Running;
}
