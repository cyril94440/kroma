import type { ChatMessage } from '@kromatv/client/party';
import { describe, expect, it } from 'vitest';
import { recentMessages } from '#tv/features/live/ChatOverlay';

const message = (id: string, at: number, text = id): ChatMessage => ({
  id,
  userId: 'u1',
  name: 'Ana',
  kind: 'user',
  text,
  key: null,
  params: {},
  action: null,
  card: null,
  replyTo: null,
  at,
  reactions: {},
});

describe('what the overlay shows of the chat', () => {
  it('keeps the last four messages of the past ten seconds', () => {
    const now = 100_000;
    const messages = [
      message('old', now - 11_000),
      ...['a', 'b', 'c', 'd', 'e'].map((id, i) => message(id, now - 5000 + i)),
    ];

    expect(recentMessages(messages, now).map((m) => m.id)).toEqual(['b', 'c', 'd', 'e']);
  });

  it('leaves out a message with no text to read', () => {
    expect(recentMessages([message('x', 99_000, '')], 100_000)).toEqual([]);
  });
});
