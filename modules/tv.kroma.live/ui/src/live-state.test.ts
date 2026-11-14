import { describe, expect, it } from 'vitest';
import { applyEvent, badges, progressOf, shouldReloadPlayer } from './live-state';
import { Status } from './schemas';

const base = Status.parse({
  state: 'running',
  channel: { id: 'p:1', name: 'TF1' },
  provider: 'primary',
  playlist: 'a/index.m3u8',
});

describe('applying events', () => {
  it('takes the whole status from a status event', () => {
    const next = applyEvent(null, { ...base, type: 'live.status', playlist: 'b/index.m3u8' });

    expect(next?.playlist).toBe('b/index.m3u8');
    expect(next).not.toHaveProperty('type');
  });

  it('replaces only the viewers on a presence event and only the health on a health event', () => {
    const viewers = applyEvent(base, {
      type: 'live.presence',
      viewers: [{ userId: 'u1', name: 'Ann', since: 1 }],
    });
    const health = applyEvent(base, { type: 'live.health', sourceKbps: 3000, stutter: true });

    expect(viewers?.viewers.map((v) => v.name)).toEqual(['Ann']);
    expect(viewers?.playlist).toBe('a/index.m3u8');
    expect(health?.health.stutter).toBe(true);
  });

  it('leaves the status alone for an event it does not understand', () => {
    expect(applyEvent(base, { type: 'other' })).toBe(base);
    expect(applyEvent(base, 42)).toBe(base);
  });
});

describe('what the page shows', () => {
  it('reloads the player only when a new playlist arrives', () => {
    expect(shouldReloadPlayer('a', 'b')).toBe(true);
    expect(shouldReloadPlayer('a', 'a')).toBe(false);
    expect(shouldReloadPlayer('a', null)).toBe(false);
  });

  it('names the backup, black and frozen states as badges', () => {
    const status = {
      ...base,
      onBackup: true,
      health: { ...base.health, black: true, frozen: true },
    };

    expect(badges(status)).toEqual(['backup', 'black', 'frozen']);
  });

  it('measures how far through a programme it is, bounded', () => {
    expect(progressOf(150, { start: 100, end: 200 })).toBe(0.5);
    expect(progressOf(50, { start: 100, end: 200 })).toBe(0);
    expect(progressOf(500, { start: 100, end: 200 })).toBe(1);
    expect(progressOf(1, null)).toBe(0);
  });
});
