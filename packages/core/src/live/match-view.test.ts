import { describe, expect, it } from 'vitest';
import { calculateMatch, clockWords } from './match-view';
import { baseMatch, fact, T0 } from './match.fixture';
import { isRevealed } from './reveal';

const view = (
  pdt: number | null,
  extra: Parameters<typeof baseMatch>[0] = {},
  now = T0,
  postSeenAt: number | null = null,
) => calculateMatch(baseMatch(extra), { pdt, now, postSeenAt });

describe('isRevealed', () => {
  it('shows a fact once the frame on screen has reached its image time', () => {
    expect(isRevealed({ t_image: 100, t_vu: 9e12 }, { pdt: 100, now: 0, delay: 60 })).toBe(true);
    expect(isRevealed({ t_image: 100, t_vu: 0 }, { pdt: 99, now: 9e12, delay: 60 })).toBe(false);
  });

  it('falls back to the first sighting plus the delay without a frame time or an image time', () => {
    expect(isRevealed({ t_image: 100, t_vu: 1000 }, { pdt: null, now: 61_000, delay: 60 })).toBe(
      true,
    );
    expect(isRevealed({ t_image: 100, t_vu: 1000 }, { pdt: null, now: 60_999, delay: 60 })).toBe(
      false,
    );
    expect(isRevealed({ t_image: null, t_vu: 1000 }, { pdt: 5e12, now: 61_000, delay: 60 })).toBe(
      true,
    );
    expect(isRevealed({ t_image: null, t_vu: 1000 }, { pdt: 5e12, now: 60_000, delay: 60 })).toBe(
      false,
    );
  });
});

describe('calculate', () => {
  it('shows no score before kick-off', () => {
    const v = view(T0 - 5000);

    expect([v.clock.started, v.score]).toEqual([false, { dom: 0, ext: 0 }]);
  });

  it('reveals the shot at ten minutes and not the goal that comes later', () => {
    const v = view(T0 + 604_000);

    expect([v.clock.label, v.stats[1]?.dom, v.score.dom]).toEqual(["11'", 1, 0]);
  });

  it('reveals a goal exactly at its image time and not a millisecond before', () => {
    const at = view(T0 + 2_253_000);
    const before = view(T0 + 2_252_999);

    expect([at.score, before.score.dom]).toEqual([{ dom: 1, ext: 0 }, 0]);
  });

  it('counts shots, shots on target, corners and cards from revealed facts only', () => {
    const v = view(T0 + 2_600_000);

    expect(v.stats.map((s) => [s.key, s.dom, s.ext])).toEqual([
      ['shots', 2, 0],
      ['onTarget', 2, 0],
      ['corners', 0, 1],
      ['blocked', 0, 0],
      ['yellows', 0, 1],
      ['possession', 44, 56],
    ]);
  });

  it('shows possession only from the revealed snapshot', () => {
    expect(view(T0 + 900_000).stats.some((s) => s.key === 'possession')).toBe(false);
    expect(view(T0 + 1_000_000).stats.find((s) => s.key === 'possession')).toMatchObject({
      dom: 44,
      ext: 56,
    });
  });

  it('caps the minute at 45+ in the first half and reads the last revealed fact when no period is calibrated', () => {
    const late = view(T0 + 50 * 60_000);
    const uncalibrated = view(
      null,
      {
        periodes: [{ periode: 1, s0: 0, t_image: null }],
        evenements: [fact('x', 'tir', 'dom', 100, { t_image: null, t_vu: 0, minute: "2'" })],
      },
      T0 + 61_000,
    );

    expect([late.clock.label, uncalibrated.clock.label]).toEqual(["45+'", "2'"]);
  });

  it('says finished only once the delay has passed and no fact is still hidden from this picture', () => {
    const post = { statut: 'post' as const };

    expect(view(T0 + 100_000, post, 1000, 0).clock.finished).toBe(false);
    expect(view(T0 + 100_000, post, 61_000, 0).clock.finished).toBe(false);
    expect(view(T0 + 10 * 3_600_000, post, 61_000, 0).clock.finished).toBe(true);
  });
});

describe('the feed', () => {
  it('lists goals, cards and changes newest first, only once they are revealed', () => {
    const early = view(T0 + 2_252_999);
    const late = view(T0 + 2_600_000);

    expect(early.feed.some((item) => item.kind === 'but')).toBe(false);
    expect(late.feed[0]?.kind).toBe('jaune');
    expect(late.feed.map((item) => item.kind)).toContain('but');
  });
});

describe('the words that replace the clock', () => {
  it('say not started before kick-off, finished after the whistle and nothing in between', () => {
    expect(clockWords(view(T0 - 5000).clock)).toBe('live.matchPre');
    expect(clockWords(view(T0 + 604_000).clock)).toBeNull();
    expect(clockWords(view(T0 + 10 * 3_600_000, { statut: 'post' }, 61_000, 0).clock)).toBe(
      'live.matchFinished',
    );
  });
});
