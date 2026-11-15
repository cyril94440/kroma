import { describe, expect, it } from 'vitest';
import { AsideProps, clampMove, OverlayProps } from './sync-player';

const fn = () => undefined;
const player = {
  getPositionMs: fn,
  isReady: fn,
  getBufferAheadMs: fn,
  getMoveRange: fn,
  seekToLiveEdge: fn,
  seekByMs: fn,
  setPlaybackRate: fn,
  subscribe: fn,
};

describe('AsideProps', () => {
  it('accepts what the live page hands the aside and keeps the same functions', () => {
    const parsed = AsideProps.safeParse({
      player,
      channel: { id: 'c1', name: 'TF1' },
      playing: true,
    });
    expect(parsed.success).toBe(true);
    expect(parsed.data?.player.seekByMs).toBe(fn);
  });

  it('refuses a player that lacks a method the sync needs', () => {
    const { seekByMs: _missing, ...partial } = player;
    expect(AsideProps.safeParse({ player: partial, channel: null, playing: true }).success).toBe(
      false,
    );
  });

  it('tolerates an unknown channel or playing flag rather than failing the panel', () => {
    const parsed = AsideProps.parse({ player, channel: 'x', playing: 'yes' });
    expect(parsed.channel).toBeNull();
    expect(parsed.playing).toBe(false);
  });
});

describe('OverlayProps', () => {
  it('reads whether the player is in its immersive overlay', () => {
    expect(OverlayProps.parse({ channel: null, playing: true, immersive: true }).immersive).toBe(
      true,
    );
    expect(OverlayProps.parse({ channel: null, playing: true }).immersive).toBe(false);
  });
});

describe('clampMove', () => {
  const range = { backMs: 5000, forwardMs: 2000 };

  it('leaves a move inside the range alone', () => {
    expect(clampMove(1000, range)).toBe(1000);
    expect(clampMove(-4000, range)).toBe(-4000);
  });

  it('limits a move to what the window allows, in both directions', () => {
    expect(clampMove(9000, range)).toBe(2000);
    expect(clampMove(-9000, range)).toBe(-5000);
  });

  it('forbids a forward move past the safe zone but never turns a backward one into a bigger jump', () => {
    const past = { backMs: 5000, forwardMs: -1500 };
    expect(clampMove(1000, past)).toBe(0);
    expect(clampMove(-150, past)).toBe(-150);
  });

  it('trusts the move when the window is unknown', () => {
    expect(clampMove(-7000, null)).toBe(-7000);
  });
});
