import { useT } from '@kromatv/module-sdk';
import { Badge, Box, Button, ListRow, Row, Section, Text } from '@kromatv/ui/kit';
import { useCallback, useEffect, useState } from 'react';
import type { HermesApi } from './hermes-api';
import type { PersonNote } from './schemas';

interface Memory {
  lessons: { id: number; text: string }[];
  suggestions: { id: number; kind: string; title: string; detail: string }[];
  notes: PersonNote[];
}

const EMPTY: Memory = { lessons: [], suggestions: [], notes: [] };

export function MemoryPanel({ api }: Readonly<{ api: HermesApi }>) {
  const t = useT();
  const [memory, setMemory] = useState<Memory>(EMPTY);

  const load = useCallback(() => {
    Promise.all([api.lessons(), api.suggestions(), api.notes()]).then(
      ([lessons, suggestions, notes]) =>
        setMemory({
          lessons: lessons.lessons,
          suggestions: suggestions.suggestions,
          notes: notes.notes,
        }),
      () => undefined,
    );
  }, [api]);

  useEffect(load, [load]);

  const remove = (action: Promise<unknown>) => void action.then(load);

  return (
    <>
      <Section.Root>
        <Section.Header>
          <Section.Title>{t('hermes.lessons.title')}</Section.Title>
        </Section.Header>
        {memory.lessons.length === 0 ? (
          <Text variant="meta" color="textDim">
            {t('hermes.lessons.empty')}
          </Text>
        ) : null}
        {memory.lessons.map((lesson) => (
          <ListRow.Root key={lesson.id}>
            <ListRow.Label>{lesson.text}</ListRow.Label>
            <ListRow.Trailing>
              <Button
                label={t('hermes.remove')}
                variant="ghost"
                size="sm"
                onPress={() => remove(api.removeLesson(lesson.id))}
              />
            </ListRow.Trailing>
          </ListRow.Root>
        ))}
      </Section.Root>
      <Box mt={20}>
        <Section.Root>
          <Section.Header>
            <Section.Title>{t('hermes.suggestions.title')}</Section.Title>
          </Section.Header>
          {memory.suggestions.length === 0 ? (
            <Text variant="meta" color="textDim">
              {t('hermes.suggestions.empty')}
            </Text>
          ) : null}
          {memory.suggestions.map((suggestion) => (
            <Box key={suggestion.id} mb={8}>
              <Row gap={8}>
                <Badge>{suggestion.kind}</Badge>
                <Text variant="label">{suggestion.title}</Text>
                <Button
                  label={t('hermes.remove')}
                  variant="ghost"
                  size="sm"
                  onPress={() => remove(api.removeSuggestion(suggestion.id))}
                />
              </Row>
              <Text variant="meta" color="textDim">
                {suggestion.detail}
              </Text>
            </Box>
          ))}
        </Section.Root>
      </Box>
      <Box mt={20}>
        <Section.Root>
          <Section.Header>
            <Section.Title>{t('hermes.notes.title')}</Section.Title>
          </Section.Header>
          {memory.notes.length === 0 ? (
            <Text variant="meta" color="textDim">
              {t('hermes.notes.empty')}
            </Text>
          ) : null}
          {memory.notes.map((note) => (
            <ListRow.Root key={`${note.userId}:${note.note}`}>
              <ListRow.Label>{`${note.name}: ${note.note}`}</ListRow.Label>
              <ListRow.Trailing>
                <Button
                  label={t('hermes.notes.forget')}
                  variant="ghost"
                  size="sm"
                  onPress={() => remove(api.forget(note.userId))}
                />
              </ListRow.Trailing>
            </ListRow.Root>
          ))}
        </Section.Root>
      </Box>
    </>
  );
}
