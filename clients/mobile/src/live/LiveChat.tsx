import type { ChatMessage } from '@kromatv/client/party';
import { Box, Button, Field, Text } from '@kromatv/ui/kit';
import { useState } from 'react';
import { useT } from '#mobile/lib/i18n';
import { LiveCard } from '#mobile/live/cards/LiveCard';

function Line({
  message,
  onWatch,
}: Readonly<{ message: ChatMessage; onWatch: (channel: string) => void }>) {
  return (
    <Box gap={8}>
      <Box row gap={8}>
        {message.kind === 'system' ? null : (
          <Text variant="meta" color={message.kind === 'agent' ? 'accentText' : 'textMuted'}>
            {message.name}
          </Text>
        )}
        <Text variant="body" flex>
          {message.text}
        </Text>
      </Box>
      <LiveCard card={message.card} onWatch={onWatch} />
    </Box>
  );
}

/** The chat under the picture, newest last, with what Hermès is doing and a
 *  composer: a phone has a keyboard, so this is where the family writes. */
export function LiveChat({
  messages,
  activity,
  onSend,
  onWatch,
}: Readonly<{
  messages: readonly ChatMessage[];
  activity: string | null;
  onSend: (text: string) => void;
  onWatch: (channel: string) => void;
}>) {
  const t = useT();
  const [draft, setDraft] = useState('');
  const shown = messages.filter((message) => message.text !== '').slice(-30);
  const send = () => {
    const text = draft.trim();
    if (text === '') return;
    setDraft('');
    onSend(text);
  };
  return (
    <Box gap={12}>
      {shown.length === 0 ? (
        <Text variant="body" color="textDim">
          {t('live.chatEmpty')}
        </Text>
      ) : (
        shown.map((message) => <Line key={message.id} message={message} onWatch={onWatch} />)
      )}
      {activity ? (
        <Text variant="meta" color="accentText">
          {activity}
        </Text>
      ) : null}
      <Box row gap={10} align="center">
        <Box flex>
          <Field.Root
            label={t('live.chatPlaceholder')}
            hideLabel
            value={draft}
            onValueChange={setDraft}
          >
            <Field.Input onSubmit={send} placeholder={t('live.chatPlaceholder')} />
          </Field.Root>
        </Box>
        <Button label={t('live.send')} variant="primary" onPress={send} />
      </Box>
    </Box>
  );
}
