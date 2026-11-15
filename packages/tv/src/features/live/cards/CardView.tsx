import { parseCard } from '@kromatv/core';
import { ChartCardView } from '#tv/features/live/cards/ChartCardView';
import { MatchCardView } from '#tv/features/live/cards/MatchCardView';
import { TableCardView } from '#tv/features/live/cards/TableCardView';

/** The card Hermès attached to a message, drawn from the closed catalogue; a
 *  card outside it draws nothing and the message text stays. */
export function CardView({
  card,
  onWatch,
}: Readonly<{ card: unknown; onWatch: (channel: string) => void }>) {
  const parsed = parseCard(card);
  if (!parsed) return null;
  if (parsed.kind === 'table') return <TableCardView card={parsed} />;
  if (parsed.kind === 'chart') return <ChartCardView card={parsed} />;
  return <MatchCardView card={parsed} onWatch={onWatch} />;
}
