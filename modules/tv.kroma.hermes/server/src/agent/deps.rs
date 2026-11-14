use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use kroma_module_sdk::host::HostCtx;
use serde_json::Value;

use crate::blackbox::mask_json;
use crate::clock::Clock;
use crate::model::{Model, ModelSource};
use crate::peers::{Contributors, Control, Room};
use crate::rules::Rules;
use crate::settings::Settings;
use crate::store::Store;
use crate::webcache::{TtlCache, TTL_MS};

const SNAPSHOT_RETRY_MS: i64 = 600_000;
const HOUR_MS: i64 = 3_600_000;
const NOTICE_COOLDOWN_MS: i64 = 600_000;

pub struct Deps {
    pub host: Arc<dyn HostCtx>,
    pub room: Arc<dyn Room>,
    pub control: Arc<dyn Control>,
    pub contributors: Arc<dyn Contributors>,
    pub store: Store,
    pub rules: Rules,
    pub clock: Clock,
    pub models: ModelSource,
    pub web_cache: TtlCache<Value>,
    pub hint_cache: TtlCache<Vec<String>>,
    recent_suggestions: Mutex<HashMap<(String, String), i64>>,
    snapshot_off_until: AtomicI64,
    notice_at: AtomicI64,
}

impl Deps {
    #[allow(clippy::too_many_arguments)]
    pub fn new(host: Arc<dyn HostCtx>, room: Arc<dyn Room>, control: Arc<dyn Control>, contributors: Arc<dyn Contributors>, store: Store, clock: Clock, models: ModelSource) -> Self {
        Self {
            host,
            room,
            control,
            contributors,
            rules: Rules::new(store.clone()),
            store,
            clock,
            models,
            web_cache: TtlCache::new(TTL_MS),
            hint_cache: TtlCache::new(TTL_MS),
            recent_suggestions: Mutex::new(HashMap::new()),
            snapshot_off_until: AtomicI64::new(0),
            notice_at: AtomicI64::new(i64::MIN / 2),
        }
    }

    pub fn now(&self) -> i64 {
        (self.clock)()
    }

    pub fn settings(&self) -> Settings {
        Settings::read(&*self.host)
    }

    pub fn model(&self) -> Option<Arc<dyn Model>> {
        (self.models)()
    }

    /// Writes one record of the audit log with every secret and URL masked.
    pub fn audit(&self, genre: &str, event: &str, data: Value) {
        let secrets = self.settings().secrets();
        self.store.record(genre, event, &mask_json(data, &secrets), self.now());
    }

    pub fn mask(&self, text: &str) -> String {
        crate::blackbox::mask(text, &self.settings().secrets())
    }

    pub fn seen_suggestion(&self, kind: &str, title: &str, now: i64, window: i64) -> bool {
        let mut recent = self.recent_suggestions.lock().unwrap_or_else(|e| e.into_inner());
        recent.retain(|_, at| now - *at <= window);
        recent.insert((kind.to_string(), title.to_lowercase()), now).is_some()
    }

    pub fn mark_snapshot_unsupported(&self, now: i64) {
        self.snapshot_off_until.store(now + SNAPSHOT_RETRY_MS, Ordering::SeqCst);
    }

    pub fn snapshot_supported(&self, now: i64) -> bool {
        now >= self.snapshot_off_until.load(Ordering::SeqCst)
    }

    pub fn take_notice_slot(&self, now: i64) -> bool {
        let last = self.notice_at.load(Ordering::SeqCst);
        now - last >= NOTICE_COOLDOWN_MS && self.notice_at.compare_exchange(last, now, Ordering::SeqCst, Ordering::SeqCst).is_ok()
    }

    pub fn calls_this_hour(&self) -> u32 {
        self.store.audit_count_since("llm", "call", self.now() - HOUR_MS).unwrap_or(0)
    }
}
