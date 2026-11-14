# AllDebrid engine

AllDebrid as a download engine for the Downloads module, and a source the
library streams from.

A backend-only module: no page, no routes of its own. It contributes to the
`tv.kroma.torrents/client` point under the instance `alldebrid` (the Downloads
module lists it in its add-a-client dialog) and to the core's `source` point,
which is how the server finds a file's bytes again. The API key is the client's
password; nothing else is asked.

## How a download works

AllDebrid fetches the torrent on its own servers, so nothing is downloaded here.
When AllDebrid reports a magnet ready, this module writes under
`<data>/alldebrid/downloads/<magnet id>/`:

- one small `.kremote` stub per video file (the JSON of
  `kroma_module_sdk::primitives::remote_file::RemoteFile`), and
- the subtitle files beside them (`srt`, `ass`, `ssa`, `vtt`), up to 5 MiB each.

Only then does `status` report `completed`, and acquisition imports the stubs
like the videos they stand for. A magnet that is ready stays on AllDebrid after
the import, because the library streams from it.

## Instant

`POST /_port/tv.kroma.torrents/client/instant` takes `{ "candidates": [{ "key", "magnet_or_url" }] }`.
At most eight are used: the magnets go up in one `magnet/upload` call, each `.torrent` link in a
call of its own, and the first candidate in rank order that AllDebrid reports `ready` is kept and
answered as `{ "key", "client_ref" }`. AllDebrid documents no flag for an upload that was already in
the account, so the account's magnet ids are listed first (one `magnet/status` call with no `id`) and
only a magnet whose id was not there, so one this call created, may be deleted again. Magnets the
user added by hand, or that this module tracks, are never deleted, chosen or not. If the account
cannot be listed nothing is deleted and the ids left behind are logged. Deletes are one call each
(`magnet/delete` takes one id), spaced to stay under the documented 12 requests a second; a delete
that fails is logged with the magnet id and never fails the call. An empty list answers `null`
without calling AllDebrid.

## Streaming and the cache

`POST /_port/source/locate` takes `{ "ref", "warm" }` and answers a local path
when the file is in the cache, otherwise a freshly unlocked URL. `warm` also
starts a background fetch of that one file into `<data>/alldebrid/cache/`, one
ranged request at a time, so a call to the module is never blocked behind a
transfer. An hourly job (`alldebrid.cache`) deletes cached files unused for
`cacheMaxDays` days and evicts the least recently used while the cache is over
`cacheMaxGb` GB. A file being fetched or used in the last ten minutes is never
chosen.

## What it keeps

The API key of each magnet's client, and the latest key seen, in
`<data>/alldebrid/credentials/` (readable by the server's user only), because
the core asks for a file without a client in hand. A rotated key keeps older
magnets working.

Pause and resume are refused (AllDebrid has no such thing), and re-announcing is
a no-op.
