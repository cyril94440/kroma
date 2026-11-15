import type { PronoPayload } from '@kromatv/client/sport';
import { clampScore, pronoStage } from '@kromatv/core';
import { Box, Button, Callout, IconButton, Text } from '@kromatv/ui/kit';
import { useState } from 'react';
import { useT } from '#mobile/lib/i18n';

const ROWS_MAX = 6;

type Row = { user_id: string; pseudo: string; prono: string; right: string };

function Stepper({
  team,
  value,
  onChange,
}: Readonly<{ team: string; value: number; onChange: (next: number) => void }>) {
  const t = useT();
  return (
    <Box row align="center" justify="space-between" gap={12}>
      <Text variant="body" flex>
        {team}
      </Text>
      <IconButton
        icon="minus"
        label={t('live.prono.less', { team })}
        variant="glass"
        onPress={() => onChange(clampScore(value - 1))}
      />
      <Text variant="title">{value}</Text>
      <IconButton
        icon="plus"
        label={t('live.prono.more', { team })}
        variant="glass"
        onPress={() => onChange(clampScore(value + 1))}
      />
    </Box>
  );
}

function Rows({ rows, me }: Readonly<{ rows: Row[]; me: string }>) {
  return (
    <Box gap={4}>
      {rows.slice(0, ROWS_MAX).map((row) => (
        <Box key={row.user_id} row justify="space-between">
          <Text variant="body" color={row.user_id === me ? 'accentText' : 'text'}>
            {`${row.pseudo} ${row.prono}`.trim()}
          </Text>
          <Text variant="label">{row.right}</Text>
        </Box>
      ))}
    </Box>
  );
}

export function MatchProno({
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
      <Text variant="body" color="textDim">
        {t('live.prono.none')}
      </Text>
    );
  }
  const mine = prono.votes[me];
  const results: Row[] =
    prono.statut === 'final'
      ? prono.resultats.map((r) => ({ ...r, right: t('live.prono.points', { points: r.points }) }))
      : prono.classement_match.map((r) => ({ ...r, right: r.texte }));
  return (
    <Box gap={12}>
      <Text variant="title">{`${prono.home} - ${prono.away}`}</Text>
      {stage === 'cancelled' ? (
        <Callout.Root tone="danger">
          <Callout.Title>{t('live.prono.cancelled')}</Callout.Title>
        </Callout.Root>
      ) : null}
      {stage === 'open' ? (
        <Box gap={10}>
          <Text variant="meta" color="accentText">
            {t('live.prono.closesIn', { seconds: prono.ferme_dans_s })}
          </Text>
          <Stepper team={prono.home_abbr || prono.home} value={home} onChange={setHome} />
          <Stepper team={prono.away_abbr || prono.away} value={away} onChange={setAway} />
          <Button
            label={t('live.prono.submit')}
            onPress={() => onVote(prono.id, `${home}-${away}`)}
          />
          {mine ? <Text variant="meta">{t('live.prono.yours', { score: mine })}</Text> : null}
          <Text variant="meta" color="textDim">
            {t('live.prono.voted', {
              count: Object.keys(prono.votes).length,
              total: prono.requis.length,
            })}
          </Text>
        </Box>
      ) : (
        <Box gap={8}>
          <Text variant="title">{`${prono.score.replace('-', ' - ')} ${prono.minute}`}</Text>
          <Rows rows={results} me={me} />
        </Box>
      )}
      {prono.saison.length > 0 ? (
        <Box gap={4}>
          <Text variant="meta" color="textDim">
            {t('live.prono.season')}
          </Text>
          <Rows
            rows={prono.saison.map((r) => ({
              ...r,
              prono: '',
              right: `${r.points} (${r.gagnes})`,
            }))}
            me={me}
          />
        </Box>
      ) : null}
    </Box>
  );
}
