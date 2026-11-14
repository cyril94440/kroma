import { useT } from '@kromatv/module-sdk';
import { Box, Button, Row, Section, Surface, Text } from '@kromatv/ui/kit';
import { useState } from 'react';
import { Entry } from './entry';
import type { PartyApi } from './party-api';
import type { PollView } from './schemas';

interface PollPanelProps {
  api: PartyApi;
  poll: PollView | null;
  last: PollView | null;
  me: string;
  admin: boolean;
  now: number;
}

const KINDS = ['yesno', 'choice', 'score', 'open'] as const;

function Results({ poll }: Readonly<{ poll: PollView }>) {
  return (
    <>
      {Object.entries(poll.byAnswer).map(([answer, who]) => (
        <Text key={answer} variant="meta" color="textMuted">
          {answer} : {who.length}
          {who.length ? ` (${who.join(', ')})` : ''}
        </Text>
      ))}
    </>
  );
}

function Starter({ api }: Readonly<{ api: PartyApi }>) {
  const t = useT();
  const [question, setQuestion] = useState('');
  const [kind, setKind] = useState<(typeof KINDS)[number]>('yesno');
  const [options, setOptions] = useState('');
  const start = () =>
    void api
      .pollStart(
        question,
        kind,
        options
          .split(',')
          .map((o) => o.trim())
          .filter(Boolean),
      )
      .then(() => setQuestion(''))
      .catch(() => undefined);
  return (
    <Box mt={8}>
      <Entry
        label={t('party.poll.question')}
        value={question}
        onValueChange={setQuestion}
        placeholder={t('party.poll.question')}
      />
      <Row wrap gap={6} mt={6}>
        {KINDS.map((k) => (
          <Button
            key={k}
            label={t(`party.poll.kind.${k}`)}
            variant={kind === k ? 'primary' : 'glass'}
            size="sm"
            onPress={() => setKind(k)}
          />
        ))}
      </Row>
      {kind === 'choice' ? (
        <Box mt={6}>
          <Entry
            label={t('party.poll.options')}
            value={options}
            onValueChange={setOptions}
            placeholder={t('party.poll.options')}
          />
        </Box>
      ) : null}
      <Box mt={8}>
        <Button
          label={t('party.poll.start')}
          variant="glass"
          size="sm"
          disabled={question.trim() === ''}
          onPress={start}
        />
      </Box>
    </Box>
  );
}

export function PollPanel({ api, poll, last, me, admin, now }: Readonly<PollPanelProps>) {
  const t = useT();
  const [free, setFree] = useState('');
  const vote = (value: string) => void api.pollVote(value).catch(() => undefined);
  const mine = poll?.votes[me];
  return (
    <Section.Root>
      <Section.Header>
        <Section.Title>{t('party.poll.title')}</Section.Title>
      </Section.Header>
      {poll ? (
        <Surface elevated border="border" pad="none" px={14} py={12}>
          <Text variant="label">{poll.question}</Text>
          <Text variant="meta" color="textDim">
            {t('party.vote.remaining', {
              seconds: Math.max(0, Math.round((poll.endsAt - now) / 1000)),
            })}
          </Text>
          {poll.options.length > 0 ? (
            <Row wrap gap={6} mt={8}>
              {poll.options.map((o) => (
                <Button
                  key={o}
                  label={o}
                  variant={mine === o ? 'primary' : 'glass'}
                  size="sm"
                  onPress={() => vote(o)}
                />
              ))}
            </Row>
          ) : (
            <Row gap={6} mt={8}>
              <Box flex>
                <Entry
                  label={t('party.poll.answer')}
                  value={free}
                  onValueChange={setFree}
                  onSubmit={() => vote(free)}
                  placeholder={t(
                    poll.kind === 'score' ? 'party.poll.scoreHint' : 'party.poll.answerHint',
                  )}
                />
              </Box>
              <Button
                label={t('party.poll.answer')}
                variant="glass"
                size="sm"
                onPress={() => vote(free)}
              />
            </Row>
          )}
          <Box mt={8}>
            <Results poll={poll} />
          </Box>
          {admin ? (
            <Box mt={8}>
              <Button
                label={t('party.poll.close')}
                variant="ghost"
                size="sm"
                onPress={() => void api.pollClose()}
              />
            </Box>
          ) : null}
        </Surface>
      ) : (
        <>
          {last ? (
            <Box>
              <Text variant="meta" color="textDim">
                {t('party.poll.last', { question: last.question })}
              </Text>
              <Results poll={last} />
            </Box>
          ) : (
            <Text variant="meta" color="textDim">
              {t('party.poll.none')}
            </Text>
          )}
          {admin ? <Starter api={api} /> : null}
        </>
      )}
    </Section.Root>
  );
}
