import type { ReactNode } from 'react';
import { ClipSurface } from '#tv/features/playback/browser/ClipSurface';
import type { ClipBackend } from '#tv/features/playback/player/clipBackend';
import { HtmlClipEngine } from '#tv/features/playback/player/htmlClipEngine';
import { TvClipPlayer } from '#tv/features/playback/TvClipPlayer';

const BROWSER_CLIP: ClipBackend<HtmlClipEngine> = {
  createClipEngine: ({ url, video, listeners }) =>
    video ? new HtmlClipEngine(video, url, listeners) : null,
  Surface: ClipSurface,
};

export function BrowserClip(): ReactNode {
  return <TvClipPlayer backend={BROWSER_CLIP} />;
}
