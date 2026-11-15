import { KromaApiError } from '@kromatv/client';
import { useFormat, useT } from '@kromatv/module-sdk';
import { Badge, Box, Button, Row, Section, Text } from '@kromatv/ui/kit';
import { useCallback, useEffect, useState } from 'react';
import { Entry } from './entry';
import { type PartyApi } from './party-api';
import { IDEA_GENRES, IDEA_STATUSES, type Idea } from './schemas';

interface IdeasPanelProps {
  api: PartyApi;
  admin: boolean;
  version: number;
}

function IdeaRow({ api, idea, admin }: Readonly<{ api: PartyApi; idea: Idea; admin: boolean }>) {
  const t = useT();
  const fmt = useFormat();
  return (
    <Box mb={10}>
      <Row gap={8}>
        <Text variant="label">{idea.author.name}</Text>
        <Badge>{t(`party.ideas.status.${idea.status}`)}</Badge>
        <Text variant="meta" color="textDim">
          {fmt.elapsed(idea.ts)}
        </Text>
      </Row>
      <Text variant="body">{idea.text}</Text>
      {idea.reply ? (
        <Text variant="meta" color="textMuted">
          {idea.reply}
        </Text>
      ) : null}
      {admin ? (
        <Row wrap gap={6} mt={4}>
          {IDEA_STATUSES.map((s) => (
            <Button
              key={s}
              label={t(`party.ideas.status.${s}`)}
              variant={idea.status === s ? 'primary' : 'ghost'}
              size="sm"
              onPress={() => void api.updateIdea(idea.id, s)}
            />
          ))}
        </Row>
      ) : null}
    </Box>
  );
}

export function IdeasPanel({ api, admin, version }: Readonly<IdeasPanelProps>) {
  const t = useT();
  const [ideas, setIdeas] = useState<Idea[]>([]);
  const [text, setText] = useState('');
  const [genre, setGenre] = useState<(typeof IDEA_GENRES)[number]>('idea');
  const [note, setNote] = useState<string | null>(null);

  const load = useCallback(() => {
    api
      .ideas()
      .then((r) => setIdeas(r.ideas))
      .catch(() => undefined);
  }, [api]);

  useEffect(() => {
    load();
  }, [load, version]);

  const send = async () => {
    setNote(null);
    try {
      await api.submitIdea(genre, text);
      setText('');
      setNote('party.ideas.sent');
    } catch (e) {
      setNote(
        e instanceof KromaApiError && e.status === 429 ? 'party.ideas.tooFast' : 'party.error',
      );
    }
  };

  return (
    <Section.Root>
      <Section.Header>
        <Section.Title>{t('party.ideas.title')}</Section.Title>
      </Section.Header>
      <Row wrap gap={6} mb={8}>
        {IDEA_GENRES.map((g) => (
          <Button
            key={g}
            label={t(`party.ideas.genre.${g}`)}
            variant={genre === g ? 'primary' : 'glass'}
            size="sm"
            onPress={() => setGenre(g)}
          />
        ))}
      </Row>
      <Row gap={8} mb={8}>
        <Box flex>
          <Entry
            label={t('party.ideas.placeholder')}
            value={text}
            onValueChange={setText}
            onSubmit={() => void send()}
            placeholder={t('party.ideas.placeholder')}
          />
        </Box>
        <Button
          label={t('party.ideas.send')}
          icon="bulb"
          variant="glass"
          size="sm"
          disabled={text.trim() === ''}
          onPress={() => void send()}
        />
      </Row>
      {note ? (
        <Text variant="meta" color="textDim" mb={8}>
          {t(note)}
        </Text>
      ) : null}
      {ideas.length === 0 ? (
        <Text variant="meta" color="textDim">
          {t('party.ideas.empty')}
        </Text>
      ) : (
        ideas.map((i) => <IdeaRow key={i.id} api={api} idea={i} admin={admin} />)
      )}
    </Section.Root>
  );
}
