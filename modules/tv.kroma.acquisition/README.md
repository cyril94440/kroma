# Acquisition

Interactive and manual release search, quality scoring, grab plus import, and the
automatic wanted-list pass.

The backend lives in this module's own `server/` crate (`kroma-acquisition`): the
`ServerModule`, the admin routes at `/acquisition/{search,analyze,add}`, the
`acquisition` point it answers at `/_port/acquisition/{search,search-now,grab,watch-now,watch-now-reject,watch-now-capable}`, and the
jobs behind the wanted list. It rides on the Downloads engine and the indexers,
consuming `tv.kroma.indexer/{search,db}` and `tv.kroma.torrents/{grab,db}`, and
`dependencies` the Downloads module (`tv.kroma.torrents`) because the grab has to
land somewhere.

`acquisition` is one of the three points the CORE calls, so it is bare rather
than namespaced, and nothing but the core may define one.

## Watch now

`/_port/acquisition/watch-now` is the core's request to fetch a title at once, for a viewer
who may skip the queue. It makes sure an approved request covers exactly that scope (a film,
or one episode), runs the interactive search over the scope, keeps the releases the profile
accepts and nobody has turned down (best first, at most eight), and asks the Downloads module's
`grab-instant` which of them the preferred client holds. The row it gets back is imported at
once, not at the next pass, and the reply says where the files landed. An episode with no instant
release of its own falls back to the season pack of its season.

Replies, tagged by `status`:

- `imported`: `request_id`, `download_id`, `release` (`key`, `title`, `claimed_height`), `paths`
  and `primary_path`, the file to play.
- `requested`: nothing instant; the approved request stays and the ordinary pipeline continues.
- `unavailable`: `reason` is `noEngine`, `noIndexer` or `noRelease`.

The core reads the file back. `/_port/acquisition/watch-now-reject` is how it says the file was
not what its release promised: the release is blocklisted for the title, the files are deleted,
the download is released and the request is reopened. The core then asks again with the
rejected release keys in `exclude`. `/_port/acquisition/watch-now-capable` answers whether the
button can be offered at all.

The settings page is the shared `/api/admin/settings?view=acquisition` view.
Disabling the module removes search, grab and the automatic pass, and hides that
page with them.

Its `storage.core` grant is read on the catalog tables plus the user columns the
requester needs, and write on `requests`, `wanted`, `monitored` and
`acq_file_tmdb`.

Layout: `server/` (backend crate), `ui/` (frontend), `locales/` (i18n),
`module.json` (manifest). See `modules/README.md` for the module authoring guide.
