import { displayedClock, isRevealed, type Clock, type ClockContext } from './reveal';
import type { Fact, MatchPayload, Player } from '@kromatv/client/sport';

const WEIGHT: Record<string, number> = { tir: 2, tir_contre: 2, tir_cadre: 4, but: 6, corner: 1.5 };
const SHOT_KINDS = ['tir', 'tir_cadre', 'tir_contre', 'but'];
const SLICES = 18;
const SLICE_SECONDS = 300;
const MOMENTUM_CAP = 12;

export interface Score {
  dom: number;
  ext: number;
}

export interface MomentumSlice extends Score {
  fromMinute: number;
}

export interface Stat {
  key: 'shots' | 'onTarget' | 'corners' | 'blocked' | 'yellows' | 'possession';
  dom: number;
  ext: number;
}

export interface OnPitch extends Player {
  rating: number | null;
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
  best: { player: Player; rating: number; goals: number; assists: number }[];
  stats: Stat[];
  momentum: MomentumSlice[];
  feed: FeedItem[];
}

function playingSeconds(clock: Clock, revealed: Fact[]): number {
  return clock.seconds ?? revealed.at(-1)?.s ?? 0;
}

function goalWhileOnPitch(own: boolean, keeper: boolean): number {
  if (own) return 0.12;
  return keeper ? -0.45 : -0.12;
}

function ratingsOf(
  players: Player[],
  revealed: Fact[],
  clock: Clock,
  shootout: number | null,
): Map<string, number> {
  const ratings = new Map<string, number>();
  if (!clock.started) return ratings;
  const playing = shootout === null ? revealed : revealed.filter((f) => f.s < shootout);
  const timeBonus = Math.min(0.3, (playingSeconds(clock, revealed) / 5400) * 0.3);
  const onPitch = new Map<string, number>();
  const offPitch = new Map<string, number>();
  for (const p of players) if (p.titulaire && p.place >= 1 && p.place <= 11) onPitch.set(p.id, 0);
  const byId = new Map(players.map((p) => [p.id, p]));
  const places = new Map(players.filter((p) => onPitch.has(p.id)).map((p) => [p.id, p.place]));
  for (const f of revealed) {
    if (f.type === 'remplacement' && f.entre && f.sort && places.has(f.sort) && byId.has(f.entre)) {
      places.set(f.entre, places.get(f.sort) ?? 0);
      places.delete(f.sort);
      onPitch.set(f.entre, f.s);
      offPitch.set(f.sort, f.s);
    } else if (f.type === 'rouge' && f.joueur && !offPitch.has(f.joueur)) {
      offPitch.set(f.joueur, f.s);
    }
  }
  for (const p of players) {
    if (!onPitch.has(p.id)) continue;
    let rating = 6 + timeBonus;
    const from = onPitch.get(p.id) ?? 0;
    const to = offPitch.get(p.id) ?? Number.POSITIVE_INFINITY;
    for (const f of playing) {
      if (f.type === 'but') {
        if (f.joueur === p.id && p.equipe === f.equipe) rating += 1.3 + 0.15;
        if (f.passeur === p.id) rating += 0.8;
        if (f.s >= from && f.s < to)
          rating += goalWhileOnPitch(f.equipe === p.equipe, p.poste === 'G');
      } else if (f.joueur === p.id) {
        if (f.type === 'tir_cadre') rating += 0.15;
        else if (f.type === 'tir' || f.type === 'tir_contre') rating += 0.05;
        else if (f.type === 'jaune') rating -= 0.4;
        else if (f.type === 'rouge') rating -= 1.5;
      }
    }
    ratings.set(p.id, Math.max(4, Math.min(10, rating)));
  }
  return ratings;
}

function lineupAt(players: Player[], revealed: Fact[], ratings: Map<string, number>) {
  const byId = new Map(players.map((p) => [p.id, p]));
  const place = new Map<string, number>();
  const layout = new Map<string, Pick<Player, 'ligne' | 'lignes' | 'x'>>();
  for (const p of players) {
    if (p.titulaire && p.place >= 1 && p.place <= 11) {
      place.set(p.id, p.place);
      layout.set(p.id, { ligne: p.ligne, lignes: p.lignes, x: p.x });
    }
  }
  for (const f of revealed) {
    if (f.type === 'remplacement' && f.entre && f.sort && byId.has(f.entre) && place.has(f.sort)) {
      place.set(f.entre, place.get(f.sort) ?? 0);
      layout.set(f.entre, layout.get(f.sort) ?? {});
      place.delete(f.sort);
      layout.delete(f.sort);
    }
  }
  const out: { dom: OnPitch[]; ext: OnPitch[] } = { dom: [], ext: [] };
  for (const [id, current] of place) {
    const player = byId.get(id);
    if (player)
      out[player.equipe].push({
        ...player,
        ...layout.get(id),
        place: current,
        rating: ratings.get(id) ?? null,
      });
  }
  out.dom.sort((a, b) => a.place - b.place);
  out.ext.sort((a, b) => a.place - b.place);
  return out;
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
  return slices.map((s) => ({
    fromMinute: s.fromMinute,
    dom: Math.min(s.dom, MOMENTUM_CAP) / MOMENTUM_CAP,
    ext: Math.min(s.ext, MOMENTUM_CAP) / MOMENTUM_CAP,
  }));
}

export interface CalcContext {
  pdt: number | null;
  now: number;
  postSeenAt: number | null;
}

/** Everything the panel shows, computed from the facts this viewer's own picture has already shown. */
export function calculateMatch(match: MatchPayload, context: CalcContext): MatchView {
  const reveal = { pdt: context.pdt, now: context.now, delay: match.delai_s };
  const revealed = match.evenements.filter((f) => isRevealed(f, reveal)).sort((a, b) => a.s - b.s);
  const clock = displayedClock(match, context, revealed);
  const shootout = match.periodes.find((p) => p.periode === 5)?.s0 ?? null;
  const playing = shootout === null ? revealed : revealed.filter((f) => f.s < shootout);
  const score: Score = { dom: 0, ext: 0 };
  for (const f of revealed) if (f.type === 'but') score[f.equipe] += 1;
  const ratings = ratingsOf(match.joueurs, revealed, clock, shootout);
  const byId = new Map(match.joueurs.map((p) => [p.id, p]));
  const name = (id: string | null | undefined) => (id ? (byId.get(id)?.nom ?? null) : null);
  const best = [...ratings.entries()]
    .sort((a, b) => b[1] - a[1])
    .slice(0, 3)
    .flatMap(([id, rating]) => {
      const player = byId.get(id);
      if (!player) return [];
      const goals = revealed.filter(
        (f) => f.type === 'but' && f.joueur === id && player.equipe === f.equipe,
      ).length;
      return [
        {
          player,
          rating,
          goals,
          assists: revealed.filter((f) => f.type === 'but' && f.passeur === id).length,
        },
      ];
    });
  const feed: FeedItem[] = revealed
    .filter((f) => ['but', 'remplacement', 'jaune', 'rouge'].includes(f.type))
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
  return {
    score,
    clock,
    onPitch: lineupAt(match.joueurs, revealed, ratings),
    ratings,
    best,
    stats: statsOf(match, revealed, { ...context, delay: match.delai_s }),
    momentum: momentumOf(playing),
    feed,
  };
}
