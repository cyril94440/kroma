use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use kroma_module_sdk::domain::Permission;
use kroma_module_sdk::host::{json_error, AuthUser, HostCtx};
use serde::{Deserialize, Serialize};

use super::{live, watcher};
use crate::directory::{ChannelPage, ChannelQuery, ChannelView};
use crate::model::{Programme, ProviderId};
use crate::runtime::{now_ms, SwitchOutcome, SwitchRequest};
use crate::status::Status;

pub fn routes<S: HostCtx + Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/status", get(status::<S>))
        .route("/channels", get(channels::<S>))
        .route("/favourites", get(favourites::<S>))
        .route("/epg/search", get(epg_search::<S>))
        .route("/epg/grid", get(epg_grid::<S>))
        .route("/presence", post(beat::<S>))
        .route("/presence/leave", post(leave::<S>))
        .route("/switch", post(switch::<S>))
}

async fn status<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, AuthUser(user): AuthUser) -> Result<Json<Status>, Response> {
    watcher(&host, &user)?;
    Ok(Json(live(&host)?.status()))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ChannelParams {
    q: String,
    group: String,
    provider: Option<ProviderId>,
    page: usize,
    per_page: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChannelsAnswer {
    #[serde(flatten)]
    page: ChannelPage,
    groups: Vec<String>,
}

async fn channels<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, AuthUser(user): AuthUser, Query(params): Query<ChannelParams>) -> Result<Json<ChannelsAnswer>, Response> {
    watcher(&host, &user)?;
    let live = live(&host)?;
    let provider = params.provider.unwrap_or(ProviderId::Primary);
    let query = ChannelQuery { q: params.q, group: params.group, provider: Some(provider), page: params.page.max(1), per_page: if params.per_page == 0 { 48 } else { params.per_page } };
    let page = live.list_channels(&query);
    Ok(Json(ChannelsAnswer { page, groups: live.groups(provider) }))
}

async fn favourites<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, AuthUser(user): AuthUser) -> Result<Json<Vec<ChannelView>>, Response> {
    watcher(&host, &user)?;
    Ok(Json(live(&host)?.favourites()))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GuideParams {
    q: String,
    from: Option<i64>,
    to: Option<i64>,
    limit: usize,
    channels: String,
}

async fn epg_search<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, AuthUser(user): AuthUser, Query(params): Query<GuideParams>) -> Result<Json<Vec<Programme>>, Response> {
    watcher(&host, &user)?;
    let limit = if params.limit == 0 { 20 } else { params.limit };
    Ok(Json(live(&host)?.guide(Some(&params.q), params.from, params.to, limit)))
}

async fn epg_grid<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, AuthUser(user): AuthUser, Query(params): Query<GuideParams>) -> Result<Json<Vec<Programme>>, Response> {
    watcher(&host, &user)?;
    let live = live(&host)?;
    let now = now_ms();
    let (from, to) = (params.from.unwrap_or(now - 2 * 3600 * 1000), params.to.unwrap_or(now + 12 * 3600 * 1000));
    let ids: Vec<String> = params.channels.split(',').map(str::trim).filter(|s| !s.is_empty()).take(60).map(str::to_string).collect();
    let ids = if ids.is_empty() { live.favourites().into_iter().map(|c| c.id).collect() } else { ids };
    Ok(Json(live.grid(&ids, from, to)))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Beat {
    companion: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BeatAnswer {
    viewers: usize,
    heartbeat_seconds: u64,
}

async fn beat<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, AuthUser(user): AuthUser, body: Option<Json<Beat>>) -> Result<Json<BeatAnswer>, Response> {
    watcher(&host, &user)?;
    let live = live(&host)?;
    let companion = body.is_some_and(|b| b.0.companion);
    if !companion {
        let (joined, paused) = {
            let mut inner = live.lock();
            let joined = inner.presence.beat(&user.id, &user.username, now_ms());
            (joined, inner.phase == crate::status::Phase::Paused)
        };
        if joined {
            live.publish_presence();
        }
        if paused {
            let waking = live.clone();
            tokio::spawn(async move {
                waking.resume().await;
            });
        }
    }
    Ok(Json(BeatAnswer { viewers: live.viewer_count(), heartbeat_seconds: crate::presence::HEARTBEAT_SECONDS }))
}

async fn leave<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, AuthUser(user): AuthUser) -> Result<StatusCode, Response> {
    watcher(&host, &user)?;
    let live = live(&host)?;
    let removed = live.lock().presence.leave(&user.id, now_ms());
    if removed {
        live.publish_presence();
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SwitchBody {
    channel_id: String,
    query: String,
}

async fn switch<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, AuthUser(user): AuthUser, Json(body): Json<SwitchBody>) -> Result<Json<SwitchOutcome>, Response> {
    watcher(&host, &user)?;
    let live = live(&host)?;
    let admin = user.can(Permission::SettingsManage);
    if !admin && live.party_present() {
        let others = live.lock().presence.viewers().iter().any(|v| v.user_id != user.id);
        if others {
            return Err(json_error(StatusCode::FORBIDDEN, "useVote").into_response());
        }
    }
    let channel_id = if body.channel_id.is_empty() {
        live.resolve(&body.query, 1).into_iter().next().map(|c| c.id).ok_or_else(|| json_error(StatusCode::NOT_FOUND, "unknownChannel"))?
    } else {
        body.channel_id
    };
    let outcome = live.switch(SwitchRequest { channel_id, requested_by: Some(user.id.clone()), reason: "viewer".into(), ..SwitchRequest::default() }).await;
    Ok(Json(outcome))
}
