# Live TV

One IPTV provider connection, read by `ffmpeg` and restreamed as HLS to everyone
at home. The module keeps the stream off when nobody watches, refuses a channel
that serves a placeholder clip, notices a black or frozen picture, and moves to a
backup provider when the main one stops delivering in front of viewers.

## What it does

- **Providers and catalogue.** An Xtream portal (`player_api.php`: channels and
  categories) or an `.m3u` file, for a main and an optional backup provider. The
  catalogue refreshes hourly and is kept in the module's own database. Adult
  channels are never loaded, 4K channels are left out of automatic choices,
  French channels rank first and `RAW` / `VIP` variants rank before plain ones.
  Every one of those rules is a setting.
- **Guide.** The portal's XMLTV (gzip or plain, 2 h back to 14 h ahead, refreshed
  every three hours), searchable by programme title, with now and next on every
  channel.
- **One ffmpeg.** Video is copied (HEVC is tagged `hvc1`, which Apple players
  need in fMP4; the codec is learned from the zap probe or, failing that, from the
  local output, which costs one relaunch the first time), audio is re-encoded to AAC stereo 160 kb/s,
  and the output is a one second, 20 segment fMP4 HLS window with
  `EXT-X-PROGRAM-DATE-TIME`, written into
  `<data>/live/public/<random folder>/`. Each start writes a new folder and the
  old ones are deleted, so a stale segment is never served.
- **Standby.** Viewers send a heartbeat every 15 s (`POST presence`), a viewer
  counts for 60 s after the last one, and with nobody present for the standby
  delay (90 s by default) the upstream connection is closed. Someone coming back
  starts it again. A viewer present is never cut.
- **Health.** Stutter detection from ffmpeg's `-progress`, black and frozen
  picture detection on the local playlist, the source bitrate, a three hour
  history, the duration of every zap and the source's resolution (read from the
  local output, never from the provider). A channel whose usual bitrate is under
  `minKbps` (a radio, a test card) is judged against its own median, so it is not
  mistaken for a dead one; a high-rate channel that collapses still is.
- **Clip buffer.** The last ten minutes of the stream stay on disk (hard links to
  the live HLS segments, so no second ffmpeg and no second connection), per run:
  a zap or a restart never mixes two runs in one clip. The `control` point cuts a
  window of up to four minutes by stream copy into
  `<published>/clips/<random>/<name>.mp4`; cut clips are purged after 45 days or
  above 6 GiB, the buffer above 1 GiB. Clips outlive the module's restarts, the
  buffer does not.
- **Snapshot.** One JPEG of the current picture (at most 720 p, under 300 KB),
  grabbed from the local HLS output. Refused in standby, with the source down,
  or in the 12 s after a channel change; one at a time, and the same picture is
  served for 15 s on the same channel and run.
- **Failover.** When the main provider delivers nothing for 45 s in front of
  viewers, the same channel is looked up at the backup, probed (one short ffprobe)
  and started there; stuttering first triggers a background trial of the other
  provider; the return to the main provider is proposed when the programme ends.
  With the Watch party module installed a proposal is voted there
  (`live.proposal`, answered through the `tv.kroma.live/control` `proposal`
  method). Without it a proposal is accepted when at most one viewer is present
  and declined otherwise.
- **Canaries and variants.** `canaries` names up to two channels of the main
  provider that should always play. When the channel in trouble is silent and a
  canary plays, the provider is fine: the module does not fail over, it tries
  another variant of the same channel (`FR:`, `VIP: ... RAW`, `... HD`), at most
  two switches per ten minutes for the whole family and never back to a variant
  declared silent in that time, then says once that every version is cut.
  Canaries are never probed at the backup provider.
- **Backup coverage.** After every catalogue refresh the quick-access channels are
  matched against the backup. The Admin overview lists each one with its gap
  (`absentFromBackup`, `notFound`, `noGuide`), and a gap that appears is recorded
  as a `coverageGap` provider event and notified to administrators.
- **Trace.** Every change in what the failover policy sees (and a sample every
  30 s while something is wrong) is appended to `<data>/live/trace/` as one JSON
  line, rotated at 2 MB. `GET /trace` replays it through a fresh policy and counts
  where the replay disagrees with the recorded decision.

## Scans

A scan captures six seconds of each channel (key frames only, one frame a second,
black detected on at least 60 % of the sample) and keeps the first and the last
distinct frame as JPEGs under `scan-<id>/`, with `black` and `still` (a frozen
picture is one frame only). Captures run one at a time, and beside the live at
the backup provider only when `backupScanWithLive` allows it.

## Settings

All of it is in the module's Admin page: the two provider URLs (secret), their
user agents, an optional guide URL, the standby delay, the failover thresholds, the canary channels,
the house rules (family filter and blocked terms, 4K, country weights, variant
weights, quick-access channels) and the connection rules of the backup provider.

## The one connection rule

A provider contract usually allows a single connection. The module opens exactly
one per provider for the live stream, and for the backup provider it enforces it
in code (`backupMaxConnections`, 1 by default): no ffmpeg reconnect flags
(a reconnect opens the new connection before closing the old one), no probe, no
trial and no second capture beside the live stream. The one exception is a scan
capture, one at a time, beside the live stream (`backupScanWithLive`, on by
default).

## https

KROMA's pinned ffmpeg reads `file`, `pipe`, `http` and `tcp` only. A provider on
`http://` works on every install. For an `https://` provider the host needs an
ffmpeg with TLS on its `PATH`; the module checks `ffmpeg -protocols` and the
Admin page says so when it is missing, instead of failing obscurely.

## Network

`ffmpeg` is a program of the host: it dials the provider from the machine, not
through the egress gateway, so the manifest declares `net.host`. The module's own
calls (catalogue, guide) go out under the purpose `provider` to `public`
addresses.

## HTTP API (under `/api/module/tv.kroma.live`)

| Route | Who | Answers |
| --- | --- | --- |
| `GET /status` | playback | state, channel, provider, now and next, viewers, health, playlist path |
| `GET /channels?q&group&provider&page&perPage` | playback | a page of channels with what is on now, and the groups |
| `GET /favourites` | playback | the quick-access channels |
| `GET /epg/search?q` / `GET /epg/grid?channels&from&to` | playback | programmes, times in ms |
| `POST /presence` / `POST /presence/leave` | playback | heartbeat |
| `POST /switch` `{channelId or query}` | playback | refused with `useVote` when the Watch party module is installed and others are watching |
| `POST /pause` `POST /resume` `POST /scan` `GET /scan` `POST /trial` `GET /history` `GET /metrics` `GET /overview` `POST /refresh` `GET /trace` | settings.manage | administration |

The playlist to hand to a player is `status.playlist`, relative to the module's
files: `useModuleFileUrl()` (the SDK) builds the URL
with the media ticket.

## Events (on `/api/events`)

`live.status` (the full status), `live.channel` (`channel`, `provider`,
`onBackup`, `now`, `next`, `playlist`), `live.presence` (`viewers`), `live.health`
(`sourceKbps`, `stutter`, `black`, `frozen`, `trouble`, `state`, `lossSeconds`),
`live.proposal` (`{proposal: {id, kind, text, channel, createdAt, expiresAt},
resolved}`, plus `viewers` while it is open for a vote, or `accepted` and `default`
(`vote`, `noVote` or `timeout`) once it is resolved; `kind` is `switchToBackup`,
`returnToPrimary` or `switchProvider`), `live.failover` (`kind`: `failover`, `failback`, `returnIdle`,
`bothDown`, `recovered`, `noEquivalent`, `backupUnreachable`, `backupDead`,
`noWorkingEquivalent`, `failbackRefused`, `channelDown`, `providerSwitch`,
`primaryBack`, `trial`, `trialNotBetter`) and `live.scan` (`{id, item}` per
channel, then `{id, done: true}`). `live.channel` is also published when the
channel is restored at start, on resume, and again once its playlist is ready.

## Notifications

Declared in `module.json` and worded in `locales/`: `failover`, `failback`,
`bothDown`, `recovered` and `channelDown` go to everyone allowed to watch
(`push` off by default), `coverageGap` to administrators.

## The `control` point

`tv.kroma.live/control`, answered under `/_port/tv.kroma.live/control/<method>`.
Every answer is `{"Ok": ...}` or `{"Err": "<reason>"}`. The first six methods are
a fixed contract other modules code against; the others were added to them.

| Method | Call | Answer |
| --- | --- | --- |
| `status` | `{}` | `{state, channel, provider, now, next, viewers: [{userId, name}], health: {sourceKbps, stutter, black, frozen}, startedAt}`; `startedAt` is the ms epoch at which the current channel's stream started (`null` in standby), the same value as `GET /status` |
| `resolve` | `{query, limit?, provider?: "primary"\|"backup"\|null}` | `{channels: [{id, name, group, score, now, provider, event?}]}`; a reference such as `p:3` resolves to its channel |
| `switch` | `{channelId, requestedBy, reason}` | `{switched, channel, error}` |
| `presence` | `{}` | `{viewers: [{userId, name, since}]}` |
| `proposal` | `{id, accept}` | `{applied}` |
| `epg` | `{query?, from?, to?, limit?}` | `{programmes}` |
| `snapshot` | `{}` | `{jpegBase64, at}`; `Err`: `standby`, `stopped`, `settling`, `changed`, `captureFailed`, `tooLarge` |
| `clipBufferInfo` | `{}` | `{fromMs, toMs}`, both `null` when the buffer is empty |
| `clipCut` | `{fromMs, toMs, name}` | `{path, durationMs}`; `path` is under the published folder; `Err`: `invalidName`, `invalidWindow`, `outOfBuffer`, `cutFailed` |
| `restart` | `{requestedBy, reason}` | `{restarted, error}`; `error`: `noChannel`, `standby`, `streamHealthy`, `recentRestart`, `channelChanged` |
| `switchProvider` | `{provider, channelId?, requestedBy, reason}` | `{switched, provider, error}`; `channelId` (`p:3`, `s:12`) names the exact channel of `provider` to go to instead of the best equivalent of the current one; it is probed once, then follows the same rule (alone or in trouble: at once, otherwise a vote; the stream is in trouble when it has delivered nothing for `troubleDeadSeconds`, when it stutters or restarts while flowing, or as soon as ffmpeg has exited and nothing flows again). `error`: `unknownProvider`, `noBackup`, `noChannel`, `alreadyOn`, `primaryDown`, `notFound`, `notWorking`, `busy`, `voteRequired` (a `live.proposal` is published), and with `channelId` `unknownChannel` and `providerMismatch` |
| `probe` | `{channelId}` | `{channelId, name, provider, alive, resolution, codec, audioCodec, fps, bitrateKbps, offAir, local, error}`; `error`: `unknownChannel`, `busy`, `liveAtThisProvider`, `standby`, `notReady` |
| `benchmark` | `{channelIds, requestedBy}` | `{status: "started"\|"refused", id, reason, total, alreadyDone}`; `reason`: `unknownChannel`, `mixedProviders`, `noChannel`, `scanRunning` |
| `scanResult` | `{}` | the last scan: `{id, query, running, provider, items, total}`; each item is `{number, id, name, ok, black, still, frame, frames, error, now, at}`, `at` being the ms epoch at which its capture ended |
| `scanFrame` | `{path}` | `{jpegBase64}`; `path` is a `frame` or `frames` value of the last `scanResult`, nothing else; `Err`: `invalidPath`, `notFound`, `tooLarge` (over 1 MiB) |
| `clipList` | `{}` | `{paths}`, the `path` of every clip still on disk, as `clipCut` returned it |
| `clipDelete` | `{path}` | `{deleted}` (`false` when the clip was already gone), the clip's folder is removed with it; `Err`: `invalidPath` for anything but a `clipCut` path |
| `history` | `{limit?}` | `{stepSeconds, points, providerEvents}`, the last `limit` points (360 by default, 1080 at most) |

The JSON of every method is pinned by a test (`src/it_points.rs`, `src/it_ops.rs`).
