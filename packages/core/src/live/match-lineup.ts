import type { Fact, Player } from '@kromatv/client/sport';

export interface OnPitch extends Player {
  rating: number | null;
}

export function lineupAt(players: Player[], revealed: Fact[], ratings: Map<string, number>) {
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

const KEEPER_TOP = 94;
const ATTACK_TOP = 57.5;
const DEFAULT_ROWS = 5;
const PLAYERS_PER_ROW = 3;

export interface PitchSlot {
  leftPercent: number;
  topPercent: number;
}

/** Where a player stands on a pitch drawn upright, home side at the bottom and
 *  away side mirrored on top; both as percentages of the pitch. */
export function pitchSlot(side: Fact['equipe'], player: OnPitch): PitchSlot {
  const x = player.x ?? 50;
  const rows = player.lignes ?? DEFAULT_ROWS;
  const row =
    player.ligne ?? Math.min(rows - 1, Math.max(0, Math.ceil(player.place / PLAYERS_PER_ROW)));
  const depth = KEEPER_TOP - (row * (KEEPER_TOP - ATTACK_TOP)) / Math.max(1, rows - 1);
  return {
    leftPercent: 3 + (side === 'ext' ? 100 - x : x) * 0.94,
    topPercent: side === 'dom' ? depth : 100 - depth,
  };
}
