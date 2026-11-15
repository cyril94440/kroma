# tv.kroma.sport

Live sport for the family watch party: "Sport en direct" list, live match panel, anti-spoiler timing, match predictions (pronos), standings, chat cards and goal clips. A KROMA WASM module (server half in `server/`, web half in `ui/`). Port of the sport layer of hermes-tv.

Depends on `tv.kroma.live` (>= 0.1.0). Optional peers: `tv.kroma.party` (chat cards through `room.say`), `tv.kroma.hermes` (this module contributes the `tools` point).

## Data sources and terms

All network access goes through the declared `net.connect` purpose `espn`: `site.api.espn.com:443` only.

- `/apis/site/v2/sports/{sport}/{league}/scoreboard?dates=YYYYMMDD` (one day per request; a date range returns 400), plus `/all/scoreboard`.
- `/apis/site/v2/sports/{sport}/{league}/summary?event={id}` for the live match (lineups, stats, events).
- Standings from the same host.

These are ESPN's unofficial, unauthenticated endpoints. They are not a documented public API, carry no SLA or licence, and may change or be withdrawn at any time. The module sends no credentials, polls conservatively (one network read per 100 ms tick, 20 s match cadence) and degrades to "no data" when ESPN is unreachable. Check ESPN's terms before any use beyond a private household.

`espnBaseUrl` (config) overrides the host for tests; the manifest only permits `site.api.espn.com`.

Peers: `tv.kroma.live/control` (status, epg, resolve required; clipBufferInfo, clipCut, clipList, clipDelete, scanResult, benchmark optional, 404/405 means unsupported and degrades). `status.startedAt` (live 0.3.0) keys the calibration and the pronos per direct, `clipList` reconciles the stored clips with the files live still holds, `clipDelete` removes a file when its clip is dropped or deleted, `tv.kroma.party/room` (`say` with `as:"system"` and a card).

## Anti-spoiler model

The picture lags real time. Reveal rule: with an image time and a frame time, reveal when `pdt >= t_image`; otherwise `now >= t_vu + delay`.

- Default delay `spoilerDelaySeconds` = 60, margin 15 s over the measured median.
- Measures come from `POST /calibration {clock, capturedAtMs}` (the UI sends the frame time at input focus) or the `calibrate_clock` tool (capture time = now - 3 s, biased late, i.e. safe). Delay = `(capture - chrono) - (ESPN minute change - (clock - 60))`.
- Precision: about +-10 s per measure (the ESPN change is seen at the 20 s poll), plus human reaction time for typed clocks. Median of the last 7 measures, kept 24 h, keyed per direct (provider, channel, startedAt).
- No vision in this module: no reading from the image is performed.
- Chat goal announcements wait for reveal + `revealMarginSeconds` (8 s); goals already older than 120 s at first sight are marked silently.

## Contracts

HTTP (user routes via `admin_routes`, permissions checked): `GET /state /view /match /standings /clips /leaderboard /trace`, `POST /open /watch /calibration /refresh /prono /prono/vote /prono/start`, `DELETE /clips/{id}`.

Events (`sport.<topic>`) carry a persisted `seq`; `GET /state` returns the current `seq`. Clients drop `seq <=` their own and refetch on a gap.

```json
{"type":"sport.view","seq":42,"payload":{"statut":"en_cours","equipes":{"dom":"PSG","ext":"Monaco"},"t_image":1731632000000,"t_vu":1731632012000,"delai_s":60}}
```

Tools (point `tv.kroma.hermes/tools`, routes `/_port/tv.kroma.hermes/tools/{list,call}`): `match_facts`, `compos_match`, `live_standings`, `start_prono`, `sport_channel_hint`, `calibrate_clock`.

`start_prono` has two doors. `POST /prono/start` is the administration button and needs `settings.manage`. The tool, reached by the assistant for any family member who asks, takes no role: the module never receives who asked, and the code alone decides on the match window as in the source product (a kick-off close, or at the latest the 10th minute; a prono already open, an unknown match or an unreachable match source are refused with their reason, which the assistant relays). The module keeps no administrator cache.

The list `match_facts` answers when no match is named shows a match ESPN calls finished as still in progress, without its score, until the picture has caught up: the end of the match is estimated at 115 minutes after the kick-off, plus the reveal delay, like the facts of a single match.

Contract v1 keeps the source's French field names (`statut`, `equipes`, `joueurs`, `periodes`, `evenements`, `votes`, `requis`, `distribution`, `classement_match`, `resultats`, `saison`, `debut_ms`, `fin_ms`, `duree_s`).

## Differences from hermes-tv

- Votes and `requis` are keyed by KROMA user id with a `noms` map; lists add `user_id`.
- Clips carry `file{module,path}` (file served by tv.kroma.live); metadata in this module's sqlite.
- Chat announcements go through party `say`.
- No vision readings; the capture scan is mapped to rows from the guide `now` title, certainty capped at "a_confirmer".
- `sport_channel_hint` replaces the web-search loop.
- Skipped: stream history chart, polls tie-in, recette-match scripts, title-based `Pronos.detecter` (replaced by `decide()`).

## Config

enabled, competitions, includeTennis/FormulaOne/Golf, poll*Seconds, matchPollSeconds, detectionSeconds, spoilerDelaySeconds, spoilerMarginSeconds, revealMarginSeconds, pronoEnabled, goalAnnouncements, clipsEnabled, clipRetentionDays, homeCountries, announceLanguage, espnBaseUrl.

## Tests

`cd server && cargo test --lib` (fake ESPN on a real socket fed by `server/fixtures/`, fake peers), `bun run ci rust wasm-test --only tv.kroma.sport` (socket-based tests gated out on wasm), `bun x vitest run modules/tv.kroma.sport`.
