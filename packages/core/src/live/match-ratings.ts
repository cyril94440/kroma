import type { Fact, Player } from '@kromatv/client/sport';
import type { Clock } from './reveal';

function playingSeconds(clock: Clock, revealed: Fact[]): number {
  return clock.seconds ?? revealed.at(-1)?.s ?? 0;
}

function goalWhileOnPitch(own: boolean, keeper: boolean): number {
  if (own) return 0.12;
  return keeper ? -0.45 : -0.12;
}

function pitchWindows(players: Player[], revealed: Fact[]) {
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
  return { onPitch, offPitch };
}

function goalDelta(f: Fact, p: Player, from: number, to: number): number {
  let delta = 0;
  if (f.joueur === p.id && p.equipe === f.equipe) delta += 1.3 + 0.15;
  if (f.passeur === p.id) delta += 0.8;
  if (f.s >= from && f.s < to) delta += goalWhileOnPitch(f.equipe === p.equipe, p.poste === 'G');
  return delta;
}

const PLAYER_EVENT_DELTA: Partial<Record<Fact['type'], number>> = {
  tir_cadre: 0.15,
  tir: 0.05,
  tir_contre: 0.05,
  jaune: -0.4,
  rouge: -1.5,
};

function factDelta(f: Fact, p: Player, from: number, to: number): number {
  if (f.type === 'but') return goalDelta(f, p, from, to);
  return f.joueur === p.id ? (PLAYER_EVENT_DELTA[f.type] ?? 0) : 0;
}

export function ratingsOf(
  players: Player[],
  revealed: Fact[],
  clock: Clock,
  shootout: number | null,
): Map<string, number> {
  const ratings = new Map<string, number>();
  if (!clock.started) return ratings;
  const playing = shootout === null ? revealed : revealed.filter((f) => f.s < shootout);
  const timeBonus = Math.min(0.3, (playingSeconds(clock, revealed) / 5400) * 0.3);
  const { onPitch, offPitch } = pitchWindows(players, revealed);
  for (const p of players) {
    if (!onPitch.has(p.id)) continue;
    const from = onPitch.get(p.id) ?? 0;
    const to = offPitch.get(p.id) ?? Number.POSITIVE_INFINITY;
    let rating = 6 + timeBonus;
    for (const f of playing) rating += factDelta(f, p, from, to);
    ratings.set(p.id, Math.max(4, Math.min(10, rating)));
  }
  return ratings;
}
