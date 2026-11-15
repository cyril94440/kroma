import type { ChatMessage } from '@kromatv/client/party';
import { Box, Button, Text } from '@kromatv/ui/kit';
import { useState } from 'react';
import { TextInput } from 'react-native';
import { useT } from '#mobile/lib/i18n';

function Line({ message }: Readonly<{ message: ChatMessage }>) {
  return (
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
  );
}

/** The chat under the picture, newest last, and a composer: a phone has a
 *  keyboard, so this is where the family writes. */
export function LiveChat({
  messages,
  onSend,
}: Readonly<{ messages: readonly ChatMessage[]; onSend: (text: string) => void }>) {
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
        shown.map((message) => <Line key={message.id} message={message} />)
      )}
      <Box row gap={10} align="center">
        <Box flex>
          <TextInput
            value={draft}
            onChangeText={setDraft}
            onSubmitEditing={send}
            placeholder={t('live.chatPlaceholder')}
            returnKeyType="send"
            accessibilityLabel={t('live.chatPlaceholder')}
          />
        </Box>
        <Button label={t('live.send')} variant="primary" onPress={send} />
      </Box>
    </Box>
  );
}
