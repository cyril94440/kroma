import { useFormat, useT } from '@kromatv/module-sdk';
import { Box, Chip, Row, Text } from '@kromatv/ui/kit';
import { useState } from 'react';
import type { PartyApi } from './party-api';
import { REACTIONS, reactedBy, reactionCount } from './reactions';
import { renderMessage } from './render-message';
import type { ChatMessage } from './schemas';

function nameColor(kind: ChatMessage['kind'], mine: boolean) {
  if (kind === 'agent') return 'accentText';
  return mine ? 'accent' : 'text';
}

interface ChatLineProps {
  api: PartyApi;
  message: ChatMessage;
  me: string;
}

export function ChatLine({ api, message, me }: Readonly<ChatLineProps>) {
  const t = useT();
  const fmt = useFormat();
  const [picking, setPicking] = useState(false);
  const mine = message.userId === me;
  const system = message.kind === 'system';
  const react = (emoji: string) => {
    setPicking(false);
    void api.react(message.id, emoji).catch(() => undefined);
  };
  const shown = Object.keys(message.reactions).filter(
    (e) => reactionCount(message.reactions, e) > 0,
  );

  return (
    <Box mb={10}>
      {system ? (
        <Text variant="meta" color="textDim">
          {renderMessage(message, t)}
        </Text>
      ) : (
        <>
          <Row gap={8}>
            <Text variant="label" color={nameColor(message.kind, mine)}>
              {message.name}
            </Text>
            <Text variant="meta" color="textDim">
              {fmt.elapsed(message.at)}
            </Text>
          </Row>
          <Text variant="body">{message.text}</Text>
        </>
      )}
      {!system ? (
        <Row wrap gap={6} mt={4}>
          {shown.map((emoji) => (
            <Chip
              key={emoji}
              label={emoji}
              count={reactionCount(message.reactions, emoji)}
              active={reactedBy(message.reactions, emoji, me)}
              variant="subtle"
              onPress={() => react(emoji)}
            />
          ))}
          {picking ? (
            REACTIONS.map((emoji) => (
              <Chip key={emoji} label={emoji} variant="outline" onPress={() => react(emoji)} />
            ))
          ) : (
            <Chip
              label={t('party.react.add')}
              icon="plus"
              variant="outline"
              onPress={() => setPicking(true)}
            />
          )}
        </Row>
      ) : null}
    </Box>
  );
}
