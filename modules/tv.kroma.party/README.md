# Watch party

The shared room around the live television: chat, majority votes to change channel, polls, reactions, an ideas box, viewer stats and playback sync for a small group watching the same stream.

It depends on `tv.kroma.live`, which owns the stream, the channel catalogue and presence. This module reads them through the `tv.kroma.live/control` point and never keeps a heartbeat of its own.

## How a client talks to it

A module has no socket, so every client message is an HTTP call and every push is an event on the core bus.

- Client to module: `POST /api/module/tv.kroma.party/<route>` with the session bearer. The caller's KROMA account is the identity; the `playback` permission lets a person take part and `settings.manage` moderates.
- Module to clients: events published on the bus (`party.*`), received over `/api/events`. Every broadcast event carries a `seq` that grows by one. A client that sees a gap, reconnects, or joins, fetches `GET state` and drops any event whose `seq` the state already holds.

| Source message | Route or event |
| --- | --- |
| `chat` | `POST chat` → `party.message` |
| history on join | `GET state`, `GET chat/history?limit&before` |
| `proposal_vote`, typed oui/non | `POST vote`, or a typed answer while a vote is open |
| `request_channel` | `POST request-switch {channelId, channelName?, force?}`; `GET channels?q=` to search |
| `proposal` | `party.vote` |
| `poll_vote`, `poll`, `poll_closed` | `POST poll/vote`, `party.poll` |
| `reagir`, `reactions_msg` | `POST react`, `party.reactions` |
| `reaction` (floating) | `POST reaction`, `party.reaction` |
| `idee`, `idees_req` | `POST ideas`, `GET ideas`, `party.ideas`; admin `GET ideas/triage`, `POST ideas/{id}` |
| `sync`, `sync_state` | `POST sync/beat`, `party.sync` |
| `sync_correction` | `POST sync/recalibration` |
| `finesync_done` | `POST sync/fine-done` |
| `sync_golive`, `sync_finesync`, `sync_kicked` | `party.sync.golive`, `party.sync.finesync`, `party.sync.kicked` (addressed) |
| `clocksync` | `GET time` |
| `stats` | `POST stats` |
| `clientlog` | `POST clientlog`, admin `GET clientlog` |
| `hermes_activity` | `party.activity` |

## The vote

Deterministic, never decided by the assistant. 30 seconds (`voteSeconds`), for against against; a blank counts for nothing; the request counts as a vote for; a tie or a minority stays. One person is one voice (the KROMA account). Only viewers present in the live module's presence list take part; someone who arrives after the vote opens may vote. The vote closes as soon as the outcome is certain. A voter who is away longer than `presenceGraceSeconds` leaves the quorum, and the vote is cancelled when the requester and all voters are gone. A request is refused while another vote is open, when the channel is already on, or when presence cannot be read. Administrators and the ids in `bypassUserIds` may switch without a vote.

A provider proposal raised by the live module (`live.proposal`) is voted the same way and answered with the control point's `proposal` method.

## Points

Consumes `tv.kroma.live/control` (`status`, `presence`, `switch`, `proposal`, `resolve`).

Defines `tv.kroma.party/room`, for the assistant module that comes later. Every method is a `POST /_port/tv.kroma.party/room/<method>` answering `{"Ok": ...}` or `{"Err": "..."}`, in camelCase:

| Method | Body | Reply |
| --- | --- | --- |
| `say` | `{text, as: "agent"\|"system", card, replyTo}` | `{id}` |
| `history` | `{limit, before}` | `{messages: [{id, userId, name, kind, text, at}]}` |
| `requestSwitch` | `{channelId, requestedBy, reason}` | `{outcome: "voteStarted"\|"switchedDirectly"\|"refused", voteId, reason}` |
| `startPoll` | `{question, kind, options, durationS}` | `{poll}` |
| `closePoll` | `{}` | `{results}` |
| `kick` | `{userId, requestedBy, reason}` | `{outcome: "kicked"\|"refused", reason, disruptions}` |
| `activity` | `{status}` | `{ok}` |
| `votePoll` (added) | `{userId, value}` | `{ok}` |

`requestedBy` is the user id on whose behalf the request is made; any id that is not a present viewer (for example `agent`) is a proposal without a voice of its own. Every message published to the room, user or assistant, is also a `party.message` event; a user message carries `mentionsAgent`, `isQuestion` and `keywordHit`, the lexical signals only. The decision to answer belongs to the assistant.

## Storage

Its own SQLite file: chat, vote and poll history, ideas, client logs. The chat archive job (`party.archive`, daily, category `maintenance`) removes messages older than `archiveDays`.

## Frontend

A page "Party" in the main sidebar, for phones and anywhere there is no player, and a panel the live page shows beside its player (`tv.kroma.live/player.aside`) with the vote banner and floating reactions over the picture in full screen (`tv.kroma.live/player.overlay`). Both reach the module through `useViewerApi` and `useServerEvents` from the SDK; nothing reads a token.

The aside also carries the playback sync client. It drives the live page's player handle (`getPositionMs`, `seekByMs`, `getMoveRange`, `isReady`, `getBufferAheadMs`, `seekToLiveEdge`, `setPlaybackRate`) and speaks `POST sync/beat`, `sync/fine-done` and `sync/recalibration`. The rules are those of the source product: the latency of the frame on screen against the server clock (`GET time`, best of eight round trips), a beat every 2 s, a recalibration only when both the smoothed and the fresh gap exceed 2 s (one clean seek, then silence, a wait that doubles when they come back within a minute), a grace after a restart or a resume, a blocked flag when the window of the player forbids the seek, a snap to the group target on joining, and the fine campaign (five samples, median, a seek when beyond 15 ms, a learned landing bias, two confirmations, 45 s budget). The playback rate is never changed while following a group.
