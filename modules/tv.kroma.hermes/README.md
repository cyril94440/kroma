# Hermès

The assistant of the family chat. It listens to the room (`tv.kroma.party`), reads the television (`tv.kroma.live`) and answers in French: it finds channels, answers questions about what is on, starts a channel vote or a poll, diagnoses the stream, proposes alternatives when a channel goes off air and learns the house preferences.

It is a port of the Python agent of the source product. Its standing rules are product requirements:

1. **Governance is code, never the model.** The model understands language and calls tools. A channel change goes through the room's `requestSwitch` (instant when the requester is alone, a 30 s vote otherwise), a kick through the room's `kick`, a poll through the room's polls. The requester or the voter is always the author of the message: no tool can name someone else.
2. **Broad read-only tools, failures returned.** A refused action comes back to the model with its reason and the model tries another way. Only one broadcast action (`change_channel`, `find_program`) runs per message, and none after a partial guide match.
3. **Everything leaves a trace** in the audit log (the "boîte noire"): raw sources, decisions with their refusals and reasons, measures.
4. French channels first, answers in French and short. Both come from the house rules and the live module's own scoring; this module never re-implements scoring.

## How a message is handled

```
party.message event ──▶ gate (code) ──▶ inbox (SQLite) ──▶ one turn at a time ──▶ party say / card
                          ▲                                    │ one model call or one tool batch per tick
              history reconcile every 15 s                     └─▶ tools ──▶ live / party / contributors / web
```

- **Events are best effort.** `party.message` only queues: the gate decides in code, the message is recorded once in the inbox (`INSERT OR IGNORE` on the message id), and nothing slow runs in the route that received it. Every 15 s (and in the daily job) the room history is read and any message the events missed is taken in. A message already seen, answered or not, is never taken twice, across restarts too. A turn that was running when the module stopped is marked `interrupted` and is not redone. A message older than three minutes is never answered.
- **The gate** (`agent/gate.rs`, from the source's `should_engage`): at least two characters, and either the assistant spoke within the engage window (90 s), or the message ends with `?`, or it carries a keyword, or it names the assistant. An open poll lets every message through (answers come without keyword). A typed yes/no from someone an open vote waits for belongs to the vote, which the room counts itself, so it is skipped. The lexical signals come from the `party.message` event and are recomputed from the text for messages found by the reconciliation.
- **A turn** is a state machine advanced one step per tick (a single blocking model call or one tool batch), so the single-instance component never holds the instance for a whole turn. Bounds: 10 model calls, final-only answers (`reply`, `show_card`, `ignore`) past 125 s or at the last step, a hard stop at 180 s with one last answer, a per-person limit per minute and a limit of model calls per hour. A turn that ends without having said anything posts one fallback message, except a deliberate `ignore`.
- **The activity line** ("consulte le guide TV") is sent through the room's `activity` method, from a fixed table keyed by tool, and cleared when the turn ends.

## The model

The core's LLM service is not reachable from a module, so this module calls a provider itself behind one small trait (`model.rs`):

| Provider | Endpoint | Shapes |
| --- | --- | --- |
| `gemini` | `{geminiBaseUrl}/v1beta/models/{model}:generateContent` | function calling (`functionDeclarations`, `toolConfig.functionCallingConfig` `ANY` + `allowedFunctionNames`, `functionResponse` parts), Google Search grounding for `web_search`, inline image for vision, `serviceTier` |
| `openai` | `{openaiBaseUrl}/chat/completions` | `tools` + `tool_choice: "required"`, `tool_calls` with JSON-string `arguments`, `tool` role messages, `image_url` data URI for vision |

Keys are `secret` config fields, sent only in a header (`x-goog-api-key`, `Authorization`), and masked out of every error, every tool result and every audit record. The administration page has a "Test the model" button (a tiny real call) and a "List models" button (`GET /v1beta/models`, `GET {base}/models`): model names are config, never trusted from memory.

`net.connect`: `llm` and `llm-search` go to `generativelanguage.googleapis.com:443`. `llm-compatible` is declared as `public` and `lan`, because the manifest's `setting:<key>` destinations name keys of the core's settings, not of a module's own config, so an operator-chosen base URL cannot be named narrower today.

## Tools

Each tool is a (name, JSON schema, availability predicate, handler) entry, with the source's names, descriptions and parameters. A tool whose backend is not there is never declared: `web_search` needs the Gemini provider, `look_at_live` needs vision enabled, and every tool that rests on a method of a peer (`snapshot`, `probe`, `benchmark`, `history`, `switchProvider`, `goLive`, `fineSync`, `clientLogs`) disappears for ten minutes once that peer answered 404 or 405 to it, so an older live or party module degrades to the first port's set instead of failing.

| Tool | Does |
| --- | --- |
| `reply`, `show_card`, `ignore` | the written answer (a reply next to investigation tools is a waiting message), a message with a card (`table`, `chart`, `match`, validated against the closed catalogue), nothing |
| `get_status`, `list_channels`, `run_ops` | the live status, the catalogue of one or both providers (with each channel's provider and its multiplex `evenement`), a diagnosis with a verdict (`diagnose`, `sante_flux`, `fournisseurs`) and `restart_ingest` |
| `search_guide`, `find_program` | the programme guide; find a programme and put it to the vote (live, upcoming, ambiguous, partial, foreign-only, web hints) |
| `change_channel` | a channel request through the room, by name (in one provider's catalogue), by reference (`p:3`, `s:4`, `#S.N`), with the optional `force` |
| `switch_provider` | the same programme at the other provider, through the live `switchProvider` point (rule and vote are code), or a precise channel of the other provider by reference |
| `probe_channel`, `benchmark_channels`, `stream_history` | a technical probe of one channel, a scan of up to eight channels read by the model and judged by code, the stream history as a summary and a ready `chart` card |
| `start_poll`, `close_poll`, `record_poll_vote`, `kick_from_party` | polls and kick through the room |
| `party_go_live`, `party_fine_sync`, `get_client_logs` | the party's two sync actions and the viewers' client journal |
| `learn_lesson`, `save_suggestion`, `note_person` | lessons (FIFO 30), suggestions (FIFO 120), per-person notes keyed by user id |
| `web_search`, `look_at_live` | a grounded web search (10 min cache, dated questions never served from it), a capture of the live picture read by the model |

What each new tool returns to the model (French keys, as in the source):

- `look_at_live`: `chaine`, `fournisseur`, `gratuit`, `figee`, `type`, `equipes`, `score`, `horloge`, `competition`, `logo`, `description`, `reponse`, `confiance`, `fiable`, `lu_il_y_a_s`. A refusal of `snapshot` comes back as `{ok:false, etat, note, raison}` with `raison` the live code (`standby` → `veille`, `stopped` → `arrete`, `settling`, `changed`, `captureFailed`, `tooLarge` → `indisponible`). A channel that changed while the model read the picture gives no reading. Refused with fewer than 60 s left in the turn.
- `run_ops`: `diagnose` adds the recent history (restarts over ten minutes, outage episodes over thirty) and, when `pseudo` is a present viewer, that viewer's freezes from the party's client logs (`cote_spectateur`); `restart_ingest` goes through `restart` in the author's name and returns `ok` or `{raison, message}` (`aucune_chaine`, `veille`, `flux_sain`, `relance_recente`, `chaine_changee`). It is the one `run_ops` action that counts as a refusable action, so a reply written before a refused restart is not posted.
- `switch_provider`: `switched_provider` (the model says it in one sentence), `vote_open` for `voteRequired` (announced by the party, turn over), or `{ok:false, status}` with `no_backup`, `no_channel`, `already_on`, `primary_down`, `not_found`, `dead`, `occupe`, `meme_fournisseur`, `autre_match`, `ref_inconnue`. On a multiplex (`evenement` on the current channel) `not_found`, `dead` and `vote_open` carry `multiplex: true` and a `suite_possible` that names the scan to run. With `ref`, the channel goes through the room's `requestSwitch` (vote or instant, the room decides) unless a recent scan compared to this very live proved it shows another match (`autre_match`); a scan that proved the same match is named in the vote reason.
- `probe_channel`: `{ref, chaine, fournisseur, vivante, codec, resolution, fps, codec_audio, debit_kbps, mbps_mesure, hors_antenne}`, `etat: occupe` (test connections taken, explicitly not a dead channel), `refus` (the live stream holds the backup connection), `veille`, `indisponible`, `ne_repond_pas`.
- `benchmark_channels`: see below. `stream_history`: `{resume, carte, usage}` or `{erreur}`, the card passes `show_card` unchanged.
- `party_go_live`, `party_fine_sync`: `{ok, membres?, note}` or `{ok:false, raison}` (`pas_de_watch_party`, `deux_membres_requis`). `get_client_logs`: `{minutes, total, par_spectateur, evenements}`.

### Scans

`benchmark_channels` starts the live `benchmark` point on up to eight channels of one provider (the live module caps a scan at eight, the source's twenty-four is not reachable), then the turn waits for the scan **across ticks** (one `scanResult` poll every third tick, the single-instance component is never held) and reads the frames one per tick. The first frame of each channel that answered is fetched with the live `scanFrame` point and read by the model; the reference picture (with `comparer_au_direct`) is a fresh capture of the live output, falling back on the last reading of this channel with teams read in the last ten minutes. The verdict (`oui`, `probable`, `inconnu`, `non`) is `matching.rs`, a port of the source's deterministic rules (team aliases and tags, Paris FC against PSG, clock and score tolerances), and a channel is grouped with the others that show the same match; `#S.N` is group N of scan S and is remembered for thirty minutes (five scans), resolved by `change_channel`, `switch_provider` and `probe_channel`. Rules kept from the source: two scans per message at most, a scan only with at least 50 s left in the turn (its budget is the lesser of 120 s and the time left minus 20 s), one announcement in the chat unless the model already posted a waiting message, the scan never touches the broadcast. Without `scanFrame` on the live module the scan still returns the state of each capture (black, no answer, portal refusal) and says there is no verdict.

### The `tools` point

This module defines `tv.kroma.hermes/tools`. A module that answers it contributes tools; the module calls every contributor when it builds a prompt (cached for 30 s) and routes each call back to the one that declared the tool. A name that a built-in tool already has is ignored.

`list` `{}` →

```json
{ "tools": [{ "name": "match_facts", "description": "…", "parameters": { "type": "object", "properties": {} } }] }
```

`call` `{ "name", "arguments": {}, "context": { "userId", "messageId" } }` → `{ "result": <any JSON> }`

A contributed tool is treated as an investigation tool (read-only). Its result is cut at 12 000 characters and masked like any other.

## Peers

Consumes `tv.kroma.party/room` (`say`, `history`, `requestSwitch` with `channelName` and `force`, `startPoll`, `closePoll`, `votePoll`, `kick`, `activity`, and the later `goLive`, `fineSync`, `clientLogs`) and `tv.kroma.live/control` (`status`, `resolve` with its `provider` filter, `provider` and `event` on each channel, `epg`, and the later `snapshot`, `restart`, `switchProvider`, `probe`, `benchmark`, `scanResult`, `history`, plus `scanFrame` which the live module does not offer yet). It never calls `switch`: a channel change is a vote, forced only if the room allows it. Both ends are tolerant structs with their JSON pinned by tests; a method a peer does not answer is reported to the model and its tool is hidden for ten minutes.

Events taken: `party.message`, `party.vote`, `party.poll` (what the gate needs), `live.status` (off-air alternatives posted by code, without the model), `live.failover` (audit only). The scan is followed by polling `scanResult`, not by the `live.scan` events, because a turn is advanced by the agent's own tick.

## The source's 28 tools

| Source tool | Here | Note |
| --- | --- | --- |
| `change_channel` | ported | `fournisseur`, `force`, `#S.N` |
| `switch_provider` | ported | `target` through `switchProvider`; `ref` through the room's `requestSwitch`; no admin shortcut (the live point has no `force`) |
| `find_program`, `search_guide`, `list_channels`, `get_status` | ported | |
| `reply`, `show_card`, `ignore`, `learn_lesson`, `save_suggestion`, `web_search` | ported | |
| `start_poll`, `close_poll`, `record_poll_vote`, `kick_from_party` | ported | |
| `look_at_live` | ported | the real `snapshot` |
| `probe_channel` | ported | `probe` |
| `benchmark_channels` | ported | `benchmark` + `scanResult` + `scanFrame` (the last one is not offered yet) |
| `stream_history` | ported | `history`, with the chart card |
| `run_ops` | ported, partly | `diagnose` (no network diagnosis: no sockets, no portal address), `fournisseurs`, `sante_flux`, `restart_ingest`; `watchparty` is not (no point method for the party's sync journal) |
| `party_go_live`, `party_fine_sync`, `get_client_logs` | ported | party `goLive`, `fineSync`, `clientLogs` |
| `match_facts`, `compos_match`, `live_standings`, `start_prono` | not here | the sport module contributes them through `tv.kroma.hermes/tools` |

`note_person` is this port's own addition. Not ported from the source's provider switching: the automatic image search for the equivalent of a multiplex channel before a switch (`_event_scan`, `_event_equivalent`, the stop-on-first-yes scan): the live module opens the vote itself (`unverifiedMultiplex`) before the agent can run a scan, so the agent only refuses a channel a scan already proved wrong and names the scan to run.

## State

Its own SQLite file: the inbox (queue and dedupe), lessons, suggestions, notes, the audit log, and a key-value table that holds the house rules once an administrator edits them. The shipped rules (`resources/house_rules.md`) are the default and the 17 lessons of the source are seeded once. The rules are reloaded at every message.

The audit log keeps records by genre (`chat`, `decisions`, `sources`, `llm`, `vision`, `audit`) for `auditDays` days (30 by default); a line is cut at 64 KB and a genre stops writing for the day at 40 MB. The daily job `hermes.maintenance` (category `maintenance`) reconciles and prunes.

## Administration

Admin → System → Hermès: status and pause, test and list the model, edit or reset the house rules, lessons, suggestions and notes per person (each removable), and the audit log by genre. Provider, model, keys and limits are the module's configuration form.
