# Media

Status: **SHIPPED** in part, with the model and the stream truth **SHIPPED**. Per-section and
per-requirement status is called out where it differs.

What a title *is* once KROMA knows about it: the technical truth about its streams. That
truth is the input to every playback decision. If a file direct-plays, it is because its
streams match what a client can render; if it cannot, this is where the reason comes from.

This file owns the *vocabulary* of media: the model, the containers, the codecs, the
stream properties. The per-device capability matrix lives in [`surfaces/`](../surfaces/)
and the decision of what to send lives in [`playback/`](../playback/). Both consume the
terms defined here; neither is redefined here.

## The media model

Status: **SHIPPED** in part; each requirement carries its own.

Five nouns, nested, each the child of the one before:

- **MEDIA-1** (SHIPPED) - **Title.** The work a person searches for: a film, or one episode
  of a series. It is the unit [`library/`](../library/) matches to metadata, and it carries
  no bytes.
- **MEDIA-2** (SHIPPED) - **Edition.** A named cut of a title: theatrical, director's,
  extended, remastered. Different runtimes, different content. A title with one cut has one
  unnamed edition.
- **MEDIA-3** (SHIPPED) - **Media file.** One physical file on disk that realises an edition
  at a given fidelity. An edition may have several: a 1080p file and a 4K file are two media
  files of the same edition, not two titles and not two editions.
- **MEDIA-4** (AGREED) - **Stream.** One track inside a media file, exactly one of video,
  audio or subtitle. A media file has one or more video streams (usually one), zero or more
  audio streams, and zero or more subtitle streams.
- **MEDIA-5** (AGREED) - **Stream properties.** The describable facts about a stream: codec,
  resolution, bit depth, channel layout, language, disposition. These are what a client is
  matched against.

**MEDIA-6** (SHIPPED) - The nesting is strict and total: every stream belongs to exactly one
media file, every media file to exactly one edition, every edition to exactly one title.
Nothing floats.

An edition is **derived** from the cut a file's name carries rather than stored beside it. That
is what makes the nesting total: the mapping from file to edition is a function, so there is no
state to drift and no file that can end up in none. A title whose files name only quality tiers
has exactly one unnamed edition holding all of them, and renaming a file on disk moves it,
because the name is where the cut was always declared.

## Versions of one title

Status: **SHIPPED** in part; each requirement carries its own.. This resolves the open question on multiple versions.

**MEDIA-7** (SHIPPED) - A 1080p file and a 4K file of the same cut are **two media files of
one edition**. They are never modelled as separate titles, and the library never shows a
duplicate. A person picks a title and a cut; the fidelity is chosen for them.

- **MEDIA-8** (SHIPPED) - KROMA ranks a title's media files and keeps a **preferred** one.
  Rank order: resolution, then HDR over SDR, then bit depth, then audio channel count, then
  bitrate. The preferred file is the default source for a play request.
- **MEDIA-9** (AGREED) - The preference is a *default*, not a lock. The model enumerates a
  title's media files and makes them comparable, so a client that cannot render the preferred
  file can be served a lesser file of the same edition instead of falling back to transcoding.
  Which file a client actually receives is [`playback/`](../playback/)'s decision.
- **MEDIA-10** (AGREED) - Editions are surfaced to the person, because they are different
  content; fidelity variants are not, because they are the same content at different quality.
  A person chooses a cut, never a resolution. Editions are enumerated on the wire and shown
  where a person looks at a title's files, and no surface ever offers a resolution. **Choosing
  a cut is the part not built**: `/stream` takes a file id but `/hls` does not, so a picker
  would play the chosen cut unmodified and the default cut whenever a transcode is needed.

## Containers and codecs

Status: **SHIPPED**

**MEDIA-11** (SHIPPED) - First-class means KROMA fully describes the format, preserves it end
to end, and expects direct play wherever a client can render it. Everything else is
*readable* but not privileged.

**MEDIA-12** (SHIPPED) - The first-class containers are **MP4**, **MKV** and **WebM**.

The first-class video codecs, HEVC first:

- **MEDIA-13** (SHIPPED) - **HEVC / H.265** is the priority codec. 8-bit and 10-bit, SDR and
  HDR, are all first-class.
- **MEDIA-14** (SHIPPED) - **H.264 / AVC** is the universal floor, assumed playable
  everywhere.
- **MEDIA-15** (SHIPPED) - **AV1** is first-class media truth whatever the client generation,
  so a capable client direct-plays it.
- **MEDIA-16** (SHIPPED) - **VP9** is first-class within WebM, chiefly for the browser
  surface.

**MEDIA-17** (SHIPPED) - A codec being first-class states how *KROMA* handles it, never that a
particular device renders it. That is the matrix in [`surfaces/`](../surfaces/).

First-class audio and subtitle codecs are named in their sections below.

## Bit depth and HDR

Status: **SHIPPED** in part; each requirement carries its own.

**MEDIA-18** (AGREED) - HDR is preserved end to end or it is not offered. KROMA never
silently flattens HDR to SDR, and a tone-mapped picture is surfaced as the compromise it is
by [`playback/`](../playback/).

Matching a client against a variant is only a refusal where the variant has no picture of
its own to fall back on. HDR10+ and HLG degrade cleanly, and so does Dolby Vision whenever its
base layer is a picture. What that base layer is comes from the stream's own
`bl_signal_compatibility_id`, which the server sends as `dolbyVisionCompatibility`: 1 and 6 are
an HDR10 picture, 4 is HLG, 2 is SDR, and 0 is none. A device with no Dolby Vision decoder
draws the base layer, and a stream whose id is 0, profile 5 in practice, is refused, because
its base layer is not a picture and the colours come out wrong. The profile number decides
only where the probe carried no id: 5 has no base layer and 7 has an HDR10 one. Profile 7 is
dual layer, an HDR10 base plus an enhancement layer: a device without a Dolby Vision decoder
plays the base layer alone, and one with a decoder that takes profile 7 plays it as Dolby
Vision. The enhancement layer is never required for a picture.

A client declares the HDR systems its display shows (`hdr10`, `hdr10plus`, `hlg`, `dv`) on
the stream request, so the server can tone-map for an SDR display and decline to copy a stream
that has no base layer to a device with no Dolby Vision decoder. Omitting the declaration
means the table is a guess, and the server behaves as before.

- **MEDIA-19** (SHIPPED) - **Bit depth.** 8-bit and 10-bit are first-class, and 10-bit is
  retained as a property of the video stream rather than rounded away in the model.
- **MEDIA-20** (SHIPPED) - KROMA distinguishes **HDR10**, **HDR10+**, **Dolby Vision** and
  **HLG** as separate properties, because each has distinct client support, and it records
  Dolby Vision's profile, level, base-layer compatibility and enhancement layer, because a
  profile a client cannot decode is not the same as one it can. HDR10+ is per-frame metadata,
  so the probe reads the first frame of a stream that is already PQ; a read that fails or runs
  long leaves the stream HDR10, and a Dolby Vision stream stays Dolby Vision whatever else it
  carries.
- **MEDIA-21** (SHIPPED) - Colour primaries, transfer characteristics and matrix coefficients
  travel with the video stream, and a client is matched against the exact HDR variant rather
  than a generic "HDR" flag.
- **MEDIA-22** (AGREED) - Where dynamic metadata, HDR10+ or Dolby Vision, cannot be carried
  to a client, the base HDR10 layer is preserved rather than HDR discarded entirely, and the
  player says so: Dolby Vision drawn as its HDR10, HLG or SDR layer, HDR10+ drawn as HDR10, and
  HDR tone-mapped to an SDR display are each a notice, never a silent change. It is a fallback
  [`playback/`](../playback/) makes visible. Dolby Vision profile 7 is always delivered over HLS as
  its HDR10 base layer alone, so that notice holds for it even on a device that decodes Dolby
  Vision; only the original file, played directly, keeps its enhancement layer. Its TrueHD is
  re-encoded rather than copied beside it (MEDIA-48).

## Audio

Status: **SHIPPED** in part; each requirement carries its own.

**MEDIA-23** (AGREED) - The first-class audio codecs are **AAC**, **AC-3** and **E-AC-3**
(Dolby Digital and Plus), **TrueHD**, **DTS** and **DTS-HD**, **FLAC** and **Opus**. Only the
first three and Opus are handled first-class today; the rest are the part still AGREED,
though TrueHD and DTS are already probed with their profile and either copied or kept as a 5.1
transcode (MEDIA-26).

**MEDIA-24** (SHIPPED) - An audio stream's **channel layout**, stereo, 5.1, 7.1 or Atmos
objects, is a first-class property: the probe keeps ffprobe's layout and profile, and a
profile naming Dolby Atmos or DTS:X marks the stream `spatial`. DTS-HD Master Audio stays
`dts` in the codec and shows in the profile.

- **MEDIA-25** (SHIPPED) - **Passthrough** is the default for multichannel and lossless audio:
  the bitstream reaches a device or receiver that can decode it untouched. This is direct play
  for audio and is always preferred.
- **MEDIA-26** (AGREED) - **Downmixing** to stereo happens only when the target cannot render
  the source layout, only as an explicit fallback, and never silently. A client that declares
  E-AC-3 (or AC-3) is given a surround source as 5.1 at 640 kbps instead of a stereo downmix;
  object audio (Atmos, DTS:X) survives only a copy, because no transcode here writes it. A downmix is a
  channel-count reduction and therefore a compromise; the original stream is retained
  unchanged as the source, and telling the person is [`playback/`](../playback/)'s job.
- **MEDIA-27** (SHIPPED) - Multiple audio streams, languages and commentary alike, are all
  enumerated and selectable. The default follows the file's own disposition flags, then the
  profile's language preference.

- **MEDIA-48** (SHIPPED) - A video copied into fragmented MP4 keeps what makes it HDR: HEVC is
  tagged `hvc1`, profile 8 keeps `hvc1` with its `dvvC` box so a Dolby Vision decoder engages
  and everything else draws the base layer, and a single-track profile 7 is cut down to its
  HDR10 base layer and signalled as nothing else. Profile 5 has no base layer a plain decoder
  can draw, so `dvh1` and the configuration box go only to a client that declared `dv`; any
  other client, and any host that cannot tone-map it for one that declared `hdr` without `dv`,
  gets plain `hvc1` with no box, which opens everywhere and draws the base layer's own
  colours, as it did before the box existed. The Apple HLS rules ask `dvh1` of profile 5 and
  nothing else can open it. TrueHD needs a muxer level that also writes the box from the
  stream's own record, which ffmpeg 6.1 cannot drop, so it is never copied beside a profile 7
  or a profile 5 not signalled as Dolby Vision: it is re-encoded like any soundtrack the
  client cannot decode. `hvc1` rather than `hev1` is deliberate (Apple requires it, and
  in-band parameter sets are the less compatible form for fragmented MP4 HLS); the residual
  risk is a television whose decoder is not probed for it, and a browser only declares `hevc`
  after probing `hvc1`.
- **MEDIA-49** (SHIPPED) - A video re-encoded to 8-bit H.264 from an HDR source is tone-mapped
  and tagged BT.709, on hosts whose ffmpeg has zimg (or libplacebo with a working Vulkan
  device); elsewhere the re-encode is as before. A tone-mapped picture is never larger than
  1080p: it is scaled down first, on the device that decodes it where there is one, and the
  re-encode is a 1080p rung even for a client that sent no frame cap. At boot each device
  route is proved on a small HDR10 clip through the same flags a session uses: the device's own
  filter (`tonemap_vaapi`, `vpp_qsv`), else hardware decode and scale with a download of the
  1080p picture and a mapping on the CPU, else the CPU for every step; one that fails is not
  offered. A mapping on the CPU costs two re-encode slots and holds background work back, and
  on a host with four cores or fewer a 4K source drops to a 720p rung. Dolby Vision profile 5
  needs libplacebo and a software decoder, which is what hands over its reshaping.
  `KROMA_TONEMAP=off` rules tone-mapping out and `KROMA_TONEMAP=cpu` rules out the device routes.

**MEDIA-47** (AGREED) - An audio stream carries the region of its language when the file says
so: from its language tag in any language (`fr-CA`, `pt-BR`), else from words of its title
that name a French dub or a country (VFQ, Québécois or Canada for `CA`; VFF, TrueFrench or
France for `FR`; Belgique for `BE`; Suisse for `CH`). A stream that says neither is its
language with no region. The quality rules read it
([`../modules/acquisition.md`](../modules/acquisition.md)).

## Subtitles

Status: **AGREED**

**MEDIA-28** (AGREED) - A subtitle stream is either **embedded**, a track inside the media
file, or **sidecar**, a separate file the library associates with it. Both are first-class
and both enumerate the same way once known.

- **MEDIA-29** (AGREED) - The first-class subtitle formats are **SRT**, **WebVTT** and
  **ASS/SSA** as text, **PGS** and **VobSub** as images. Text is the default because it
  overlays without touching the video; image subtitles are heavier and sometimes force a
  compromise.
- **MEDIA-30** (AGREED) - The **forced** disposition, only the foreign-language lines a
  viewer needs, and the **SDH** disposition, for the deaf and hard of hearing, are recorded
  and honoured as distinct properties. A person picking forced gets forced, not full.
- **MEDIA-31** (AGREED) - **Burning in**, rendering a subtitle permanently into the video, is
  a last resort used only when a subtitle cannot reach the target as a selectable overlay. It
  is never the default and never silent, the original subtitle stream is always preserved, and
  the burn is a derived output rather than a replacement. *When* it happens is
  [`playback/`](../playback/).

## Artwork and images

Status: **SHIPPED** in part; each requirement carries its own.

**MEDIA-32** (SHIPPED) - Every title carries a **poster**, a **backdrop**, a **logo** and, per
episode, a **thumb**. These are media too, and are treated with the same discipline.

- **MEDIA-33** (AGREED) - Image sources rank by trust: embedded in the media file, then a
  sidecar file next to it, then fetched by the [`library/`](../library/) metadata refresh. A
  local image outranks a fetched one, and a person's explicit choice outranks both.
- **MEDIA-34** (AGREED) - KROMA derives a fixed set of sizes per image, a small grid
  thumbnail through a full-bleed backdrop, so a client requests a size and never the original.
  The original is retained as the master.
- **MEDIA-35** (AGREED) - Derived sizes are cached and served without re-deriving, and a
  source image changing invalidates its own derivatives and nothing else.
- **MEDIA-36** (AGREED) - Artwork is never a reason a title fails to appear. A title with no
  image shows a typed placeholder, never a broken one.

## Probing and trust

Status: **SHIPPED** in part; each requirement carries its own.. This resolves the open question on probe trust.

**MEDIA-37** (SHIPPED) - KROMA learns a file's streams by **probing** it once, when the
library first sees it, and **trusts that probe** for playback decisions. Re-probing on every
play would tax the server for a fact that rarely changes.

A probe answers in one of three ways, and they are not the same answer. A file it
described is described. A file it opened and refused is unreadable (MEDIA-45), and its
reason is kept. A probe that could not run at all, no ffprobe on the box or output KROMA
could not parse, is neither: nothing has been learned about the file, so the container
extension stands in for the video codec until something can look properly.

- **MEDIA-38** (AGREED) - A probe is the authoritative stream truth until the file's bytes
  change, meaning its size or modification time, which invalidates the probe and schedules a
  re-probe.
- **MEDIA-39** (AGREED) - When a direct play fails in a way that implicates the stream
  description, such as a codec the probe named but the client could not initialise, KROMA
  re-probes that file once and records the corrected truth, so the same failure does not
  recur. A failure that implicates the network or the disk instead does not trigger a
  re-probe.
- **MEDIA-40** (AGREED) - Trust is per-file and durable: a corrected probe is written back,
  not held in memory for one session.
- **MEDIA-56** (SHIPPED) - A release that makes the probe learn something new (HDR format,
  Dolby Vision detail, an audio profile) never unprobes a library. Each file carries the
  version of the facts its probe wrote, and the files that could gain from the new version are
  read again in the background, after every file never probed, one at a time, a batch of 200 a
  pass (a boot, a scan) so a large library catches up over days of ordinary use. Until a file's
  new facts land, in the one write that replaces the old ones, it is as probed as it was: it
  keeps representing its item, counts as on disk and shows what it showed. A stub with no
  local copy is left alone where this ffmpeg cannot read the network, and a re-read that
  fails keeps the facts the file had.

## Files KROMA can read but not fully describe

Status: **SHIPPED** in part; each requirement carries its own. This resolves the
"read but not describe" must-answer.

The fault is recorded against the **media file**, not against the title, and travels
with the title because a file list travels with it. A title with one good copy and one
truncated copy is not a broken title: the good copy represents it, and the broken one
shows its reason where a person is looking at files. The reason is the probe's own
words, which is why it is not a translated string.

**MEDIA-41** (AGREED) - A file whose container opens and whose streams enumerate, but which
carries a stream KROMA cannot fully describe, an unknown codec or absent or contradictory
metadata, is **partially known**. It is never hidden and never silently dropped.

- **MEDIA-42** (AGREED) - The title **appears** in the library with whatever *is* known. A
  single indescribable stream does not disqualify the file, and the describable streams remain
  first-class.
- **MEDIA-43** (AGREED) - The unknown stream is marked **undescribed** and carries its raw
  identifier, so a person and a diagnostician can see exactly what was not understood.
- **MEDIA-44** (SHIPPED) - An undescribed stream is *not direct-playable*, because KROMA will
  not gamble that a client renders what KROMA itself cannot name
  ([`playback/`](../playback/)).
- **MEDIA-45** (SHIPPED) - A file that will not open at all, a truncated or corrupt container,
  is **unreadable**: surfaced as a typed error against the title with the reason, and excluded
  from play until it is re-scanned. It is a visible fault, not an absence.
- **MEDIA-46** (AGREED) - Undescribed and unreadable are distinct states. The first is "we
  see it but cannot vouch for it", the second is "we cannot see it". Both are on record;
  neither is a blank.

## Remote files

Status: **SHIPPED**.

A media file need not hold its bytes. A **remote file** is a media file whose bytes live
with a module, kept in the library as a small stub, so a title fetched through a service is
streamed instead of stored. Everything above holds for it: the title appears, is probed,
plays, and carries the facts a client decides on.

- **MEDIA-50** (SHIPPED) - A stub is a title like any other: it appears with the container
  and byte size of the video it stands for and its title and episode as that video's name
  would give them, and a stub that cannot be read, or names no video container, is skipped
  with a warning rather than shown broken.
- **MEDIA-51** (SHIPPED) - A media file whose bytes are streamed says so (`remote`), so a
  client can label it. The wire carries it only when it is true.
- **MEDIA-52** (SHIPPED) - Where a remote file's bytes are is answered by its source each
  time they are needed, as a complete local copy or as a short-lived link, and never kept
  past the read that asked.
- **MEDIA-53** (SHIPPED) - A link is used only if it is `https`, carries no credentials, and
  names a public host the source declared it connects to. A local copy is used only if it is
  a regular file of the declared size inside the source's own folders. Anything else is
  refused, and the link, a bearer, never appears in a log or an error.
- **MEDIA-54** (SHIPPED) - A source that is off, gone or silent fails playback with a
  translated message that says which, never a hang, and fails a probe in a way a later
  attempt can recover from.
- **MEDIA-55** (SHIPPED) - Work that reads a whole title (scrub thumbnails, intro and credits
  detection, loudness, embedded-subtitle extraction, transcription) runs on a remote file only
  once its source holds a complete local copy. Until then it is deferred, neither run over the
  network nor recorded as failed, and it never asks the source to fetch the copy.
