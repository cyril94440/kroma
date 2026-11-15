import { apiErrorBody } from '@kromatv/client';
import type { Clip, PronoPayload, SportEvent, SportView } from '@kromatv/client/sport';

const FALLBACK_CLIP_DELAY_MS = 180_000;
const UPCOMING_MAX = 8;
export const SCORE_MAX = 15;

export const COMPETITIONS = [
  'Ligue 1',
  'Ligue 2',
  'Premier League',
  'Liga',
  'Serie A',
  'Bundesliga',
  'Ligue des champions',
] as const;

/** A clip shows once this viewer's own picture has passed its end; without a
 *  frame time, three minutes after it was cut. */
export function clipVisible(clip: Clip, pdt: number | null, now: number): boolean {
  return pdt === null ? now >= clip.ts + FALLBACK_CLIP_DELAY_MS : pdt >= clip.fin_ms;
}

export function visibleClips(
  clips: readonly Clip[],
  pdt: number | null,
  now: number,
  matchId: string | null,
): Clip[] {
  return clips.filter((c) => (matchId === null || c.match_id === matchId) && clipVisible(c, pdt, now));
}

export interface SportRows {
  live: SportEvent[];
  upcoming: SportEvent[];
}

/** The matches on air, and the most followed ones to come in kick-off order. */
export function sportRows(view: SportView): SportRows {
  const live = view.evenements.filter((e) => e.statut === 'in');
  const upcoming = view.evenements
    .filter((e) => e.statut === 'pre')
    .sort((a, b) => b.pop - a.pop || a.debut - b.debut)
    .slice(0, UPCOMING_MAX)
    .sort((a, b) => a.debut - b.debut);
  return { live, upcoming };
}

export function clampScore(value: number): number {
  return Math.min(SCORE_MAX, Math.max(0, value));
}

export type PronoStage = 'none' | 'open' | 'playing' | 'final' | 'cancelled';

export function pronoStage(prono: PronoPayload | null): PronoStage {
  if (!prono) return 'none';
  if (prono.annule) return 'cancelled';
  if (prono.statut === 'ouvert') return 'open';
  return prono.statut === 'final' ? 'final' : 'playing';
}

export type PronoErrorKey =
  | 'live.prono.error.noOpenProno'
  | 'live.prono.error.closed'
  | 'live.prono.error.invalidScore';

const PRONO_ERRORS: Record<string, PronoErrorKey> = {
  noOpenProno: 'live.prono.error.noOpenProno',
  invalidScore: 'live.prono.error.invalidScore',
};

export function pronoErrorKey(code: string | null): PronoErrorKey {
  return (code && PRONO_ERRORS[code]) || 'live.prono.error.closed';
}

export function errorCode(error: unknown): string | null {
  return apiErrorBody(error).error ?? null;
}
