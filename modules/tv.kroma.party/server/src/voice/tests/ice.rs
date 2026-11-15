use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use serde_json::json;

use kroma_module_sdk::host::testing::StubHost;

use super::super::ice::{ice_servers, IceCache, IceServer, TurnSource, DEFAULT_STUN};

const NOW: i64 = 1_700_000_000_000;
const HOUR: i64 = 3_600_000;

struct FakeTurn {
    calls: AtomicUsize,
    fail: Mutex<bool>,
    seen: Mutex<Vec<(String, String)>>,
}

impl FakeTurn {
    fn new() -> Arc<Self> {
        Arc::new(Self { calls: AtomicUsize::new(0), fail: Mutex::new(false), seen: Mutex::new(Vec::new()) })
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl TurnSource for FakeTurn {
    fn mint(&self, key_id: &str, api_token: &str, _ttl_s: u32) -> Result<Vec<IceServer>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.seen.lock().unwrap().push((key_id.into(), api_token.into()));
        if *self.fail.lock().unwrap() {
            return Err(anyhow!("HTTP 500"));
        }
        Ok(vec![IceServer {
            urls: vec!["turn:relay.example:3478".into()],
            username: Some("minted-user".into()),
            credential: Some("minted-secret".into()),
        }])
    }
}

fn cloudflare() -> StubHost {
    StubHost::with_db("ice")
        .with_config("voiceCloudflareKeyId", json!("key-1"))
        .with_config("voiceCloudflareApiToken", json!("token-1"))
}

fn serve(host: &StubHost, turn: &FakeTurn, cache: &Mutex<IceCache>, now: i64) -> Vec<IceServer> {
    ice_servers(host, turn, cache, now)
}

#[test]
fn defaults_to_the_public_stun_server_alone() {
    let host = StubHost::with_db("ice");
    let cache = Mutex::new(IceCache::default());

    let servers = serve(&host, &FakeTurn::new(), &cache, NOW);

    assert_eq!(servers, vec![IceServer { urls: vec![DEFAULT_STUN.into()], username: None, credential: None }]);
}

#[test]
fn serves_the_configured_stun_and_static_turn_with_their_credentials() {
    let host = StubHost::with_db("ice")
        .with_config("voiceStunUrls", json!("stun:a.example:3478, stun:b.example:3478"))
        .with_config("voiceTurnUrls", json!("turn:t.example:3478 turns:t.example:5349"))
        .with_config("voiceTurnUsername", json!("family"))
        .with_config("voiceTurnCredential", json!("static-secret"));
    let cache = Mutex::new(IceCache::default());

    let servers = serve(&host, &FakeTurn::new(), &cache, NOW);

    assert_eq!(servers.len(), 2);
    assert_eq!(servers[0].urls, vec!["stun:a.example:3478", "stun:b.example:3478"]);
    assert_eq!(servers[1].urls, vec!["turn:t.example:3478", "turns:t.example:5349"]);
    assert_eq!(servers[1].username.as_deref(), Some("family"));
    assert_eq!(servers[1].credential.as_deref(), Some("static-secret"));
}

#[test]
fn ignores_urls_that_are_not_ice_schemes() {
    let host = StubHost::with_db("ice")
        .with_config("voiceStunUrls", json!("http://evil.example, javascript:alert(1)"))
        .with_config("voiceTurnUrls", json!("file:///etc/passwd"));
    let cache = Mutex::new(IceCache::default());

    let servers = serve(&host, &FakeTurn::new(), &cache, NOW);

    assert!(servers.is_empty());
}

#[test]
fn minted_cloudflare_credentials_replace_the_static_list_and_are_cached() {
    let host = cloudflare();
    let turn = FakeTurn::new();
    let cache = Mutex::new(IceCache::default());

    let first = serve(&host, &turn, &cache, NOW);
    let second = serve(&host, &turn, &cache, NOW + 11 * HOUR);

    assert_eq!(first, second);
    assert_eq!(first[0].credential.as_deref(), Some("minted-secret"));
    assert_eq!(turn.calls(), 1);
    assert_eq!(turn.seen.lock().unwrap()[0], ("key-1".to_string(), "token-1".to_string()));
}

#[test]
fn mints_again_after_the_refresh_interval() {
    let host = cloudflare();
    let turn = FakeTurn::new();
    let cache = Mutex::new(IceCache::default());
    serve(&host, &turn, &cache, NOW);

    serve(&host, &turn, &cache, NOW + 12 * HOUR);

    assert_eq!(turn.calls(), 2);
}

#[test]
fn a_failed_mint_falls_back_to_stun_and_is_retried_only_after_a_minute() {
    let host = cloudflare();
    let turn = FakeTurn::new();
    *turn.fail.lock().unwrap() = true;
    let cache = Mutex::new(IceCache::default());

    let first = serve(&host, &turn, &cache, NOW);
    let soon = serve(&host, &turn, &cache, NOW + 30_000);
    let later = serve(&host, &turn, &cache, NOW + 61_000);

    assert_eq!(first[0].urls, vec![DEFAULT_STUN]);
    assert_eq!(soon, first);
    assert_eq!(later, first);
    assert_eq!(turn.calls(), 2);
}

#[test]
fn keeps_serving_the_previous_credentials_while_a_refresh_fails_until_they_expire() {
    let host = cloudflare();
    let turn = FakeTurn::new();
    let cache = Mutex::new(IceCache::default());
    let minted = serve(&host, &turn, &cache, NOW);
    *turn.fail.lock().unwrap() = true;

    let stale = serve(&host, &turn, &cache, NOW + 13 * HOUR);
    let expired = serve(&host, &turn, &cache, NOW + 25 * HOUR);

    assert_eq!(stale, minted);
    assert_eq!(expired[0].urls, vec![DEFAULT_STUN]);
}

#[test]
fn a_cloudflare_key_without_a_token_is_not_used() {
    let host = StubHost::with_db("ice").with_config("voiceCloudflareKeyId", json!("key-1"));
    let turn = FakeTurn::new();
    let cache = Mutex::new(IceCache::default());

    serve(&host, &turn, &cache, NOW);

    assert_eq!(turn.calls(), 0);
}
