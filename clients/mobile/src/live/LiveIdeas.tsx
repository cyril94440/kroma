import { IDEA_GENRES, type Idea, type IdeaGenre } from '@kromatv/client/party';
import { KromaApiError } from '@kromatv/client';
import { Badge, Box, Button, Field, Text } from '@kromatv/ui/kit';
import { useState } from 'react';
import { useT } from '#mobile/lib/i18n';

const TOO_FAST = 429;

type Note = 'live.ideas.sent' | 'live.ideas.tooFast' | 'live.ideas.error';

/** The ideas box: write one, and read everyone's with the answer each got. */
export function LiveIdeas({
  ideas,
  onSubmit,
}: Readonly<{
  ideas: readonly Idea[];
  onSubmit: (genre: IdeaGenre, text: string) => Promise<unknown>;
}>) {
  const t = useT();
  const [text, setText] = useState('');
  const [genre, setGenre] = useState<IdeaGenre>('idea');
  const [note, setNote] = useState<Note | null>(null);
  const send = () => {
    const trimmed = text.trim();
    if (trimmed === '') return;
    onSubmit(genre, trimmed).then(
      () => {
        setText('');
        setNote('live.ideas.sent');
      },
      (error: unknown) => {
        const tooFast = error instanceof KromaApiError && error.status === TOO_FAST;
        setNote(tooFast ? 'live.ideas.tooFast' : 'live.ideas.error');
      },
    );
  };
  return (
    <Box gap={12}>
      <Box row wrap gap={6}>
        {IDEA_GENRES.map((entry) => (
          <Button
            key={entry}
            label={t(`live.ideas.genre.${entry}`)}
            size="sm"
            variant={genre === entry ? 'primary' : 'glass'}
            onPress={() => setGenre(entry)}
          />
        ))}
      </Box>
      <Box row gap={10} align="center">
        <Box flex>
          <Field.Root
            label={t('live.ideas.placeholder')}
            hideLabel
            value={text}
            onValueChange={setText}
          >
            <Field.Input onSubmit={send} placeholder={t('live.ideas.placeholder')} />
          </Field.Root>
        </Box>
        <Button
          icon="bulb"
          label={t('live.ideas.send')}
          variant="glass"
          disabled={text.trim() === ''}
          onPress={send}
        />
      </Box>
      {note ? (
        <Text variant="meta" color="textDim">
          {t(note)}
        </Text>
      ) : null}
      {ideas.length === 0 ? (
        <Text variant="body" color="textDim">
          {t('live.ideas.empty')}
        </Text>
      ) : null}
      {ideas.map((idea) => (
        <Box key={idea.id} gap={4}>
          <Box row gap={8} align="center">
            <Text variant="label">{idea.author.name}</Text>
            <Badge>{t(`live.ideas.status.${idea.status}`)}</Badge>
          </Box>
          <Text variant="body">{idea.text}</Text>
          {idea.reply ? (
            <Text variant="meta" color="textMuted">
              {idea.reply}
            </Text>
          ) : null}
        </Box>
      ))}
    </Box>
  );
}
