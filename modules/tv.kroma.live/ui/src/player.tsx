import { ModuleSlot, useT } from '@kromatv/module-sdk';
import { type CSSProperties, useCallback, useEffect, useState } from 'react';
import type { PlayerChannel, PlayerHandle } from './player-handle';
import { useHls } from './use-hls';

interface LivePlayerProps {
  url: string | null;
  title: string;
  onLost: () => void;
  channel?: PlayerChannel | null;
  onHandle?: (handle: PlayerHandle | null) => void;
  onPlaying?: (playing: boolean) => void;
}

const FRAME: CSSProperties = {
  position: 'relative',
  background: '#000',
  width: '100%',
  aspectRatio: '16 / 9',
  borderRadius: 12,
  overflow: 'hidden',
};
const IMMERSIVE: CSSProperties = {
  position: 'fixed',
  inset: 0,
  zIndex: 1000,
  background: '#000',
  width: '100vw',
  height: '100vh',
  borderRadius: 0,
};
const OVERLAY: CSSProperties = {
  position: 'absolute',
  top: 0,
  left: 0,
  right: 0,
  pointerEvents: 'none',
};
const BAR: CSSProperties = {
  position: 'absolute',
  left: 0,
  right: 0,
  bottom: 0,
  display: 'flex',
  gap: 8,
  alignItems: 'center',
  padding: 10,
  background: 'linear-gradient(transparent, rgba(0,0,0,.7))',
};
const BUTTON: CSSProperties = {
  background: 'rgba(255,255,255,.16)',
  color: '#fff',
  border: 0,
  borderRadius: 8,
  padding: '6px 12px',
  cursor: 'pointer',
  font: 'inherit',
};

export function LivePlayer({
  url,
  title,
  onLost,
  channel = null,
  onHandle,
  onPlaying,
}: Readonly<LivePlayerProps>) {
  const t = useT();
  const [video, setVideo] = useState<HTMLVideoElement | null>(null);
  const [immersive, setImmersive] = useState(false);
  const [muted, setMuted] = useState(false);
  const state = useHls(video, url, onLost, channel?.id ?? null);
  const { handle, playing } = state;
  useEffect(() => {
    onHandle?.(handle);
    return () => onHandle?.(null);
  }, [handle, onHandle]);
  useEffect(() => {
    onPlaying?.(playing);
  }, [playing, onPlaying]);

  const toggleImmersive = useCallback(() => setImmersive((on) => !on), []);
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'f') toggleImmersive();
      if (event.key === 'Escape') setImmersive(false);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [toggleImmersive]);

  return (
    <div style={{ ...FRAME, ...(immersive ? IMMERSIVE : null) }} data-testid="live-player">
      <video
        ref={setVideo}
        aria-label={title}
        playsInline
        muted={muted}
        disablePictureInPicture
        onDoubleClick={toggleImmersive}
        onClick={state.toggle}
        style={{ width: '100%', height: '100%', objectFit: 'contain', background: '#000' }}
      >
        <track kind="captions" />
      </video>
      {state.blocked ? (
        <button
          type="button"
          style={{ ...BUTTON, position: 'absolute', inset: 'auto', top: '45%', left: '45%' }}
          onClick={state.goLive}
        >
          {t('live.player.play')}
        </button>
      ) : null}
      {state.failed ? (
        <div style={{ position: 'absolute', top: 12, left: 12, color: '#fff' }}>
          {t('live.player.failed')}
        </div>
      ) : null}
      {handle ? (
        <div style={OVERLAY}>
          <ModuleSlot
            name="tv.kroma.live/player.overlay"
            player={handle}
            channel={channel}
            playing={playing}
            immersive={immersive}
          />
        </div>
      ) : null}
      <div style={BAR}>
        <button type="button" style={BUTTON} onClick={state.toggle}>
          {state.playing ? t('live.player.pause') : t('live.player.play')}
        </button>
        <button type="button" style={BUTTON} onClick={() => setMuted((on) => !on)}>
          {muted ? t('live.player.unmute') : t('live.player.mute')}
        </button>
        {state.behind ? (
          <button type="button" style={{ ...BUTTON, background: '#ef4444' }} onClick={state.goLive}>
            {t('live.player.backToLive')}
          </button>
        ) : null}
        <span style={{ flex: 1 }} />
        <button type="button" style={BUTTON} onClick={toggleImmersive}>
          {immersive ? t('live.player.exitImmersive') : t('live.player.immersive')}
        </button>
      </div>
    </div>
  );
}
