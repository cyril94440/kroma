import { VideoView } from 'expo-video';
import type { ReactNode } from 'react';
import { StyleSheet } from 'react-native';
import type { LiveSurfaceProps } from '#tv/features/live/engine/liveEngine';
import type { ExpoLiveEngine } from '#tv/features/live/native/ExpoLiveEngine';

/** The live picture on the native shells: expo-video's view over the engine's player. */
export function LiveSurface({ live, title }: Readonly<LiveSurfaceProps<ExpoLiveEngine>>): ReactNode {
  const player = live.engineRef.current?.player ?? null;
  if (!player) return null;
  return (
    <VideoView
      key={live.surfaceNonce}
      player={player}
      style={StyleSheet.absoluteFill}
      contentFit="contain"
      surfaceType="textureView"
      nativeControls={false}
      accessibilityLabel={title}
    />
  );
}
