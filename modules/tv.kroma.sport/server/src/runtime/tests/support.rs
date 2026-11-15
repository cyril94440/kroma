use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use kroma_module_sdk::host::testing::StubHost;
use kroma_module_sdk::host::{HostCtx, HostStorage};
use serde_json::{json, Value};

use crate::espn::fake::FakeEspn;
use crate::espn::{scoreboard, summary, HttpEspn};
use crate::iso::parse_secs;
use crate::peers::fakes::{FakeLive, FakeRoom};
use crate::peers::{Candidate, ChannelRef, LiveStatus, Programme, Slot, Viewer};
use crate::runtime::Sport;
use crate::store::Store;

pub const CANAL_FOOT: &str = "primary:canalfoot";

pub struct Rig {
    pub espn: FakeEspn,
    pub live: Arc<FakeLive>,
    pub room: Arc<FakeRoom>,
    pub host: StubHost,
    pub clock: Arc<AtomicI64>,
    pub sport: Arc<Sport>,
}

pub fn at(iso: &str) -> f64 {
    parse_secs(iso).unwrap()
}

pub fn scoreboard_at(now: f64) -> Value {
    let mut board: Value = serde_json::from_str(include_str!(
        "../../../fixtures/espn_all_scoreboard_20261103.json"
    ))
    .unwrap();
    for event in board["events"].as_array_mut().unwrap() {
        let start = parse_secs(event["date"].as_str().unwrap()).unwrap();
        let state = if now < start {
            "pre"
        } else if now < start + 115.0 * 60.0 {
            "in"
        } else {
            "post"
        };
        let competition = &mut event["competitions"][0];
        competition["status"]["type"]["state"] = state.into();
        competition["status"]["period"] = (if state != "post" && now - start < 47.0 * 60.0 {
            1
        } else {
            2
        })
        .into();
        if state == "in" {
            competition["status"]["displayClock"] =
                format!("{}'", ((now - start) / 60.0) as i64 + 1).into();
        }
        for competitor in competition["competitors"].as_array_mut().unwrap() {
            if competitor["score"].as_str().is_none_or(str::is_empty) {
                competitor["score"] = "0".into();
            }
        }
    }
    board
}

pub fn guide_programmes() -> Vec<Programme> {
    let epg: std::collections::BTreeMap<String, Vec<(f64, f64, String, String)>> =
        serde_json::from_str(include_str!("../../../fixtures/epg_sport_20261103.json")).unwrap();
    epg["canalplusfoot.fr"]
        .iter()
        .map(|(start, stop, title, hay)| Programme {
            channel_id: CANAL_FOOT.into(),
            channel_name: "FR: Canal+ Foot".into(),
            title: title.clone(),
            description: hay.clone(),
            start: (*start * 1000.0) as i64,
            end: (*stop * 1000.0) as i64,
        })
        .collect()
}

impl Rig {
    pub fn new(start: &str) -> Rig {
        let espn = FakeEspn::start();
        let host = StubHost::with_db("sport-rig").with_config("espnBaseUrl", json!(espn.base));
        host.store()
            .get()
            .unwrap()
            .execute_batch(crate::store::MIGRATIONS)
            .unwrap();
        let live = Arc::new(FakeLive::default());
        let room = Arc::new(FakeRoom::default());
        let clock = Arc::new(AtomicI64::new((at(start) * 1000.0) as i64));
        let ticking = clock.clone();
        let base = espn.base.clone();
        let ctx: Arc<dyn HostCtx> = Arc::new(host.clone());
        let sport = Sport::with_clock(
            ctx,
            Store::new(host.store().clone()),
            Arc::new(HttpEspn::new(move || base.clone())),
            live.clone(),
            room.clone(),
            Arc::new(move || ticking.load(Ordering::SeqCst)),
        );
        let rig = Rig {
            espn,
            live,
            room,
            host,
            clock,
            sport,
        };
        rig.watch_canal_foot(&["Ana"]);
        rig
    }

    pub fn watch_canal_foot(&self, viewers: &[&str]) {
        *self.live.status.lock().unwrap() = LiveStatus {
            state: "running".into(),
            channel: Some(ChannelRef {
                id: CANAL_FOOT.into(),
                name: "FR: Canal+ Foot".into(),
            }),
            provider: "primary".into(),
            now: Some(Slot {
                title: "UEFA Champions League".into(),
                start: (at("2026-11-03T19:55:00Z") * 1000.0) as i64,
                end: (at("2026-11-03T22:05:00Z") * 1000.0) as i64,
            }),
            viewers: viewers
                .iter()
                .map(|n| Viewer {
                    user_id: format!("u-{}", n.to_lowercase()),
                    name: n.to_string(),
                })
                .collect(),
            started_at: Some(1),
            ..LiveStatus::default()
        };
        *self.live.programmes.lock().unwrap() = guide_programmes();
        *self.live.channels.lock().unwrap() = vec![Candidate {
            id: CANAL_FOOT.into(),
            name: "FR: Canal+ Foot".into(),
            group: "FR".into(),
            score: 10.0,
        }];
        *self.live.cut_supported.lock().unwrap() = true;
    }

    pub fn now(&self) -> f64 {
        self.clock.load(Ordering::SeqCst) as f64 / 1000.0
    }

    pub fn serve_board(&self) {
        let now = self.now();
        let day = crate::iso::day_key(now);
        let previous = crate::iso::day_key(now - 24.0 * 3600.0);
        self.espn
            .serve(&scoreboard("all", &day), &scoreboard_at(now));
        self.espn
            .serve(&scoreboard("all", &previous), &json!({"events": []}));
    }

    pub fn serve_summary(&self, body: &Value) {
        self.espn.serve(&summary("all", "401915309"), body);
    }

    pub fn run(&self, seconds: i64) {
        for _ in 0..seconds {
            self.clock.fetch_add(1000, Ordering::SeqCst);
            self.serve_board();
            self.sport.tick();
        }
    }

    pub fn run_until(&self, limit_s: i64, done: impl Fn(&Rig) -> bool) -> bool {
        for _ in 0..limit_s {
            if done(self) {
                return true;
            }
            self.run(1);
        }
        done(self)
    }

    pub fn events(&self, topic: &str) -> Vec<Value> {
        self.sport
            .tap
            .lock()
            .unwrap()
            .iter()
            .filter(|(t, _)| t == topic)
            .map(|(_, p)| p.clone())
            .collect()
    }
}

pub fn villarreal_summary() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/espn_summary_villarreal_psg_live.json"
    ))
    .unwrap()
}

impl Rig {
    pub fn run_coarse(&self, seconds: i64, step: i64) {
        for _ in 0..seconds / step {
            self.clock.fetch_add(step * 1000, Ordering::SeqCst);
            self.serve_board();
            self.sport.tick();
        }
    }
}
