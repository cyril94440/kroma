import { describe, expect, it } from 'vitest';
import { calculateMatch } from './match-view';
import { baseMatch, fact, T0 } from './match.fixture';
import { isRevealed, minuteLabel } from './reveal';

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
  it('shows no score and no rating before kick-off', () => {
    const v = view(T0 - 5000);

    expect([v.clock.started, v.score, v.ratings.size]).toEqual([false, { dom: 0, ext: 0 }, 0]);
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

  it('weights momentum by 5-minute slices capped at twelve', () => {
    const v = view(T0 + 2_600_000);

    expect([v.momentum[7]?.dom, v.momentum[2]?.dom, v.momentum[7]?.ext]).toEqual([
      0.5,
      4 / 12,
      1.5 / 12,
    ]);
  });

  it('shows possession only from the revealed snapshot', () => {
    expect(view(T0 + 900_000).stats.some((s) => s.key === 'possession')).toBe(false);
    expect(view(T0 + 1_000_000).stats.find((s) => s.key === 'possession')).toMatchObject({
      dom: 44,
      ext: 56,
    });
  });

  it('rates the scorer, the assist and the conceding keeper with the documented weights', () => {
    const v = view(T0 + 2_300_000);
    const time = 6 + Math.min(0.3, ((v.clock.seconds ?? 0) / 5400) * 0.3);
    const later = view(T0 + 4_000_000);
    const conceded = 6 + Math.min(0.3, ((later.clock.seconds ?? 0) / 5400) * 0.3) + 0.12 - 0.45;

    expect(v.ratings.get('d9')).toBeCloseTo(time + 1.3 + 0.15 + 0.15 + 0.12, 9);
    expect(v.ratings.get('d8')).toBeCloseTo(time + 0.8 + 0.12, 9);
    expect(later.ratings.get('d1')).toBeCloseTo(conceded, 9);
    expect([...v.ratings.values()].every((r) => r >= 4 && r <= 10)).toBe(true);
  });

  it('puts the substitute in the place of the player who went off once the change is revealed', () => {
    const change = fact('s1', 'remplacement', 'dom', 3000, { entre: 'd18', sort: 'd11' });
    const hidden = view(T0 + 3_000_000, { evenements: [change] });
    const shown = view(T0 + 3_004_000, { evenements: [change] });

    expect(hidden.onPitch.dom.some((p) => p.id === 'd11')).toBe(true);
    expect(shown.onPitch.dom.find((p) => p.place === 11)?.id).toBe('d18');
    expect(shown.ratings.has('d11') && shown.ratings.has('d18')).toBe(true);
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

  it('keeps the shootout out of the momentum and the ratings', () => {
    const shootout = [
      { periode: 1, s0: 0, t_image: T0 },
      { periode: 5, s0: 7200, t_image: T0 + 9_000_000 },
    ];
    const kicks = fact('k1', 'but', 'dom', 7300, { joueur: 'd9', t_image: T0 + 100 });

    const v = view(T0 + 9_500_000, { periodes: shootout, evenements: [kicks] });

    expect(v.momentum.every((s) => s.dom === 0)).toBe(true);
    expect(minuteLabel(7300, 5)).toBe('TAB');
  });
});
