import type { ChatMessage } from '@kromatv/client/party';
import { useT } from '@kromatv/ui';
import { Box, Drawer, FocusColumn, ListRow, styles, Text, useStyles } from '@kromatv/ui/kit';
import { CardView } from '#tv/features/live/cards/CardView';

const sDefinition = styles({ column: { gap: 14 } });

const REPLIES = ['live.reply.yes', 'live.reply.no', 'live.reply.nice', 'live.reply.zap'] as const;

export function ChatLine({ message }: Readonly<{ message: ChatMessage }>) {
  return (
    <Box row gap={12}>
      {message.kind === 'system' ? null : (
        <Text variant="labelTv" color={message.kind === 'agent' ? 'accentText' : 'textMuted'}>
          {message.name}
        </Text>
      )}
      <Text variant="bodyTv" flex>
        {message.text}
      </Text>
    </Box>
  );
}

function ChatEntry({
  message,
  onWatch,
}: Readonly<{ message: ChatMessage; onWatch: (channel: string) => void }>) {
  return (
    <Box gap={10}>
      <ChatLine message={message} />
      <CardView card={message.card} onWatch={onWatch} />
    </Box>
  );
}

/** The chat as a readable side panel, newest last, with a few quick replies:
 *  a television has no keyboard, so longer messages are written on the phone. */
export function ChatDrawer({
  open,
  messages,
  activity,
  onClose,
  onReply,
  onWatch,
}: Readonly<{
  open: boolean;
  messages: readonly ChatMessage[];
  activity: string | null;
  onClose: () => void;
  onReply: (text: string) => void;
  onWatch: (channel: string) => void;
}>) {
  const s = useStyles(sDefinition);
  const t = useT();
  const shown = messages.filter((message) => message.text !== '').slice(-6);
  return (
    <Drawer.Root open={open} onClose={onClose} title={t('live.chat')} width="lg" floating>
      <Drawer.Panel>
        <Box gap={14}>
          {shown.length === 0 ? (
            <Text variant="bodyTv" color="textDim">
              {t('live.chatEmpty')}
            </Text>
          ) : (
            shown.map((message) => (
              <ChatEntry key={message.id} message={message} onWatch={onWatch} />
            ))
          )}
          {activity ? (
            <Text variant="labelTv" color="accentText">
              {activity}
            </Text>
          ) : null}
          <Text variant="labelTv" color="textDim">
            {t('live.chatHint')}
          </Text>
        </Box>
      </Drawer.Panel>
      <Drawer.Footer>
        <FocusColumn style={s.column}>
          <Text variant="overlineTv" color="accentText">
            {t('live.quickReplies')}
          </Text>
          {REPLIES.map((key, at) => (
            <ListRow.Root
              key={key}
              size="tv"
              ground="surface"
              chevron={false}
              autoFocus={at === 0}
              onPress={() => onReply(t(key))}
            >
              <ListRow.Label>{t(key)}</ListRow.Label>
            </ListRow.Root>
          ))}
        </FocusColumn>
      </Drawer.Footer>
    </Drawer.Root>
  );
}
