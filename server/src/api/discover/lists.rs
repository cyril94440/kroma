use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::api::error::json_error;
use crate::api::extract::AuthUser;
use crate::api::permission::require;
use crate::api::util::blocking;
use crate::i18n::AskedLocale;
use crate::infra::metadata::discover::{self, DiscoverList};
use crate::model::{DiscoverGenre, DiscoverResponse, Permission, RequestKind};
use crate::services::settings;
use crate::state::SharedState;

use super::flags::flag_hits;
use super::{require_tmdb, scope_from_type};

#[derive(Debug, Deserialize)]
pub struct ListParams {
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    page: Option<u32>,
}

fn single_kind(kind: Option<&str>) -> Result<RequestKind, Response> {
    match kind {
        Some("movie") => Ok(RequestKind::Movie),
        Some("tv") | Some("show") => Ok(RequestKind::Show),
        _ => Err(json_error(
            StatusCode::BAD_REQUEST,
            "type must be movie or tv",
        )),
    }
}

/// `GET /api/discover/{trending|popular|upcoming|top-rated}?type=&page=`. No
/// `type` = movies and shows merged (page 1) for the discover rails;
/// `type=movie|tv` = a paginated single-kind list backing the full page.
/// `upcoming` is `movie/upcoming` for movies and `tv/on_the_air` for shows.
pub async fn browse(
    State(state): State<SharedState>,
    AskedLocale(reader): AskedLocale,
    AuthUser(user): AuthUser,
    Path(name): Path<String>,
    Query(params): Query<ListParams>,
) -> Result<Response, Response> {
    require(&user, Permission::RequestsCreate)?;
    let Some(list) = DiscoverList::parse(&name) else {
        return Err(json_error(StatusCode::NOT_FOUND, "unknown discover list"));
    };
    let key = require_tmdb(&state, &user)?;
    let lang = settings::metadata_language_for(&state.settings, &state.config, reader);
    let scope = scope_from_type(params.kind.as_deref());
    let page = params.page.unwrap_or(1);
    let out = blocking(move || {
        let found = discover::list(&key, &lang, list, scope, page)
            .map_err(|()| anyhow::anyhow!("TMDB list failed"))?;
        let conn = state.db.get()?;
        Ok(DiscoverResponse {
            results: flag_hits(&conn, found.hits, &user)?,
            page: found.page,
            total_pages: found.total_pages,
        })
    })
    .await?;
    Ok(Json(out).into_response())
}

/// `GET /api/discover/genres?type=movie|tv` the genres to browse, named in the
/// viewer's language.
pub async fn genres(
    State(state): State<SharedState>,
    AskedLocale(reader): AskedLocale,
    AuthUser(user): AuthUser,
    Query(params): Query<ListParams>,
) -> Result<Response, Response> {
    require(&user, Permission::RequestsCreate)?;
    let kind = single_kind(params.kind.as_deref())?;
    let key = require_tmdb(&state, &user)?;
    let lang = settings::metadata_language_for(&state.settings, &state.config, reader);
    let found = blocking(move || {
        discover::genres(&key, &lang, kind).map_err(|()| anyhow::anyhow!("TMDB genres failed"))
    })
    .await?;
    let genres: Vec<DiscoverGenre> = found
        .into_iter()
        .map(|g| DiscoverGenre {
            id: g.id,
            name: g.name,
        })
        .collect();
    Ok(Json(json!({ "genres": genres })).into_response())
}

/// `GET /api/discover/genre/:genreId?type=movie|tv&page=` the most popular
/// titles of one genre.
pub async fn by_genre(
    State(state): State<SharedState>,
    AskedLocale(reader): AskedLocale,
    AuthUser(user): AuthUser,
    Path(genre_id): Path<u32>,
    Query(params): Query<ListParams>,
) -> Result<Response, Response> {
    require(&user, Permission::RequestsCreate)?;
    let kind = single_kind(params.kind.as_deref())?;
    let key = require_tmdb(&state, &user)?;
    let lang = settings::metadata_language_for(&state.settings, &state.config, reader);
    let page = params.page.unwrap_or(1);
    let out = blocking(move || {
        let found = discover::list_by_genre(&key, &lang, kind, genre_id, page)
            .map_err(|()| anyhow::anyhow!("TMDB genre browse failed"))?;
        let conn = state.db.get()?;
        Ok(DiscoverResponse {
            results: flag_hits(&conn, found.hits, &user)?,
            page: found.page,
            total_pages: found.total_pages,
        })
    })
    .await?;
    Ok(Json(out).into_response())
}
