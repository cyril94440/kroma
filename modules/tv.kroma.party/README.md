# Watch party

The shared room around the live television: chat, majority votes to change channel, polls, reactions, an ideas box, viewer stats, playback sync and a voice chat for a small group watching the same stream.

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
| `welcome`, `peers` | `POST voice/join` answer, `GET voice/state`, `party.voice.roster` |
| `rtc` | `POST voice/signal`, `party.voice.signal` (addressed) |
| `speaking` | `POST voice/speaking`, `party.voice.speaking` |
| `mic` | `POST voice/mute`, the `party.voice.joined` and `party.voice.left` chat lines |
| `turn` | the `iceServers` of the join answer and `GET voice/ice` |

## The vote

Deterministic, never decided by the assistant. 30 seconds (`voteSeconds`), for against against; a blank counts for nothing; the request counts as a vote for; a tie or a minority stays. One person is one voice (the KROMA account). Only viewers present in the live module's presence list take part; someone who arrives after the vote opens may vote. The vote closes as soon as the outcome is certain. A voter who is away longer than `presenceGraceSeconds` leaves the quorum, and the vote is cancelled when the requester and all voters are gone. A request is refused while another vote is open, when the channel is already on, or when presence cannot be read. Administrators and the ids in `bypassUserIds` may switch without a vote.

A provider proposal raised by the live module (`live.proposal`, kinds `switchToBackup`, `returnToPrimary` and `switchProvider`) is voted the same way and answered with the control point's `proposal` method.

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
| `goLive` | `{requestedBy}` | `{members}`, or `Err("noActiveParty")` |
| `fineSync` | `{requestedBy}` | `{ok}`, or `Err("needTwoMembers")` |
| `clientLogs` | `{userId: string\|null, limit, minutes?}` | `{events: [{at, userId, name, device, category, event, data}]}`, the entries `GET clientlog` returns |

`requestSwitch` also takes an optional `channelName`, used to name the channel in the vote announcement when the live module cannot resolve the id, and an optional `force`, honoured only when `requestedBy` is listed in `bypassUserIds` (an administrator must be listed to be forced on behalf of); otherwise the request goes to a vote. `requestedBy` is the user id on whose behalf the request is made; any id that is not a present viewer (for example `agent`) is a proposal without a voice of its own. Every message published to the room, user or assistant, is also a `party.message` event; a user message carries `mentionsAgent`, `isQuestion` and `keywordHit`, the lexical signals only. The decision to answer belongs to the assistant.

## Voice chat

A mesh of browser to browser audio connections, as in the source product: every person who joined talks to every other one directly, the module only introduces them and relays the handshake. It is sized for a family (`voiceMaxPeers`, 8 by default, because every extra person adds a connection for everyone).

**Identity.** The person is the KROMA account, the participant is a *peer*: the account plus a `deviceId` the client generates (random, kept in the tab's session storage, 8 to 64 characters of `A-Za-z0-9_-`). Two tabs or devices of one account are two peers; a tab that reloads sends the same `deviceId` and replaces its stale entry. `join` always mints a new `peerId`, so a peer id names one stay in the room. The two peers of one account are never linked to each other, they would only feed back.

| Route | Body | Answer |
| --- | --- | --- |
| `GET voice/state` | | `{revision, now, participants: [{peerId, userId, name, muted}], speaking: [peerId], max}` |
| `POST voice/join` | `{deviceId, muted, watching}` | `{peerId, revision, participants, speaking, iceServers, beatSeconds, expireSeconds}`; `409 full`, `429 rateLimited`, `400 badDevice` |
| `POST voice/leave` | `{peerId, quiet?}` | `{ok}`; `quiet` (sent when a page unloads) skips the chat line, and a return within 3 minutes is not announced either |
| `POST voice/beat` | `{peerId}` | `{ok, revision}`, or `404 notJoined` when the module forgot the peer |
| `POST voice/mute` | `{peerId, muted}` | `{ok}` |
| `POST voice/speaking` | `{peerId, speaking}` | `{ok}`; at most 10 `true` a second per peer |
| `POST voice/signal` | `{from, to, session, kind, description?, candidate?}` | `{ok, n}`; `404 peerGone`, `404 notJoined`, `403 forbidden`, `413 tooLarge`, `429 rateLimited` |
| `GET voice/ice` | | `{iceServers}`, for participants only |

`kind` is `description` (`{type: "offer" \| "answer", sdp}`, 16 KiB at most), `candidate` (`{candidate, sdpMid, sdpMLineIndex, usernameFragment}`, 2 KiB at most) or `hello`. A peer may only send as itself; 240 signals per 10 s per peer.

| Event | Audience | Payload |
| --- | --- | --- |
| `party.voice.roster` | everyone | `{revision, participants}`, the whole roster |
| `party.voice.speaking` | everyone | `{speaking: [peerId], at}`, the whole set |
| `party.voice.signal` | the account that owns `to`, nobody else | `{to, from, fromUserId, session, n, kind, description?, candidate?}` |

None of them carries the party `seq` and none is part of `GET state`; a client reads `GET voice/state` for the room. An addressed event reaches every device of the target account, so a client keeps only the ones whose `to` is its own `peerId`.

**Surviving a lost event.** Events are best effort, so nothing depends on one arriving. The roster and the speaking set are whole state, the roster with a `revision` (it starts from the clock, so it never goes back across a restart): a lost one is repaired by the next, by the revision every heartbeat answers, or by a read. A speaking flag lapses 5 s after its last report and the speaker repeats it every 2 s, so a lost "stopped" clears itself. The module keeps a counter `n` per ordered pair of peers, and the receiving side reads a gap in it as a lost message. Peers heartbeat every 10 s and are dropped after 30 s of silence; a heartbeat for an unknown peer answers `notJoined` and the client joins again under a new peer id.

**Negotiation.** The perfect negotiation pattern, with the polite side decided by the peer ids (the larger id is polite). The impolite side owns the *session*, an id it mints each time it builds a connection; the polite side builds its own connection when it first sees an offer under a new session. Every message carries the session, so late messages of a replaced connection are dropped. Signals of one connection are sent one at a time, in order, because two requests in flight can reach the module in either order. Recovery is the same move in every case: an unanswered offer is sent again twice, a connection that has not connected after 12 s is rebuilt under a new session (the polite side asks with `hello`), a failed or long-disconnected connection restarts ICE twice before it is rebuilt, a gap in `n` triggers a rebuild if the connection is not up 3 s later. A peer that leaves closes its connections everywhere; a `peerGone` answer does too.

**Audio.** One send-and-receive transceiver per connection from the start, so muting is `track.enabled = false` and a microphone granted later is a `replaceTrack`, never a renegotiation. Echo cancellation and noise suppression are on, as in the source. "Speaking" is measured on the local microphone (RMS 0.045, 80 ms attack, 550 ms release) and reported, as in the source. Joining asks for the microphone first; if it is refused the person joins muted and listens. There is no push to talk and no ducking of the programme audio, because the source has neither.

**Where it needs HTTPS.** `getUserMedia` and `RTCPeerConnection` need a secure context (HTTPS or localhost); outside one the panel says so instead of failing.

**TURN and STUN.** The `iceServers` come from the join answer and `GET voice/ice`, both authenticated, are never broadcast and never logged. `voiceStunUrls` (default Google's public STUN, as in the source) and an optional static relay (`voiceTurnUrls`, `voiceTurnUsername`, `voiceTurnCredential`, a secret). With `voiceCloudflareKeyId` and `voiceCloudflareApiToken` (a secret) the module mints time-limited Cloudflare TURN credentials the way the source does (24 h, refreshed after 12 h, a failure falls back to the static servers or STUN alone and is retried after a minute); that is the one reason the manifest declares `net.connect` to `rtc.live.cloudflare.com:443` for the purpose `turn`. The media itself never goes through the module.

**Who may join.** Any signed-in account with the `playback` permission. A peer that joined from the live page (`watching`) is dropped when its account leaves the live module's presence list for longer than `presenceGraceSeconds`; one that joined from the Party page is held by its heartbeat alone.

**Frontend.** `PartyAside` (the live page) and `PartyPage` carry the voice panel: join and leave, mute, silence the others, who is in the room and who is speaking. In full screen the overlay names who is speaking. The voice lasts as long as the panel that started it is on screen.

## Storage

Its own SQLite file: chat, vote and poll history, ideas, client logs. The chat archive job (`party.archive`, daily, category `maintenance`) removes messages older than `archiveDays`.

## Frontend

A page "Party" in the main sidebar, for phones and anywhere there is no player, and a panel the live page shows beside its player (`tv.kroma.live/player.aside`) with the vote banner and floating reactions over the picture in full screen (`tv.kroma.live/player.overlay`). Both reach the module through `useViewerApi` and `useServerEvents` from the SDK; nothing reads a token.

The aside also carries the playback sync client. It drives the live page's player handle (`getPositionMs`, `seekByMs`, `getMoveRange`, `isReady`, `getBufferAheadMs`, `seekToLiveEdge`, `setPlaybackRate`) and speaks `POST sync/beat`, `sync/fine-done` and `sync/recalibration`. The rules are those of the source product: the latency of the frame on screen against the server clock (`GET time`, best of eight round trips), a beat every 2 s, a recalibration only when both the smoothed and the fresh gap exceed 2 s (one clean seek, then silence, a wait that doubles when they come back within a minute), a grace after a restart or a resume, a blocked flag when the window of the player forbids the seek, a snap to the group target on joining, and the fine campaign (five samples, median, a seek when beyond 15 ms, a learned landing bias, two confirmations, 45 s budget). The playback rate is never changed while following a group.
