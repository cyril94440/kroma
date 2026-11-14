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

Each tool is a (name, JSON schema, availability predicate, handler) entry. A tool whose backend is not there is never declared: `web_search` needs the Gemini provider, `look_at_live` needs vision enabled and a live module that answers `snapshot`.

| Tool | Does |
| --- | --- |
| `reply`, `show_card`, `ignore` | the written answer (a reply next to investigation tools is a waiting message), a message with a card (`table`, `chart`, `match`, validated against the closed catalogue), nothing |
| `get_status`, `list_channels`, `run_ops` | the live status, the catalogue search, a diagnosis with a verdict |
| `search_guide`, `find_program` | the programme guide; find a programme and put it to the vote (live, upcoming, ambiguous, partial, foreign-only, web hints) |
| `change_channel` | a channel request through the room |
| `start_poll`, `close_poll`, `record_poll_vote`, `kick_from_party` | polls and kick through the room |
| `learn_lesson`, `save_suggestion`, `note_person` | lessons (FIFO 30), suggestions (FIFO 120), per-person notes keyed by user id |
| `web_search`, `look_at_live` | a grounded web search (10 min cache, dated questions never served from it), a capture read by the model |

### The `tools` point

This module defines `tv.kroma.hermes/tools`. A module that answers it contributes tools; the module calls every contributor when it builds a prompt (cached for 30 s) and routes each call back to the one that declared the tool. A name that a built-in tool already has is ignored.

`list` `{}` →

```json
{ "tools": [{ "name": "match_facts", "description": "…", "parameters": { "type": "object", "properties": {} } }] }
```

`call` `{ "name", "arguments": {}, "context": { "userId", "messageId" } }` → `{ "result": <any JSON> }`

A contributed tool is treated as an investigation tool (read-only). Its result is cut at 12 000 characters and masked like any other.

## Peers

Consumes `tv.kroma.party/room` (`say`, `history`, `requestSwitch`, `startPoll`, `closePoll`, `votePoll`, `kick`, `activity`) and `tv.kroma.live/control` (`status`, `resolve`, `epg`, and the optional `snapshot` → `{ "jpegBase64", "at" }`). It never calls `switch`: a channel change is a vote. Both ends are tolerant structs with their JSON pinned by tests.

Events taken: `party.message`, `party.vote`, `party.poll` (what the gate needs), `live.status` (off-air alternatives posted by code, without the model), `live.failover` (audit only).

## State

Its own SQLite file: the inbox (queue and dedupe), lessons, suggestions, notes, the audit log, and a key-value table that holds the house rules once an administrator edits them. The shipped rules (`resources/house_rules.md`) are the default and the 17 lessons of the source are seeded once. The rules are reloaded at every message.

The audit log keeps records by genre (`chat`, `decisions`, `sources`, `llm`, `vision`, `audit`) for `auditDays` days (30 by default); a line is cut at 64 KB and a genre stops writing for the day at 40 MB. The daily job `hermes.maintenance` (category `maintenance`) reconciles and prunes.

## Administration

Admin → System → Hermès: status and pause, test and list the model, edit or reset the house rules, lessons, suggestions and notes per person (each removable), and the audit log by genre. Provider, model, keys and limits are the module's configuration form.
