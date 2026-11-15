import type { ReactNode } from 'react';
import type { LiveBackend } from '#tv/features/live/engine/liveEngine';
import { ExpoLiveEngine } from '#tv/features/live/native/ExpoLiveEngine';
import { LiveSurface } from '#tv/features/live/native/LiveSurface';
import { TvLive } from '#tv/features/live/TvLive';

const NATIVE_LIVE: LiveBackend<ExpoLiveEngine> = {
  createLiveEngine: ({ url, listeners }) => new ExpoLiveEngine(url, listeners),
  Surface: LiveSurface,
};

/** The live screen on the native shells, on expo-video's platform player. */
export function NativeLive(): ReactNode {
  return <TvLive backend={NATIVE_LIVE} />;
}
