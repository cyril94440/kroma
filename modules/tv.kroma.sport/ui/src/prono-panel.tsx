import { useT } from '@kromatv/module-sdk';
import { Box, Button, Callout, Row, Text } from '@kromatv/ui/kit';
import { useState } from 'react';
import type { PronoPayload } from './schemas';
import type { SportApi } from './sport-api';

const SCORE_MAX = 15;

interface Stepper {
  label: string;
  value: number;
  onChange: (next: number) => void;
}

function ScoreStepper({ label, value, onChange }: Readonly<Stepper>) {
  return (
    <Row gap={8} align="center">
      <Text variant="body">{label}</Text>
      <Button
        label="-"
        size="sm"
        variant="glass"
        onPress={() => onChange(Math.max(0, value - 1))}
      />
      <Text variant="title">{value}</Text>
      <Button
        label="+"
        size="sm"
        variant="glass"
        onPress={() => onChange(Math.min(SCORE_MAX, value + 1))}
      />
    </Row>
  );
}

interface PronoPanelProps {
  prono: PronoPayload | null;
  me: string;
  api: SportApi;
  season: PronoPayload['saison'];
}

export function PronoPanel({ prono, me, api, season }: Readonly<PronoPanelProps>) {
  const t = useT();
  const [home, setHome] = useState(0);
  const [away, setAway] = useState(0);
  const [message, setMessage] = useState<string | null>(null);
  if (!prono)
    return (
      <Text variant="body" color="textDim">
        {t('sport.prono.none')}
      </Text>
    );
  const mine = prono.votes[me];
  const submit = () => {
    api.vote(prono.id, `${home}-${away}`).then(
      () => setMessage(t('sport.prono.saved')),
      (error: { code?: string }) => setMessage(t(`sport.prono.error.${error.code ?? 'closed'}`)),
    );
  };
  return (
    <Box data-testid="sport-prono">
      <Text variant="title">{`${prono.home} – ${prono.away}`}</Text>
      <Text variant="meta" color="textDim">
        {prono.competition}
      </Text>
      {prono.annule ? (
        <Callout.Root tone="danger">
          <Callout.Title>{t('sport.prono.cancelled')}</Callout.Title>
        </Callout.Root>
      ) : null}
      {prono.statut === 'ouvert' ? (
        <Box mt={12} gap={8}>
          <Text variant="meta" color="accentText">
            {t('sport.prono.closesIn', { seconds: prono.ferme_dans_s })}
          </Text>
          <ScoreStepper label={prono.home_abbr || prono.home} value={home} onChange={setHome} />
          <ScoreStepper label={prono.away_abbr || prono.away} value={away} onChange={setAway} />
          <Button label={t('sport.prono.submit')} onPress={submit} />
          {mine ? <Text variant="meta">{t('sport.prono.yours', { score: mine })}</Text> : null}
          {message ? (
            <Text variant="meta" color="textDim">
              {message}
            </Text>
          ) : null}
          <Text variant="meta" color="textDim">
            {t('sport.prono.voted', {
              count: Object.keys(prono.votes).length,
              total: prono.requis.length,
            })}
          </Text>
        </Box>
      ) : (
        <Box mt={12} gap={4}>
          <Text variant="title">{`${prono.score.replace('-', ' – ')} ${prono.minute}`}</Text>
          {(prono.statut === 'final' ? prono.resultats : prono.classement_match).map((row) => (
            <Row key={row.user_id} between>
              <Text
                variant="body"
                color={row.user_id === me ? 'accent' : 'text'}
              >{`${row.pseudo} ${row.prono}`}</Text>
              <Text variant="label">{'points' in row ? `+${row.points}` : row.texte}</Text>
            </Row>
          ))}
        </Box>
      )}
      {season.length > 0 ? (
        <Box mt={16}>
          <Text variant="meta" color="textDim">
            {t('sport.prono.season')}
          </Text>
          {season.map((row) => (
            <Row key={row.user_id} between>
              <Text variant="body">{row.pseudo}</Text>
              <Text variant="label">{`${row.points} (${row.gagnes})`}</Text>
            </Row>
          ))}
        </Box>
      ) : null}
    </Box>
  );
}
