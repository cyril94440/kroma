use serde::Deserialize;

use crate::model::RequestKind;

use super::super::{api, tmdb_json, PURPOSE};
use super::hits::map_page;
use super::wire::PageResp;
use super::{DiscoverPage, DiscoverScope};

/// A named TMDB list a person can browse for titles the library lacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoverList {
    Trending,
    Popular,
    Upcoming,
    TopRated,
}

impl DiscoverList {
    pub fn parse(s: &str) -> Option<DiscoverList> {
        match s {
            "trending" => Some(DiscoverList::Trending),
            "popular" => Some(DiscoverList::Popular),
            "upcoming" => Some(DiscoverList::Upcoming),
            "top-rated" => Some(DiscoverList::TopRated),
            _ => None,
        }
    }

    fn path(self, kind: RequestKind) -> &'static str {
        match (self, kind) {
            (DiscoverList::Trending, RequestKind::Movie) => "trending/movie/week",
            (DiscoverList::Trending, RequestKind::Show) => "trending/tv/week",
            (DiscoverList::Popular, RequestKind::Movie) => "movie/popular",
            (DiscoverList::Popular, RequestKind::Show) => "tv/popular",
            (DiscoverList::Upcoming, RequestKind::Movie) => "movie/upcoming",
            (DiscoverList::Upcoming, RequestKind::Show) => "tv/on_the_air",
            (DiscoverList::TopRated, RequestKind::Movie) => "movie/top_rated",
            (DiscoverList::TopRated, RequestKind::Show) => "tv/top_rated",
        }
    }
}

/// One TMDB genre, named in the language the request asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Genre {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Deserialize)]
struct GenresResp {
    #[serde(default)]
    genres: Vec<GenreRow>,
}

#[derive(Debug, Deserialize)]
struct GenreRow {
    id: u32,
    name: String,
}

fn paged_params(language: &str, page: u32) -> Vec<(&'static str, String)> {
    vec![
        ("language", language.to_string()),
        ("page", page.max(1).to_string()),
    ]
}

fn fetch_page(
    api_key: &str,
    path: &str,
    params: &[(&str, String)],
    scope: DiscoverScope,
) -> Result<DiscoverPage, ()> {
    let resp: PageResp = tmdb_json(PURPOSE, &format!("{}/{path}", api()), api_key, params)?;
    Ok(map_page(resp, scope))
}

/// One page of a list. `All` is movies and shows interleaved, each side taken
/// from the same page number; the trending list answers it in one call.
// The `()` Err is the unit error threaded up from `tmdb_json` via `?`; callers only
// care ok/err, so a richer error type would add nothing here.
#[allow(clippy::result_unit_err)]
pub fn list(
    api_key: &str,
    language: &str,
    list: DiscoverList,
    scope: DiscoverScope,
    page: u32,
) -> Result<DiscoverPage, ()> {
    let params = paged_params(language, page);
    match scope {
        DiscoverScope::Movies => {
            fetch_page(api_key, list.path(RequestKind::Movie), &params, scope)
        }
        DiscoverScope::Shows => fetch_page(api_key, list.path(RequestKind::Show), &params, scope),
        DiscoverScope::All if list == DiscoverList::Trending => {
            fetch_page(api_key, "trending/all/week", &params, scope)
        }
        DiscoverScope::All => {
            let movies = fetch_page(
                api_key,
                list.path(RequestKind::Movie),
                &params,
                DiscoverScope::Movies,
            );
            let shows = fetch_page(
                api_key,
                list.path(RequestKind::Show),
                &params,
                DiscoverScope::Shows,
            );
            merge(movies, shows)
        }
    }
}

fn merge(
    movies: Result<DiscoverPage, ()>,
    shows: Result<DiscoverPage, ()>,
) -> Result<DiscoverPage, ()> {
    let (movies, shows) = match (movies, shows) {
        (Ok(m), Ok(s)) => (m, s),
        (Ok(only), Err(())) | (Err(()), Ok(only)) => return Ok(only),
        (Err(()), Err(())) => return Err(()),
    };
    let mut left = movies.hits.into_iter();
    let mut right = shows.hits.into_iter();
    let mut hits = Vec::new();
    loop {
        match (left.next(), right.next()) {
            (None, None) => break,
            (a, b) => hits.extend(a.into_iter().chain(b)),
        }
    }
    Ok(DiscoverPage {
        hits,
        page: movies.page,
        total_pages: movies.total_pages.max(shows.total_pages),
    })
}

/// The genres TMDB files movies or shows under.
// The `()` Err is the unit error threaded up from `tmdb_json` via `?`.
#[allow(clippy::result_unit_err)]
pub fn genres(api_key: &str, language: &str, kind: RequestKind) -> Result<Vec<Genre>, ()> {
    let path = match kind {
        RequestKind::Movie => "genre/movie/list",
        RequestKind::Show => "genre/tv/list",
    };
    let resp: GenresResp = tmdb_json(
        PURPOSE,
        &format!("{}/{path}", api()),
        api_key,
        &[("language", language.to_string())],
    )?;
    Ok(resp
        .genres
        .into_iter()
        .map(|g| Genre {
            id: g.id,
            name: g.name,
        })
        .collect())
}

/// One page of the most popular titles of a genre.
// The `()` Err is the unit error threaded up from `tmdb_json` via `?`.
#[allow(clippy::result_unit_err)]
pub fn list_by_genre(
    api_key: &str,
    language: &str,
    kind: RequestKind,
    genre_id: u32,
    page: u32,
) -> Result<DiscoverPage, ()> {
    let (path, scope) = match kind {
        RequestKind::Movie => ("discover/movie", DiscoverScope::Movies),
        RequestKind::Show => ("discover/tv", DiscoverScope::Shows),
    };
    let mut params = paged_params(language, page);
    params.push(("with_genres", genre_id.to_string()));
    params.push(("sort_by", "popularity.desc".to_string()));
    params.push(("include_adult", "false".to_string()));
    fetch_page(api_key, path, &params, scope)
}

#[cfg(test)]
mod tests;
