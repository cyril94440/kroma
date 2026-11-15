import type { ReactNode } from 'react';
import { HtmlLiveEngine } from '#tv/features/live/browser/HtmlLiveEngine';
import { LiveSurface } from '#tv/features/live/browser/LiveSurface';
import type { LiveBackend } from '#tv/features/live/engine/liveEngine';
import { TvLive } from '#tv/features/live/TvLive';

const BROWSER_LIVE: LiveBackend<HtmlLiveEngine> = {
  createLiveEngine: ({ url, video, listeners }) =>
    video ? new HtmlLiveEngine(video, url, listeners) : null,
  Surface: LiveSurface,
};

/** The live screen on the browser shells, on the page's own `<video>`. */
export function BrowserLive(): ReactNode {
  return <TvLive backend={BROWSER_LIVE} />;
}
