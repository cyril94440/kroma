import type { SportEvent, SportView } from '@kromatv/client/sport';
import { COMPETITIONS, sportRows } from '@kromatv/core';
import { type LiveClient, useStandings } from '@kromatv/core/react';
import { Box, Button, Chip, Text } from '@kromatv/ui/kit';
import { useMemo, useState } from 'react';
import { useT } from '#mobile/lib/i18n';

const STANDING_ROWS = 20;

function label(event: SportEvent): string {
  const [home, away] = event.lignes;
  if (!home || !away) return event.compet;
  const score =
    event.masquer_score || event.statut === 'pre' ? 'vs' : `${home.score} - ${away.score}`;
  return `${home.nom} ${score} ${away.nom}`;
}

function hint(event: SportEvent): string {
  const when =
    event.statut === 'pre'
      ? new Date(event.debut).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
      : event.temps;
  return [when, event.chaine?.nom].filter(Boolean).join('  ·  ');
}

function EventRow({
  event,
  onWatch,
}: Readonly<{ event: SportEvent; onWatch: (event: SportEvent) => void }>) {
  const t = useT();
  return (
    <Box row align="center" justify="space-between" gap={10}>
      <Box flex>
        <Text variant="body">{label(event)}</Text>
        <Text variant="meta" color="textDim">
          {hint(event)}
        </Text>
      </Box>
      {event.chaine?.ref ? (
        <Button
          icon="device-tv"
          label={t('live.sport.watch', { channel: event.chaine.nom })}
          size="sm"
          variant="glass"
          onPress={() => onWatch(event)}
        />
      ) : null}
    </Box>
  );
}

function Standings({ client }: Readonly<{ client: Pick<LiveClient, 'sport'> }>) {
  const t = useT();
  const [competition, setCompetition] = useState<string>(COMPETITIONS[0]);
  const { state, table } = useStandings(client, competition, true);
  return (
    <Box gap={10}>
      <Box row wrap gap={6}>
        {COMPETITIONS.map((name) => (
          <Chip
            key={name}
            label={name}
            active={name === competition}
            onPress={() => setCompetition(name)}
          />
        ))}
      </Box>
      {table ? (
        <Box gap={4}>
          <Text variant="meta" color="textDim">
            {t('live.standings.provisional', { count: table.matchs_comptes })}
          </Text>
          {table.lignes.slice(0, STANDING_ROWS).map((row) => (
            <Box
              key={row.club}
              row
              justify="space-between"
              bg={row.outcome ? 'accentSoft' : undefined}
            >
              <Text variant="body" flex>{`${row.rang}  ${row.club}`}</Text>
              <Text variant="label">{`${row.pts}  ${row.diff > 0 ? '+' : ''}${row.diff}  ${row.match}`}</Text>
            </Box>
          ))}
        </Box>
      ) : (
        <Text variant="body" color="textDim">
          {t(state === 'unavailable' ? 'live.standings.unavailable' : 'live.standings.loading')}
        </Text>
      )}
    </Box>
  );
}

/** What is on air and coming up in sport, then the live standings; a match's
 *  button puts its channel on the way the module decides. */
export function LiveSport({
  view,
  client,
  onWatch,
}: Readonly<{
  view: SportView | null;
  client: Pick<LiveClient, 'sport'>;
  onWatch: (event: SportEvent) => void;
}>) {
  const t = useT();
  const rows = useMemo(() => (view ? sportRows(view) : { live: [], upcoming: [] }), [view]);
  return (
    <Box gap={16}>
      {rows.live.length > 0 ? (
        <Box gap={10}>
          <Text variant="meta" color="accentText">
            {t('live.sport.live')}
          </Text>
          {rows.live.map((event) => (
            <EventRow key={event.id} event={event} onWatch={onWatch} />
          ))}
        </Box>
      ) : null}
      {rows.upcoming.length > 0 ? (
        <Box gap={10}>
          <Text variant="meta" color="accentText">
            {t('live.sport.upcoming')}
          </Text>
          {rows.upcoming.map((event) => (
            <EventRow key={event.id} event={event} onWatch={onWatch} />
          ))}
        </Box>
      ) : null}
      {rows.live.length + rows.upcoming.length === 0 ? (
        <Text variant="body" color="textDim">
          {t('live.sport.empty')}
        </Text>
      ) : null}
      <Text variant="meta" color="accentText">
        {t('live.standings')}
      </Text>
      <Standings client={client} />
    </Box>
  );
}
