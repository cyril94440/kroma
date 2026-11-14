# Torrent downloads

Torrent engines behind one DownloadClient trait: the rqbit helper this module starts, and one module per external engine: Transmission, qBittorrent, AllDebrid. An engine is picked by the `kind` of the client row, and a kind is valid when an enabled module answers the `tv.kroma.torrents/client` point under it; an engine that needs no URL (AllDebrid takes an API key) leaves it empty.

## Releases a client already holds

An engine may also answer the optional `instant` method of `tv.kroma.torrents/client`: given
ranked candidates (`[{ "key", "magnet_or_url" }]`, at most eight) it keeps the first one it can
hand over at once and answers `{ "key", "client_ref" }`, or `null` when none is ready, leaving
nothing else behind. An empty list asks only whether it can; an engine without the route
(rqbit, qBittorrent, Transmission) answers 404 and is read as unable. An engine that answers
`instant` must move no data through this machine.

`grab/grab-instant` takes `{ "releases": [{ "key", "spec" }] }` (each `spec` a grab spec), asks the
preferred enabled client, and on a hit inserts the ledger row already tied to the engine's
reference and completed through the same path the monitor takes, so nothing is added twice.
It answers `{ "status": "grabbed" | "nothing" | "unsupported", "key", "download" }`. Neither the
kill switch nor `torrentMaxActive` applies to it: the switch guards the embedded engine's peer
traffic, which an instant engine has none of, and the row never waits for a slot.
`grab/instant-capable` answers whether the preferred client can do it.

The ledger's byte counters are lifetime totals, so `server/src/bandwidth/` samples them on the monitor's tick and stores one window a minute, folded coarser as rows age. Each window keeps three shares apart: what the embedded engine moved while the VPN seal held, what it moved while a configured bridge was not sealed, and what an external daemon moved, which no bridge ever carries. That split is what makes the VPN page's throughput figure a claim rather than a guess.

Layout: `server/` (Rust backend), `ui/` (frontend), `locales/` (i18n), `module.json` (manifest). See `modules/README.md` for the module authoring guide.
