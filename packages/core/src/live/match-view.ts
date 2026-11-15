import type { Fact, MatchPayload } from '@kromatv/client/sport';
import { type Clock, type ClockContext, displayedClock, isRevealed } from './reveal';

const SHOT_KINDS = ['tir', 'tir_cadre', 'tir_contre', 'but'];
const FEED_KINDS = new Set(['but', 'remplacement', 'jaune', 'rouge']);

export interface Score {
  dom: number;
  ext: number;
}

export interface Stat {
  key: 'shots' | 'onTarget' | 'corners' | 'blocked' | 'yellows' | 'possession';
  dom: number;
  ext: number;
}

export interface FeedItem {
  id: string;
  kind: Fact['type'];
  side: Fact['equipe'];
  minute: string;
  player: string | null;
  assist: string | null;
  cameOn: string | null;
  wentOff: string | null;
  ownGoal: boolean;
}

export interface MatchView {
  score: Score;
  clock: Clock;
  stats: Stat[];
  feed: FeedItem[];
}

function statsOf(
  match: MatchPayload,
  revealed: Fact[],
  context: ClockContext & { delay: number },
): Stat[] {
  const count = (side: Fact['equipe'], kinds: string[]) =>
    revealed.filter((f) => f.equipe === side && kinds.includes(f.type)).length;
  const row = (key: Stat['key'], kinds: string[]): Stat => ({
    key,
    dom: count('dom', kinds),
    ext: count('ext', kinds),
  });
  const stats = [
    row('shots', SHOT_KINDS),
    row('onTarget', ['tir_cadre', 'but']),
    row('corners', ['corner']),
    row('blocked', ['tir_contre']),
    row('yellows', ['jaune']),
  ];
  const shown = match.possession
    .filter((p) => isRevealed(p, context))
    .sort((a, b) => (a.t_image ?? a.t_vu) - (b.t_image ?? b.t_vu));
  const last = shown.at(-1);
  if (last) stats.push({ key: 'possession', dom: Math.round(last.dom), ext: Math.round(last.ext) });
  return stats;
}

function feedOf(match: MatchPayload, revealed: Fact[]): FeedItem[] {
  const byId = new Map(match.joueurs.map((p) => [p.id, p]));
  const name = (id: string | null | undefined) => (id ? (byId.get(id)?.nom ?? null) : null);
  return revealed
    .filter((f) => FEED_KINDS.has(f.type))
    .map((f) => ({
      id: f.id,
      kind: f.type,
      side: f.equipe,
      minute: f.minute,
      player: name(f.joueur),
      assist: name(f.passeur),
      cameOn: name(f.entre),
      wentOff: name(f.sort),
      ownGoal: f.type === 'but' && f.joueur !== null && byId.get(f.joueur)?.equipe !== f.equipe,
    }))
    .reverse();
}

export type CalcContext = ClockContext;

/** The score, clock, counters and key moments of a match, computed only from the
 *  facts this viewer's own picture has already shown. */
export function calculateMatch(match: MatchPayload, context: CalcContext): MatchView {
  const reveal = { pdt: context.pdt, now: context.now, delay: match.delai_s };
  const revealed = match.evenements.filter((f) => isRevealed(f, reveal)).sort((a, b) => a.s - b.s);
  const score: Score = { dom: 0, ext: 0 };
  for (const f of revealed) if (f.type === 'but') score[f.equipe] += 1;
  return {
    score,
    clock: displayedClock(match, context, revealed),
    stats: statsOf(match, revealed, { ...context, delay: match.delai_s }),
    feed: feedOf(match, revealed),
  };
}

/** The words that stand in for the clock before kick-off and after the final
 *  whistle, or null while the clock itself is what to show. */
export function clockWords(clock: Clock): 'live.matchFinished' | 'live.matchPre' | null {
  if (clock.finished) return 'live.matchFinished';
  return clock.started ? null : 'live.matchPre';
}
