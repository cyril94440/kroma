import { z } from 'zod';
import type { HdrCapabilities, PlaybackCapabilities } from './capabilities';

const hdrSupportSchema = z.object({
  dolbyVisionDecoderProfiles: z.array(z.number().int().nonnegative()).optional(),
  displayHdrTypes: z.array(z.enum(['dolbyVision', 'hdr10', 'hdr10Plus', 'hlg'])).optional(),
});

/** What a device answered when asked about HDR: the Dolby Vision profiles its
 * decoders take and the HDR systems its display reports. A field left out is a
 * question the platform cannot answer, which is not the same as an empty list. */
export type HdrSupport = z.infer<typeof hdrSupportSchema>;

export function parseHdrSupport(value: unknown): HdrSupport | null {
  const parsed = hdrSupportSchema.safeParse(value);
  return parsed.success ? parsed.data : null;
}

/** The table refined by what the device answered. Dolby Vision is claimed only
 * when the display reports it and the decoder side is either unknown or has a
 * profile; a display list narrows HDR10, HDR10+ and HLG, and an empty one is an
 * SDR display. An answer that names neither side leaves the table as it was. */
export function withHdrSupport(
  caps: PlaybackCapabilities,
  support: HdrSupport | null | undefined,
): PlaybackCapabilities {
  const { dolbyVisionDecoderProfiles: profiles, displayHdrTypes: display } = support ?? {};
  if (!display && !profiles) return caps;
  const dolbyVision =
    display !== undefined &&
    display.includes('dolbyVision') &&
    (profiles === undefined || profiles.length > 0);
  const hdr: HdrCapabilities = {
    hdr10: display ? display.includes('hdr10') : caps.hdr.hdr10,
    hdr10Plus: display ? display.includes('hdr10Plus') : caps.hdr.hdr10Plus,
    hlg: display ? display.includes('hlg') : caps.hdr.hlg,
    dolbyVision,
    ...(dolbyVision && profiles ? { dolbyVisionProfiles: profiles } : {}),
  };
  return { ...caps, hdr, hdrDeclared: true };
}
