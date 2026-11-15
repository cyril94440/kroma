import { type CSSProperties } from 'react';
import { OverlayProps } from './sync-player';
import { useParty } from './use-party';
import { VoiceSpeakers } from './voice-speakers';
import { VotePanel } from './vote-panel';

const BANNER: CSSProperties = { pointerEvents: 'auto', margin: 12, maxWidth: 420 };
const SPEAKERS: CSSProperties = { position: 'absolute', left: 12, bottom: 12 };
const FLOATS: CSSProperties = {
  position: 'absolute',
  right: 12,
  top: 12,
  display: 'flex',
  flexDirection: 'column',
  gap: 4,
  color: '#fff',
  fontSize: 22,
  textShadow: '0 1px 3px rgba(0,0,0,.8)',
};

function Banner() {
  const { api, model, now } = useParty();
  const state = model.state;
  if (!state) return null;
  return (
    <>
      <div style={BANNER} data-testid="party-vote-overlay">
        <VotePanel
          api={api}
          vote={state.vote}
          closed={model.lastClosed}
          me={state.me.userId}
          now={now}
        />
      </div>
      <div style={SPEAKERS}>
        <VoiceSpeakers />
      </div>
      <div style={FLOATS}>
        {model.floating.map((f) => (
          <span key={f.key}>{f.emoji}</span>
        ))}
      </div>
    </>
  );
}

export default function PartyOverlay(props: Readonly<Record<string, unknown>>) {
  const parsed = OverlayProps.safeParse(props);
  return parsed.success && parsed.data.immersive ? <Banner /> : null;
}
