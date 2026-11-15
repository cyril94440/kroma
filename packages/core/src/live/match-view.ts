import type { Fact, MatchPayload, Player } from '@kromatv/client/sport';
import { lineupAt, type OnPitch } from './match-lineup';
import { ratingsOf } from './match-ratings';
import { type Clock, type ClockContext, displayedClock, isRevealed } from './reveal';

const SHOT_KINDS = ['tir', 'tir_cadre', 'tir_contre', 'but'];
const WEIGHT: Record<string, number> = { tir: 2, tir_contre: 2, tir_cadre: 4, but: 6, corner: 1.5 };
const SLICES = 18;
const SLICE_SECONDS = 300;
const MOMENTUM_CAP = 12;
const BEST_COUNT = 3;
const FEED_KINDS = new Set(['but', 'remplacement', 'jaune', 'rouge']);

export interface Score {
  dom: number;
  ext: number;
}

export interface MomentumSlice extends Score {
  fromMinute: number;
}

export interface BestPlayer {
  player: Player;
  rating: number;
  goals: number;
  assists: number;
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
  onPitch: { dom: OnPitch[]; ext: OnPitch[] };
  ratings: Map<string, number>;
  best: BestPlayer[];
  stats: Stat[];
  momentum: MomentumSlice[];
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

function momentumOf(playing: Fact[]): MomentumSlice[] {
  const slices = Array.from({ length: SLICES }, (_, index) => ({
    dom: 0,
    ext: 0,
    fromMinute: (index * SLICE_SECONDS) / 60,
  }));
  for (const f of playing) {
    const weight = WEIGHT[f.type];
    if (!weight) continue;
    slices[Math.min(SLICES - 1, Math.max(0, Math.floor(f.s / SLICE_SECONDS)))]![f.equipe] += weight;
  }
  return slices.map((slice) => ({
    fromMinute: slice.fromMinute,
    dom: Math.min(slice.dom, MOMENTUM_CAP) / MOMENTUM_CAP,
    ext: Math.min(slice.ext, MOMENTUM_CAP) / MOMENTUM_CAP,
  }));
}

function bestOf(ratings: Map<string, number>, players: Player[], revealed: Fact[]): BestPlayer[] {
  const byId = new Map(players.map((p) => [p.id, p]));
  return [...ratings.entries()]
    .sort((a, b) => b[1] - a[1])
    .slice(0, BEST_COUNT)
    .flatMap(([id, rating]) => {
      const player = byId.get(id);
      if (!player) return [];
      const goals = revealed.filter(
        (f) => f.type === 'but' && f.joueur === id && player.equipe === f.equipe,
      ).length;
      const assists = revealed.filter((f) => f.type === 'but' && f.passeur === id).length;
      return [{ player, rating, goals, assists }];
    });
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

/** The score, clock, line-ups, ratings, counters, momentum and key moments of a
 *  match, computed only from the facts this viewer's own picture has already shown. */
export function calculateMatch(match: MatchPayload, context: CalcContext): MatchView {
  const reveal = { pdt: context.pdt, now: context.now, delay: match.delai_s };
  const revealed = match.evenements.filter((f) => isRevealed(f, reveal)).sort((a, b) => a.s - b.s);
  const score: Score = { dom: 0, ext: 0 };
  for (const f of revealed) if (f.type === 'but') score[f.equipe] += 1;
  const clock = displayedClock(match, context, revealed);
  const shootout = match.periodes.find((p) => p.periode === 5)?.s0 ?? null;
  const playing = shootout === null ? revealed : revealed.filter((f) => f.s < shootout);
  const ratings = ratingsOf(match.joueurs, revealed, clock, shootout);
  return {
    score,
    clock,
    onPitch: lineupAt(match.joueurs, revealed, ratings),
    ratings,
    best: bestOf(ratings, match.joueurs, revealed),
    stats: statsOf(match, revealed, { ...context, delay: match.delai_s }),
    momentum: momentumOf(playing),
    feed: feedOf(match, revealed),
  };
}

/** The words that stand in for the clock before kick-off and after the final
 *  whistle, or null while the clock itself is what to show. */
export function clockWords(clock: Clock): 'live.matchFinished' | 'live.matchPre' | null {
  if (clock.finished) return 'live.matchFinished';
  return clock.started ? null : 'live.matchPre';
}
