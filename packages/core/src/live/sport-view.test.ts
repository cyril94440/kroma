import { KromaApiError } from '@kromatv/client';
import type { Clip, PronoPayload, SportEvent, SportView } from '@kromatv/client/sport';
import { describe, expect, it } from 'vitest';
import {
  clampScore,
  clipVisible,
  errorCode,
  pronoErrorKey,
  pronoStage,
  sportRows,
  visibleClips,
} from './sport-view';

const clip = (extra: Partial<Clip> = {}): Clip => ({
  id: 'c1',
  match_id: 'm1',
  dom: 'Rennes',
  ext: 'OM',
  competition: 'Ligue 1',
  minute: "37'",
  equipe: 'dom',
  buteur: 'Kebbal',
  score: '1-0',
  debut_ms: 1000,
  fin_ms: 9000,
  duree_s: 8,
  ts: 100_000,
  file: { module: 'tv.kroma.live', path: 'clips/c1.mp4' },
  ...extra,
});

const event = (id: string, extra: Partial<SportEvent> = {}): SportEvent => ({
  id,
  sport: 'foot',
  compet: '',
  statut: 'pre',
  debut: 0,
  temps: '',
  lignes: [],
  chaine: null,
  a_lecran: false,
  masquer_score: false,
  ligue1: false,
  pop: 0,
  ...extra,
});

describe('clipVisible', () => {
  it('shows a clip once the picture has passed its end', () => {
    expect([clipVisible(clip(), 8999, 0), clipVisible(clip(), 9000, 0)]).toEqual([false, true]);
  });

  it('waits three minutes after the cut without a frame time', () => {
    expect([clipVisible(clip(), null, 279_999), clipVisible(clip(), null, 280_000)]).toEqual([
      false,
      true,
    ]);
  });
});

describe('visibleClips', () => {
  it('keeps the clips of the followed match only', () => {
    const all = [clip(), clip({ id: 'c2', match_id: 'm2' })];

    expect(visibleClips(all, 9000, 0, 'm2').map((c) => c.id)).toEqual(['c2']);
    expect(visibleClips(all, 9000, 0, null)).toHaveLength(2);
  });
});

describe('sportRows', () => {
  it('lists what is on air and the eight most followed to come, in kick-off order', () => {
    const view = {
      evenements: [
        event('live', { statut: 'in' }),
        ...Array.from({ length: 10 }, (_, i) => event(`p${i}`, { pop: i, debut: 100 - i })),
      ],
    } as SportView;

    const rows = sportRows(view);

    expect(rows.live.map((e) => e.id)).toEqual(['live']);
    expect(rows.upcoming.map((e) => e.id)).toEqual([
      'p9',
      'p8',
      'p7',
      'p6',
      'p5',
      'p4',
      'p3',
      'p2',
    ]);
  });
});

describe('pronos', () => {
  const prono = (extra: Partial<PronoPayload>) => ({ statut: 'ouvert', ...extra }) as PronoPayload;

  it('names the stage of a prono', () => {
    expect([
      pronoStage(null),
      pronoStage(prono({})),
      pronoStage(prono({ statut: 'en_cours' })),
      pronoStage(prono({ statut: 'final' })),
      pronoStage(prono({ annule: true })),
    ]).toEqual(['none', 'open', 'playing', 'final', 'cancelled']);
  });

  it('clamps a score to the range the server accepts', () => {
    expect([clampScore(-1), clampScore(7), clampScore(99)]).toEqual([0, 7, 15]);
  });

  it('reads the server refusal code and falls back to closed', () => {
    const refused = new KromaApiError(409, 'x', { error: 'noOpenProno' });

    expect(pronoErrorKey(errorCode(refused))).toBe('live.prono.error.noOpenProno');
    expect(pronoErrorKey(errorCode(new Error('x')))).toBe('live.prono.error.closed');
  });
});
