import { describe, expect, it } from 'vitest';
import { clipUrl, clipVisible } from './clip-url';
import type { Clip } from './schemas';

const clip: Clip = {
  id: 'g1',
  match_id: 'm',
  dom: 'A',
  ext: 'B',
  competition: 'L1',
  minute: "9'",
  equipe: 'dom',
  buteur: 'X',
  score: '1-0',
  debut_ms: 1000,
  fin_ms: 2000,
  duree_s: 110,
  ts: 5000,
  file: { module: 'tv.kroma.live', path: 'clips/g1.mp4' },
};

describe('clipUrl', () => {
  it('asks the host for the file in the folder of the module that published it', () => {
    const fileUrl = (path: string, moduleId?: string) =>
      `https://h/api/module-files/${moduleId ?? 'own'}/${path}?t=abc`;

    expect(clipUrl(fileUrl, clip.file)).toBe(
      'https://h/api/module-files/tv.kroma.live/clips/g1.mp4?t=abc',
    );
  });
});

describe('clipVisible', () => {
  it('waits for the frame on screen to pass the end of the clip', () => {
    expect([clipVisible(clip, 1999, 9e12), clipVisible(clip, 2000, 0)]).toEqual([false, true]);
  });

  it('falls back to three minutes after the cut without a frame time', () => {
    expect([
      clipVisible(clip, null, 5000 + 179_999),
      clipVisible(clip, null, 5000 + 180_000),
    ]).toEqual([false, true]);
  });
});
