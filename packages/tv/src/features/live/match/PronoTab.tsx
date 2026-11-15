import type { PronoPayload } from '@kromatv/client/sport';
import { pronoStage } from '@kromatv/core';
import { useT } from '@kromatv/ui';
import { Box, Button, Callout, FocusColumn, Text } from '@kromatv/ui/kit';
import { useState } from 'react';
import { ScoreStepper } from '#tv/features/live/match/ScoreStepper';

const ROWS_MAX = 6;

function Standings({
  rows,
  me,
}: Readonly<{ rows: { user_id: string; pseudo: string; prono: string; right: string }[]; me: string }>) {
  return (
    <Box gap={6}>
      {rows.slice(0, ROWS_MAX).map((row) => (
        <Box key={row.user_id} row justify="space-between">
          <Text variant="bodyTv" color={row.user_id === me ? 'accentText' : 'text'}>
            {`${row.pseudo} ${row.prono}`}
          </Text>
          <Text variant="labelTv">{row.right}</Text>
        </Box>
      ))}
    </Box>
  );
}

export function PronoTab({
  prono,
  me,
  onVote,
}: Readonly<{
  prono: PronoPayload | null;
  me: string;
  onVote: (id: string, score: string) => void;
}>) {
  const t = useT();
  const [home, setHome] = useState(0);
  const [away, setAway] = useState(0);
  const stage = pronoStage(prono);
  if (!prono || stage === 'none') {
    return (
      <Text variant="bodyTv" color="textDim">
        {t('live.prono.none')}
      </Text>
    );
  }
  const mine = prono.votes[me];
  const results =
    prono.statut === 'final'
      ? prono.resultats.map((r) => ({ ...r, right: t('live.prono.points', { points: r.points }) }))
      : prono.classement_match.map((r) => ({ ...r, right: r.texte }));
  return (
    <Box gap={16}>
      <Text variant="headingTv">{`${prono.home} - ${prono.away}`}</Text>
      {stage === 'cancelled' ? (
        <Callout.Root tone="danger">
          <Callout.Title>{t('live.prono.cancelled')}</Callout.Title>
        </Callout.Root>
      ) : null}
      {stage === 'open' ? (
        <FocusColumn style={{ gap: 14 }}>
          <Text variant="labelTv" color="accentText">
            {t('live.prono.closesIn', { seconds: prono.ferme_dans_s })}
          </Text>
          <ScoreStepper team={prono.home_abbr || prono.home} value={home} autoFocus onChange={setHome} />
          <ScoreStepper team={prono.away_abbr || prono.away} value={away} onChange={setAway} />
          <Button label={t('live.prono.submit')} onPress={() => onVote(prono.id, `${home}-${away}`)} />
          {mine ? <Text variant="labelTv">{t('live.prono.yours', { score: mine })}</Text> : null}
          <Text variant="labelTv" color="textDim">
            {t('live.prono.voted', { count: Object.keys(prono.votes).length, total: prono.requis.length })}
          </Text>
        </FocusColumn>
      ) : (
        <Box gap={10}>
          <Text variant="headingTv">{`${prono.score.replace('-', ' - ')} ${prono.minute}`}</Text>
          <Standings rows={results} me={me} />
        </Box>
      )}
      {prono.saison.length > 0 ? (
        <Box gap={6}>
          <Text variant="overlineTv" color="accentText">
            {t('live.prono.season')}
          </Text>
          <Standings
            rows={prono.saison.map((r) => ({ ...r, prono: '', right: `${r.points} (${r.gagnes})` }))}
            me={me}
          />
        </Box>
      ) : null}
    </Box>
  );
}
