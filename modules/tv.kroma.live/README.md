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
- **One ffmpeg.** Video is copied, audio is re-encoded to AAC stereo 160 kb/s,
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
  history and the duration of every zap.
- **Failover.** When the main provider delivers nothing for 45 s in front of
  viewers the same channel is started at the backup; stuttering first triggers a
  background trial of the other provider; the return to the main provider is
  proposed when the programme ends. With the Watch party module installed a
  proposal is voted there (`live.proposal`, answered through the
  `tv.kroma.live/control` `proposal` method). Without it a proposal is accepted
  when at most one viewer is present and declined otherwise.

## Settings

All of it is in the module's Admin page: the two provider URLs (secret), their
user agents, an optional guide URL, the standby delay, the failover thresholds,
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
| `POST /pause` `POST /resume` `POST /scan` `GET /scan` `POST /trial` `GET /history` `GET /metrics` `GET /overview` `POST /refresh` | settings.manage | administration |

The playlist to hand to a player is `status.playlist`, relative to the module's
files: `useModuleFileUrl()` (the SDK) builds the URL
with the media ticket.

## Events (on `/api/events`)

`live.status` (the full status), `live.channel` (`channel`, `provider`,
`onBackup`, `now`, `next`, `playlist`), `live.presence` (`viewers`), `live.health`
(`sourceKbps`, `stutter`, `black`, `frozen`, `trouble`, `state`, `lossSeconds`),
`live.proposal` (`proposal`, `resolved`, `accepted`, `default`), `live.failover`
(`kind`: `failover`, `failback`, `returnIdle`, `bothDown`, `recovered`,
`noEquivalent`, `primaryBack`, `trial`, `trialNotBetter`) and `live.scan`.

## The `control` point

`tv.kroma.live/control`, answered under `/_port/tv.kroma.live/control/<method>`:
`status`, `resolve`, `switch`, `presence`, `proposal`, `epg`. The JSON of each is
pinned by a test (`src/it_points.rs`).

## Slots the watch page opens

The page defines two slots (`ModuleSlot`, filled by any module through `defineModule({ slots })`), each given the same props:

| Slot | Where | Props |
| --- | --- | --- |
| `tv.kroma.live/player.aside` | beside the player on a wide screen, under it on a narrow one; no room is taken when nothing fills it | `player`, `channel`, `playing` |
| `tv.kroma.live/player.overlay` | over the picture, so it stays visible in the full screen overlay | `player`, `channel`, `playing`, `immersive` |

`channel` is `{ id, name } | null`, `playing` a boolean, and `player` a stable object of functions: `getPositionMs()` (wall-clock ms of the frame on screen from the playlist's programme date time, `null` when unknown), `isReady()`, `getBufferAheadMs()`, `getMoveRange()` (`{ backMs, forwardMs }` a seek may travel inside the safe zone of the window, `forwardMs` negative when already past it), `seekToLiveEdge()`, `seekByMs(deltaMs)` (negative goes back, no clamp), `setPlaybackRate(rate)` and `subscribe(listener)` (snapshots `{ positionMs, playing, channelId }` every 250 ms and on play, pause and seek). The filler owns its own types and validates what it receives.
