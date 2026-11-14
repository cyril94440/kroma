# Torrent downloads

Torrent engines behind one DownloadClient trait: the rqbit helper this module starts, and one module per external engine: Transmission, qBittorrent, AllDebrid. An engine is picked by the `kind` of the client row, and a kind is valid when an enabled module answers the `tv.kroma.torrents/client` point under it; an engine that needs no URL (AllDebrid takes an API key) leaves it empty.

## Releases a client already holds

An engine may also answer the optional `instant` method of `tv.kroma.torrents/client`. The call
is `{ "client", "args": { "candidates": [{ "key", "magnet_or_url" }], "budget_ms" } }`: ranked
candidates, at most eight, to be answered within `budget_ms`. The reply is
`{ "peerless": bool, "hit": { "key", "client_ref" } | null }`: the first candidate the engine can
hand over at once (leaving nothing else behind), and the engine's statement about peer traffic.
An empty candidate list asks only whether it can, and what it says about peer traffic; an engine
without the route (rqbit, qBittorrent, Transmission) answers 404 and is read as unable. The bare
`{ "key", "client_ref" }` or `null` an older engine answers is still read, and never counts as
peerless.

`"peerless": true` is the engine's declaration that it moves no peer traffic through this
machine. Only an engine that says so is exempt from the VPN kill switch; without the statement the
gate applies and, while it is closed, the engine is not asked for any candidate.

`grab/grab-instant` takes `{ "releases": [{ "key", "spec" }], "budget_ms" }` (each `spec` a grab
spec; `budget_ms` defaults to 20 s and bounds the engine call and the wait for the release to be
ready). On a hit it inserts the ledger row tied to the engine's reference and completes it
through the same path the monitor takes, so nothing is added twice. It answers
`{ "status": "grabbed" | "nothing" | "unsupported" | "failed", "key", "download", "reason",
"deterministic" }`. `failed` means the engine held the release but could not settle it (for
example a ready magnet with no video file): its row is set to `removed` (never `failed`, so the
request's give-up rule does not count it), the wanted rows it took are reset, the engine is asked
to discard it, and `deterministic` says whether the release itself is at fault (the engine reported
an error) rather than a timeout. `torrentMaxActive` does not apply, because the row never waits
for a slot. `grab/instant-capable` answers whether the preferred client can do it.
`grab/drop-data` takes `{ "id", "discard" }`; `discard` asks the engine to remove the release
as well as its data.

A download whose engine has not answered a status poll for fifteen minutes is set to `failed`
with the last error, instead of showing as downloading forever.

A client created with an empty URL or an empty secret is put to its engine's own `test` before it
is saved, and refused with the engine's reason when that fails: only the engine knows which of its
fields it requires.

The ledger's byte counters are lifetime totals, so `server/src/bandwidth/` samples them on the monitor's tick and stores one window a minute, folded coarser as rows age. Each window keeps three shares apart: what the embedded engine moved while the VPN seal held, what it moved while a configured bridge was not sealed, and what an external daemon moved, which no bridge ever carries. That split is what makes the VPN page's throughput figure a claim rather than a guess.

Layout: `server/` (Rust backend), `ui/` (frontend), `locales/` (i18n), `module.json` (manifest). See `modules/README.md` for the module authoring guide.
