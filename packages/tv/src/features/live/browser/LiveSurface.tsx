import type { ReactNode } from 'react';
import type { LiveSurfaceProps } from '#tv/features/live/engine/liveEngine';
import type { HtmlLiveEngine } from '#tv/features/live/browser/HtmlLiveEngine';

const ROUNDED = { borderRadius: 'inherit' } as const;

/** The live picture on the browser targets: a plain `<video>` the engine plays on. */
export function LiveSurface({
  live: { attachVideo },
  title,
}: Readonly<LiveSurfaceProps<HtmlLiveEngine>>): ReactNode {
  return (
    <video ref={attachVideo} playsInline aria-label={title} style={ROUNDED}>
      <track kind="captions" />
    </video>
  );
}
