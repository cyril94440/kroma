import { Card } from '@kromatv/client/party';

/** The card a chat message carries, or null for none or one outside the closed
 *  catalogue: the message text always stays, the card is simply not drawn. */
export function parseCard(raw: unknown): Card | null {
  const parsed = Card.safeParse(raw);
  return parsed.success ? parsed.data : null;
}

export interface ChartBar {
  label: string;
  value: number;
  ratio: number;
}

/** Each point of a chart card with its height against the largest value, so a
 *  bar or a polyline is drawn the same way on every surface. */
export function chartBars(points: readonly { label: string; value: number }[]): ChartBar[] {
  const peak = Math.max(...points.map((p) => Math.abs(p.value)), 0);
  return points.map((p) => ({ ...p, ratio: peak === 0 ? 0 : Math.abs(p.value) / peak }));
}

export type RowTone = 'live' | 'win' | 'draw' | 'loss' | null;

/** How a table row of a provisional standing is marked: its outcome while a
 *  match is in play, nothing for a settled row. */
export function rowTone(
  card: { live?: { rows: { live: boolean; outcome: string | null }[] } },
  row: number,
): RowTone {
  const entry = card.live?.rows[row];
  if (!entry?.live) return null;
  if (entry.outcome === 'win' || entry.outcome === 'draw' || entry.outcome === 'loss') {
    return entry.outcome;
  }
  return 'live';
}
