# Hardware transcoding

Most of the time, KROMA sends a video file to the screen exactly as it is. This
is called direct play, and it costs the server almost nothing. KROMA converts a
video only when the screen cannot play it, for example an HEVC film on a TV
that only plays H.264, or a 4K film on a device that stops at 1080p.

That conversion is the one part of KROMA that needs real processing power. On a
NAS, one 4K film converted on the processor can use all of it. A graphics chip
does the same work at a few percent of the processor. This page shows how to
give KROMA access to it.

## What KROMA can use

| Hardware | Name in the log | Where |
| --- | --- | --- |
| Intel graphics, Quick Sync | `qsv` or `vaapi` | most Intel processors, including Intel Synology models |
| AMD graphics, VA-API | `vaapi` | Linux computers that run KROMA outside Docker, with the AMD driver installed |
| NVIDIA graphics cards | `nvenc` | computers with an NVIDIA card |
| Apple silicon and Intel Macs | `videotoolbox` | macOS, when KROMA runs outside Docker |

KROMA tries each one at startup and only uses it if it can convert a test
frame. A chip that is present but that KROMA cannot open counts as missing.

## Check what KROMA uses

Every start, the server log says where conversions run:

```
INFO video re-encodes run on hardware accel="vaapi" reason=h264_vaapi encoded a test frame
```

or, when it found nothing it could use, a line that starts with
`video re-encodes run on the CPU`. Its `reason` names what each method lacked,
for example:

```
WARN video re-encodes run on the CPU reason=every listed device refused a test encode (qsv: /dev/dri/renderD128 is not readable by this user ...)
```

Read the log in KROMA under **Server** > **Logs**, with
`docker compose logs kroma` in Docker, or in
`/var/packages/kroma/var/kroma.log` on the Synology package.

## Synology package

The package sets it up on Intel models. There is nothing to do. See
[Synology: hardware transcoding](synology.md#hardware-transcoding) for the
models that have no graphics chip, or no driver for it.

## Docker with Intel graphics

KROMA needs three things, and the log line names the one that is missing.

1. **The device.** Pass the graphics device into the container. Check that it
   exists on the computer first: `ls /dev/dri` lists a `renderD128` entry.
2. **The permission to open it.** The device belongs to a group, usually
   `render`, and KROMA's user must be in it. Find the group's ID:

   ```bash
   getent group render | cut -d: -f3
   ```

   On some systems the group is `video`. On a Synology, it is `videodriver`.

3. **Both in the compose file**, under `kroma:`:

   ```yaml
       devices:
         - /dev/dri:/dev/dri
       group_add:
         - "993"
   ```

   Replace `993` with the group ID from step 2.

Recreate the container with `docker compose up -d` and read the log again.

The image carries Intel's drivers, so nothing else is installed on the
computer. It carries no driver for AMD graphics: in Docker, an AMD chip is not
used.

## Docker with an NVIDIA card

Install the
[NVIDIA Container Toolkit](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/latest/install-guide.html)
on the computer, then give the container the card:

```yaml
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: all
              capabilities: [gpu, video]
```

## Choose by hand

To force a method, or to rule hardware out while you look for a problem, set
`KROMA_HWACCEL` to `qsv`, `vaapi`, `nvenc`, `videotoolbox` or `software`. On a
computer with more than one graphics device, `KROMA_RENDER_NODE` selects the
one to use. See the [configuration reference](configuration.md#transcoding).

## HDR films

When KROMA converts an HDR film for a screen that cannot show HDR, it also
converts the colours to standard ones, so the picture is not dark and grey. That
picture is never larger than 1080p: KROMA shrinks it first, because mapping
colours is the costliest step and costs in proportion to the pixels.

With a graphics chip, KROMA keeps the chip decoding and shrinking the picture
and, if it works on your machine, mapping the colours there too. At startup it
tries, on a small test clip, the chip's own colour filter and then handing the
1080p picture to the processor; it uses the first that works and falls back to
the processor for every step when none does. The log line `tone-mapping settled`
says which routes worked. When the processor maps the colours, the conversion
counts as two of the conversions allowed at once and background work waits.

On a small NAS without a graphics chip, a 4K HDR film is converted at 720p
instead of 1080p when the machine has four cores or fewer, because that is the
size it can keep up with (about 50 pictures a second against 28 on four cores
at 1080p). Set `KROMA_TONEMAP` to `off` to skip the colour step, or to `cpu` to
rule out the chip's own routes while you look for a problem. Dolby Vision
profile 5 films need a graphics card with Vulkan support for this step, are
decoded by the processor, and without it keep their colours as they are.

Dolby Vision profile 7 films (Blu-ray rips) play as their HDR10 layer when KROMA
streams them, even on a Dolby Vision screen, and their TrueHD soundtrack is
converted rather than copied. Dolby Vision profile 5 films are sent as Dolby
Vision only to apps that say their screen supports it; any other app gets the
plain stream, or the converted one when your machine can map the colours.

## After an upgrade

A release that makes KROMA read more from your files (HDR format, Dolby Vision
detail, audio profiles) does not make your library unreadable while it catches
up. Every file keeps the details it had, and the ones that can gain are read
again in the background, 200 at a time at each startup and each scan, one at a
time and after any new files. A large library is up to date after a few days of
ordinary use.

## When there is no graphics chip

Without hardware, KROMA converts on the processor. That works for 1080p on
most machines, and is slow for 4K. Two things help more than any setting:

- Play on a screen that supports the file. Then KROMA converts nothing.
- Keep the number of conversions at once low with `KROMA_MAX_TRANSCODES`. A
  request past the limit is refused, so the screen says so instead of
  stuttering.
