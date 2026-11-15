import { KromaApiError } from '@kromatv/client';
import { useT } from '@kromatv/module-sdk';
import { Box, Button, EmptyState, Row, Surface, Text } from '@kromatv/ui/kit';
import { useEffect, useRef, useState } from 'react';
import { ScrollView } from 'react-native';
import { ChatLine } from './chat-line';
import { Entry } from './entry';
import type { Floating } from './party-state';
import type { PartyApi } from './party-api';
import type { PartyState } from './schemas';

interface ChatPanelProps {
  api: PartyApi;
  state: PartyState;
  floating: Floating[];
}

export function ChatPanel({ api, state, floating }: Readonly<ChatPanelProps>) {
  const t = useT();
  const [text, setText] = useState('');
  const [error, setError] = useState<string | null>(null);
  const scroller = useRef<ScrollView>(null);
  const last = state.messages.at(-1)?.id;

  useEffect(() => {
    if (last) scroller.current?.scrollToEnd({ animated: false });
  }, [last]);

  const send = async () => {
    const body = text.trim();
    if (!body) return;
    setText('');
    setError(null);
    try {
      await api.chat(body);
    } catch (e) {
      setText(body);
      setError(
        e instanceof KromaApiError && e.status === 429 ? 'party.chat.rateLimited' : 'party.error',
      );
    }
  };

  return (
    <Surface elevated border="border" pad="none" px={16} py={16}>
      <Box h={420}>
        <ScrollView ref={scroller}>
          {state.messages.length === 0 ? (
            <EmptyState.Root>
              <EmptyState.Title>{t('party.chat.empty')}</EmptyState.Title>
            </EmptyState.Root>
          ) : (
            state.messages.map((m) => (
              <ChatLine key={m.id} api={api} message={m} me={state.me.userId} />
            ))
          )}
        </ScrollView>
      </Box>
      {state.activity ? (
        <Text variant="meta" color="textDim" mt={8}>
          {state.config.agentName} : {state.activity}
        </Text>
      ) : null}
      {floating.length > 0 ? (
        <Row wrap gap={8} mt={8}>
          {floating.map((f) => (
            <Text key={f.key} variant="meta" color="textMuted">
              {f.emoji} {f.name}
            </Text>
          ))}
        </Row>
      ) : null}
      {error ? (
        <Text variant="meta" color="danger" mt={8}>
          {t(error)}
        </Text>
      ) : null}
      <Row gap={8} mt={12}>
        <Box flex>
          <Entry
            label={t('party.chat.placeholder')}
            value={text}
            onValueChange={setText}
            onSubmit={() => void send()}
            placeholder={t('party.chat.placeholder')}
          />
        </Box>
        <Button
          label={t('party.chat.send')}
          icon="send"
          variant="primary"
          size="sm"
          onPress={() => void send()}
          disabled={text.trim() === ''}
        />
      </Row>
    </Surface>
  );
}
