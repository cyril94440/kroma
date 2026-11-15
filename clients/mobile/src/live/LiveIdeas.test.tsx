// @vitest-environment jsdom
import { KromaApiError } from '@kromatv/client';
import type { Idea } from '@kromatv/client/party';
import { KitProvider } from '@kromatv/ui/kit';
import { onScreen } from '@kromatv/ui/testing';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('#mobile/lib/i18n', async () => {
  const { english } = await import('#mobile/live/i18n.fixture');
  return { useT: () => english };
});

const { LiveIdeas } = await import('#mobile/live/LiveIdeas');

afterEach(cleanup);

const idea: Idea = {
  id: 'i1',
  ts: 1,
  genre: 'bug',
  text: 'No sound',
  status: 'inProgress',
  reply: 'On it',
  author: { userId: 'u', name: 'Mamie' },
  updatedAt: 2,
};

const show = (onSubmit: (genre: string, text: string) => Promise<unknown>, ideas: Idea[] = []) =>
  render(
    onScreen(
      <KitProvider config={{ entry: { physicalKeyboard: true } }}>
        <LiveIdeas ideas={ideas} onSubmit={onSubmit as never} />
      </KitProvider>,
    ),
  );

const write = (text: string) =>
  fireEvent.change(screen.getByLabelText('A suggestion, a bug, a remark'), {
    target: { value: text },
  });
const send = () => act(async () => fireEvent.click(screen.getByRole('button', { name: 'Send' })));

describe('the phone ideas box', () => {
  it('sends the chosen genre with the text and thanks the sender', async () => {
    const onSubmit = vi.fn(async () => ({ id: 'i2' }));
    show(onSubmit);

    fireEvent.click(screen.getByRole('button', { name: 'Bug' }));
    write('  No sound  ');
    await send();

    expect([onSubmit.mock.calls, screen.queryByText('Thanks, it was received') !== null]).toEqual([
      [['bug', 'No sound']],
      true,
    ]);
  });

  it('asks to slow down when the server says one idea every ten seconds', async () => {
    show(vi.fn(async () => Promise.reject(new KromaApiError(429, 'slow'))));

    write('Again');
    await send();

    expect(screen.getByText('Easy: one idea every 10 seconds')).toBeTruthy();
  });

  it('keeps the text and says it failed on any other error', async () => {
    show(vi.fn(async () => Promise.reject(new Error('down'))));

    write('Again');
    await send();

    expect(screen.getByText('The idea could not be sent')).toBeTruthy();
  });

  it('lists every idea with its status and answer', () => {
    show(vi.fn(), [idea]);

    expect(
      ['Mamie', 'In progress', 'No sound', 'On it'].map((t) => screen.queryByText(t) !== null),
    ).toEqual([true, true, true, true]);
  });
});
