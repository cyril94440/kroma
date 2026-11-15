import { useT } from '@kromatv/module-sdk';
import { Box, Chip, Row, Text } from '@kromatv/ui/kit';
import { useEffect, useState } from 'react';
import type { StandingsReply } from './schemas';
import type { SportApi } from './sport-api';

const COMPETITIONS = [
  'Ligue 1',
  'Ligue 2',
  'Premier League',
  'Liga',
  'Serie A',
  'Bundesliga',
  'Ligue des champions',
];
const POLL_MS = 2500;

export function StandingsPanel({ api }: Readonly<{ api: SportApi }>) {
  const t = useT();
  const [competition, setCompetition] = useState(COMPETITIONS[0] ?? 'Ligue 1');
  const [replies, setReplies] = useState<Record<string, StandingsReply>>({});

  useEffect(() => {
    let alive = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const read = () => {
      api.standings(competition).then(
        (next) => {
          if (!alive) return;
          setReplies((all) => ({ ...all, [competition]: next }));
          if (next.pending) timer = setTimeout(read, POLL_MS);
        },
        () => undefined,
      );
    };
    read();
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [api, competition]);

  const reply = replies[competition];
  const table = reply?.standings;
  const missing =
    reply && !reply.pending ? 'sport.standings.unavailable' : 'sport.standings.loading';
  return (
    <Box data-testid="sport-standings">
      <Row wrap gap={6} mb={8}>
        {COMPETITIONS.map((name) => (
          <Chip
            key={name}
            label={name}
            active={name === competition}
            onPress={() => setCompetition(name)}
          />
        ))}
      </Row>
      {table ? (
        <>
          <Text variant="meta" color="textDim">
            {t('sport.standings.provisional', { count: table.matchs_comptes })}
          </Text>
          {table.lignes.map((row) => (
            <Row key={row.club} between mb={2} bg={row.outcome ? 'accentSoft' : undefined}>
              <Text variant="body">{`${row.rang}  ${row.club}`}</Text>
              <Text variant="label">{`${row.pts}  ${row.diff > 0 ? '+' : ''}${row.diff}  ${row.match}`}</Text>
            </Row>
          ))}
        </>
      ) : (
        <Text variant="body" color="textDim">
          {t(missing)}
        </Text>
      )}
    </Box>
  );
}
