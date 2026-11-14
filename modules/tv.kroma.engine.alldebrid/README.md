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

The manifest declares `fs.read` and `fs.write` for `downloads` although the stubs are
written under the module's own data directory: the host holds
`<data>/alldebrid/downloads` for the import (and the core) only for a first-party
module that declares `downloads`, and mounts it as its own root, writable only when
the module also declares `fs.write: downloads`. Without them the importer cannot read
the stubs and the module cannot write them.

## Instant

`POST /_port/tv.kroma.torrents/client/instant` takes
`{ "candidates": [{ "key", "magnet_or_url" }], "budget_ms" }` and answers
`{ "peerless": true, "hit": { "key", "client_ref" } | null }`.

- At most eight candidates are used, and the whole call stays inside `budget_ms`
  (20 s when absent): every HTTP call is capped to the time left and nothing is
  started once it is spent.
- Magnets go up three at a time in rank order, and the walk stops at the first
  batch holding a `ready` one (fewer uncached uploads than sending all eight, at
  the price of up to three round trips). `.torrent` links are fetched and
  uploaded one by one, in rank order, only when no magnet was ready.
- `MAGNET_TOO_MANY_ACTIVE` and `MAGNET_TOO_MANY` stop the uploads at once.
- `peerless: true` is the module's statement that no peer traffic moves through
  this machine; the downloads module only skips its VPN kill switch for an
  engine that says so.
- Ownership: AllDebrid documents no flag for an upload that was already in the
  account, and does not promise `magnet/status` lists every magnet. So the
  account is listed first and only an id that is not in that listing and is
  higher than every id in it (ids only grow) counts as created by this module.
  Only such magnets are ever deleted, here or by `remove`. If the account cannot
  be listed, nothing is deleted. An empty list answers `hit: null` without
  calling AllDebrid.

## Removing

`remove` deletes on AllDebrid only a magnet this module created: one that has
not finished, or a finished one when the call carries `discard: true` (a
rejected release). A finished magnet stays otherwise, because the library
streams from it; a magnet the user added in AllDebrid is never touched.
`delete_data` removes the stubs saved here. A title removed from the library, or
a stub replaced by an upgrade, does not remove its magnet from the account:
nothing here can prove no other stub refers to it, so those magnets are left for
the user to clear in AllDebrid.

## Streaming and the cache

`POST /_port/source/locate` takes `{ "ref", "warm" }` and answers a local path
when the file is in the cache, otherwise a freshly unlocked URL (no account call
is made per locate; plans and unlocked links are memoised for minutes). `warm`
also starts a background fetch of that one file into `<data>/alldebrid/cache/`,
one ranged request at a time, so a call to the module is never blocked behind a
transfer. A fetch only starts when the file fits `cacheMaxGb` counting the
finished files, the partial files and the fetches running; finished files idle
for ten minutes or more are evicted least recently used first to make room,
otherwise the file is not cached and playback carries on by relay. At most two
fetches run at once, a file larger than the whole budget is never fetched, and
the purge policy also runs whenever a fetch finishes, besides the hourly job
(`alldebrid.cache`, which deletes files unused for `cacheMaxDays`). A component
cannot ask the host how much disk is free, so `cacheMaxGb` is the only bound:
set it below the space you can spare. Every ranged answer is checked against its
`Content-Range`, and a file is only named by its final path once its size matches
what AllDebrid reported; AllDebrid exposes no checksum, so size is the only
integrity check.

A 429 or 503 from AllDebrid (12 requests per second and 600 per minute are
documented) is "retry later": the module pauses its own calls for a few seconds,
doubling up to a minute, instead of failing every viewer's playback request.
Delayed links (`link/unlock` answering a `delayed` id) are refused with an
explicit error; the delayed-link flow is not implemented.

## Network permissions

The manifest allows `api.alldebrid.com` for the API, and `*.debrid.it`,
`*.alldeb.ovh` and `*.alldebrid.com` for the unlocked links. AllDebrid does not
document which hosts its links use, and a `*.alldebrid.com` entry does not match
the bare domain. When a fetch fails because a link's host is not allowed, the
module logs `AllDebrid returned a link on <host>, which module.json does not
allow under debrid-transfer: add it to net.connect` so an operator can see
which entry to add.

## What it keeps

The API key of each magnet's client, and the latest key seen, in
`<data>/alldebrid/credentials/`, because the core asks for a file without a
client in hand. A component cannot set or read a file mode, so the protection is
the folder: the host creates this module's data directory owner-only (0700)
before the module starts and tightens it again if it was ever opened to others,
which keeps other users of the host out. Backups pack the database rows, avatars
and subtitles only, never a module's data directory. A magnet's record is
deleted with the magnet, and the latest key with the last record. The key
travels in every request the core makes to the module, so it is also in the
downloads module's database.

Pause and resume are refused (AllDebrid has no such thing), and re-announcing is
a no-op.
