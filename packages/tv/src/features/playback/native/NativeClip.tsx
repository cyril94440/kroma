import type { ReactNode } from 'react';
import { ClipSurface } from '#tv/features/playback/native/ClipSurface';
import type { ClipBackend } from '#tv/features/playback/player/clipBackend';
import { ExpoClipEngine } from '#tv/features/playback/player/expoClipEngine';
import { TvClipPlayer } from '#tv/features/playback/TvClipPlayer';

const NATIVE_CLIP: ClipBackend<ExpoClipEngine> = {
  createClipEngine: ({ url, listeners }) => new ExpoClipEngine(url, listeners),
  Surface: ClipSurface,
};

export function NativeClip(): ReactNode {
  return <TvClipPlayer backend={NATIVE_CLIP} />;
}
