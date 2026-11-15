// @vitest-environment jsdom
import type { ChatMessage } from '@kromatv/client/party';
import { KitProvider } from '@kromatv/ui/kit';
import { onScreen } from '@kromatv/ui/testing';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('#mobile/lib/i18n', async () => {
  const { english } = await import('#mobile/live/i18n.fixture');
  return { useT: () => english };
});

const { LiveChat } = await import('#mobile/live/LiveChat');

afterEach(cleanup);

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
  const onSend = vi.fn();
  const onWatch = vi.fn();
  render(
    onScreen(
      <KitProvider config={{ entry: { physicalKeyboard: true } }}>
        <LiveChat messages={messages} activity={activity} onSend={onSend} onWatch={onWatch} />
      </KitProvider>,
    ),
  );
  return { onSend, onWatch };
};

describe('the phone chat', () => {
  it('sends the trimmed draft and empties the field', () => {
    const { onSend } = show([]);
    const field = screen.getByLabelText('Write a message') as HTMLInputElement;

    fireEvent.change(field, { target: { value: '  Allez Rennes  ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Send' }));

    expect([onSend.mock.calls, field.value]).toEqual([[['Allez Rennes']], '']);
  });

  it('sends nothing for a blank draft', () => {
    const { onSend } = show([]);

    fireEvent.change(screen.getByLabelText('Write a message'), { target: { value: '   ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Send' }));

    expect(onSend).not.toHaveBeenCalled();
  });

  it('shows what Hermès is doing and the card under its message', () => {
    const card = { kind: 'match', home: 'PSG', away: 'OM' };
    show([message('a', { kind: 'agent', name: 'Hermès', card })], 'Hermès: checking the guide');

    expect([
      screen.queryByText('Hermès: checking the guide') !== null,
      screen.queryByText('PSG') !== null,
    ]).toEqual([true, true]);
  });
});
