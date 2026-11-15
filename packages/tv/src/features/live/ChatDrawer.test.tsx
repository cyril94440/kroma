// @vitest-environment jsdom
import type { ChatMessage } from '@kromatv/client/party';
import { createInputTestHost, onScreen } from '@kromatv/ui/testing';
import { cleanup, fireEvent, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ChatDrawer } from '#tv/features/live/ChatDrawer';

const { clearPressGuard, render } = createInputTestHost();

afterEach(() => {
  cleanup();
  clearPressGuard();
});

const message = (id: string, extra: Partial<ChatMessage> = {}): ChatMessage => ({
  id,
  userId: 'u1',
  name: 'Ana',
  kind: 'user',
  text: `text ${id}`,
  key: null,
  params: {},
  action: null,
  card: null,
  replyTo: null,
  at: 1,
  reactions: {},
  ...extra,
});

const show = (messages: ChatMessage[], activity: string | null = null) => {
  const onReply = vi.fn();
  const onWatch = vi.fn();
  render(
    onScreen(
      <ChatDrawer
        open
        messages={messages}
        activity={activity}
        onClose={vi.fn()}
        onReply={onReply}
        onWatch={onWatch}
      />,
    ),
  );
  return { onReply, onWatch };
};

describe('the chat drawer', () => {
  it('sends the words of the quick reply that was pressed', async () => {
    const { onReply } = show([message('a')]);
    const reply = await screen.findByText('Great game');
    clearPressGuard();

    fireEvent.click(reply);

    expect(onReply).toHaveBeenCalledWith('Great game');
  });

  it('shows what Hermès is doing above the quick replies', () => {
    show([message('a')], 'Hermès: searching the web');

    expect(screen.getByText('Hermès: searching the web')).toBeTruthy();
  });

  it('draws the card of a message under its text and keeps the text', async () => {
    const card = {
      kind: 'match',
      home: 'PSG',
      away: 'OM',
      channel: 'Canal+',
      action: { id: 'watch', label: 'Put this channel on' },
    };
    const { onWatch } = show([message('a', { kind: 'agent', name: 'Hermès', card })]);
    const watch = await screen.findByRole('button', { name: 'Put this channel on' });
    clearPressGuard();

    fireEvent.click(watch);

    expect([screen.getByText('text a') !== null, onWatch.mock.calls]).toEqual([true, [['Canal+']]]);
  });

  it('keeps a message whose card is outside the catalogue as plain text', () => {
    show([message('a', { card: { kind: 'html' } })]);

    expect(screen.getByText('text a')).toBeTruthy();
  });
});
