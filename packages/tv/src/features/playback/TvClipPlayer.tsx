import { Player, TV_FLAGS } from '@kromatv/ui';
import { type ReactNode, useEffect, useMemo } from 'react';
import { usePlayerDock } from '#tv/app/playerDock';
import { useEnv } from '#tv/app/providers/env';
import { useNav, useParams } from '#tv/app/router';
import type { ClipBackend, ClipEngine } from '#tv/features/playback/player/clipBackend';
import { useClipPlayback } from '#tv/features/playback/player/useClipPlayback';
import { useTvClipController } from '#tv/features/playback/use-tv-clip-controller';

const noStoryboard = () => null;

/** A short file the caller already holds a URL for, such as a goal clip, in the
 *  shared player chrome on the shell's clip backend. The clip running out leaves,
 *  as Back does. A film in the picture-in-picture window stops for it. */
export function TvClipPlayer<E extends ClipEngine>({
  backend,
}: Readonly<{ backend: ClipBackend<E> }>): ReactNode {
  const { url, title, subtitle } = useParams('clip');
  const nav = useNav();
  const { mousePointer } = useEnv();
  const { docked, undock } = usePlayerDock();
  useEffect(() => {
    if (docked) undock();
  }, [docked, undock]);

  const clip = useClipPlayback(backend, url);
  const controller = useTvClipController(clip);
  const flags = useMemo(
    () => ({ ...TV_FLAGS, pointer: mousePointer, tracks: false }),
    [mousePointer],
  );
  const { endedNonce } = clip;
  useEffect(() => {
    if (endedNonce > 0) nav.back();
  }, [endedNonce, nav]);
  const { Surface } = backend;

  return (
    <Player.Root controller={controller} flags={flags} title={title} onClose={nav.back}>
      <Player.Title>{title}</Player.Title>
      <Player.Subtitle>{subtitle}</Player.Subtitle>
      <Player.Transport tileAt={noStoryboard} />
      <Player.Media>
        <Surface clip={clip} title={title} />
      </Player.Media>
    </Player.Root>
  );
}
