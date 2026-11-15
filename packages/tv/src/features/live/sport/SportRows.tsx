import type { SportEvent } from '@kromatv/client/sport';
import type { SportRows as Rows } from '@kromatv/core';
import { useT } from '@kromatv/ui';
import { FocusColumn, ListRow, styles, Text, useStyles } from '@kromatv/ui/kit';

const sDefinition = styles({ column: { gap: 12 } });

function eventLabel(event: SportEvent): string {
  const [home, away] = event.lignes;
  if (!home || !away) return event.compet;
  const score =
    event.masquer_score || event.statut === 'pre' ? 'vs' : `${home.score} - ${away.score}`;
  return `${home.nom} ${score} ${away.nom}`;
}

function eventHint(event: SportEvent): string {
  const when =
    event.statut === 'pre'
      ? new Date(event.debut).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
      : event.temps;
  return [when, event.chaine?.nom].filter(Boolean).join('  ·  ');
}

export function SportRows({
  rows,
  onWatch,
}: Readonly<{ rows: Rows; onWatch: (event: SportEvent) => void }>) {
  const s = useStyles(sDefinition);
  const t = useT();
  const empty = rows.live.length + rows.upcoming.length === 0;
  return (
    <FocusColumn style={s.column}>
      {rows.live.length > 0 ? (
        <Text variant="overlineTv" color="accentText">
          {t('live.sport.live')}
        </Text>
      ) : null}
      {rows.live.map((event, at) => (
        <ListRow.Root
          key={event.id}
          icon="ball-football"
          size="tv"
          ground="surface"
          chevron={false}
          autoFocus={at === 0}
          disabled={!event.chaine?.ref}
          onPress={() => onWatch(event)}
        >
          <ListRow.Label>{eventLabel(event)}</ListRow.Label>
          <ListRow.Hint>{eventHint(event)}</ListRow.Hint>
        </ListRow.Root>
      ))}
      {rows.upcoming.length > 0 ? (
        <Text variant="overlineTv" color="accentText">
          {t('live.sport.upcoming')}
        </Text>
      ) : null}
      {rows.upcoming.map((event) => (
        <ListRow.Root
          key={event.id}
          icon="clock"
          size="tv"
          ground="surface"
          chevron={false}
          disabled={!event.chaine?.ref}
          onPress={() => onWatch(event)}
        >
          <ListRow.Label>{eventLabel(event)}</ListRow.Label>
          <ListRow.Hint>{eventHint(event)}</ListRow.Hint>
        </ListRow.Root>
      ))}
      {empty ? (
        <Text variant="bodyTv" color="textDim">
          {t('live.sport.empty')}
        </Text>
      ) : null}
    </FocusColumn>
  );
}
