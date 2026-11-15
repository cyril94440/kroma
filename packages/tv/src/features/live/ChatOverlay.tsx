import type { ChatMessage } from '@kromatv/client/party';
import type { Floating } from '@kromatv/core';
import { Box, Text } from '@kromatv/ui/kit';
import { ChatLine } from '#tv/features/live/ChatDrawer';

const OVERLAY_MESSAGE_MS = 10_000;
const OVERLAY_LINES = 4;

export function recentMessages(
  messages: readonly ChatMessage[],
  now: number,
): readonly ChatMessage[] {
  return messages
    .filter((message) => message.text !== '' && now - message.at < OVERLAY_MESSAGE_MS)
    .slice(-OVERLAY_LINES);
}

/** What the family just said, over the picture's lower left for ten seconds,
 *  with the reactions drifting up beside it. Nothing here takes the focus. */
export function ChatOverlay({
  messages,
  floating,
  now,
}: Readonly<{ messages: readonly ChatMessage[]; floating: readonly Floating[]; now: number }>) {
  const lines = recentMessages(messages, now);
  if (lines.length === 0 && floating.length === 0) return null;
  return (
    <Box
      absolute
      left={64}
      bottom={220}
      w={720}
      gap={10}
      p={20}
      radius="lg"
      bg="black/60"
      pointerEvents="none"
      z={12}
    >
      {lines.map((message) => (
        <ChatLine key={message.id} message={message} />
      ))}
      <Box row gap={10}>
        {floating.map((entry) => (
          <Text key={entry.key} variant="headingTv">
            {entry.emoji}
          </Text>
        ))}
      </Box>
    </Box>
  );
}
