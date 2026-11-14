import { useT } from '@kromatv/module-sdk';
import { Button, Callout, Row, Text } from '@kromatv/ui/kit';
import { useState } from 'react';
import type { PartyApi } from './party-api';
import type { VoteClosed, VoteView } from './schemas';

interface VotePanelProps {
  api: PartyApi;
  vote: VoteView | null;
  closed: (VoteClosed & { at: number }) | null;
  me: string;
  now: number;
}

const CLOSED_SHOWN_MS = 15_000;
const TONE = {
  passed: 'success',
  refused: 'accent',
  impossible: 'accent',
  cancelled: 'neutral',
} as const;

export function VotePanel({ api, vote, closed, me, now }: Readonly<VotePanelProps>) {
  const t = useT();
  const [sent, setSent] = useState<boolean | null>(null);
  if (!vote) {
    if (!closed || now - closed.at > CLOSED_SHOWN_MS) return null;
    return (
      <Callout.Root size="sm" tone={TONE[closed.verdict]} icon="check">
        <Callout.Title>{t(`party.vote.result.${closed.verdict}`)}</Callout.Title>
        <Callout.Detail>{t('party.vote.tally', { yes: closed.yes, no: closed.no })}</Callout.Detail>
      </Callout.Root>
    );
  }
  const remaining = Math.max(0, Math.round((vote.endsAt - now) / 1000));
  const mine = vote.by?.userId === me;
  const label = vote.kind === 'provider' ? t(`party.vote.provider.${vote.label}`) : vote.label;
  const answer = (approve: boolean) => {
    setSent(approve);
    void api.vote(approve).catch(() => setSent(null));
  };
  const known = vote.votes[me];

  return (
    <Callout.Root size="sm" tone="accent" icon="broadcast">
      <Callout.Title>
        {vote.by
          ? t('party.vote.asked', { name: vote.by.name, label })
          : t('party.vote.proposed', { label })}
      </Callout.Title>
      <Callout.Detail>
        {t('party.vote.tally.open', {
          yes: vote.tally.yes,
          no: vote.tally.no,
          blank: vote.tally.blank,
        })}
        {' · '}
        {t('party.vote.remaining', { seconds: remaining })}
      </Callout.Detail>
      {mine ? (
        <Text variant="meta" color="textDim" mt={6}>
          {t('party.vote.yours')}
        </Text>
      ) : (
        <Row gap={8} mt={8}>
          <Button
            label={t('party.vote.for')}
            icon="thumb-up"
            variant={(sent ?? known) === true ? 'primary' : 'glass'}
            size="sm"
            onPress={() => answer(true)}
          />
          <Button
            label={t('party.vote.against')}
            icon="thumb-down"
            variant={(sent ?? known) === false ? 'danger' : 'glass'}
            size="sm"
            onPress={() => answer(false)}
          />
        </Row>
      )}
    </Callout.Root>
  );
}
