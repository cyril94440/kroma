import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Outbox } from './voice-outbox';
import type { SendResult } from './voice-peer';
import type { SignalBody } from './voice-schemas';

function hello(session: string): SignalBody {
  return { from: 'a', to: 'b', session, kind: 'hello' };
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('Outbox', () => {
  it('sends one signal at a time in the order they were pushed', async () => {
    const order: string[] = [];
    const send = vi.fn(async (body: SignalBody) => {
      order.push(`start ${body.session}`);
      await new Promise((done) => setTimeout(done, 10));
      order.push(`end ${body.session}`);
      return 'ok' as const;
    });
    const outbox = new Outbox(send, () => false);

    void outbox.push(hello('one'));
    const last = outbox.push(hello('two'));
    await vi.advanceTimersByTimeAsync(50);
    await last;

    expect(order).toEqual(['start one', 'end one', 'start two', 'end two']);
  });

  it('retries a failed request twice with growing waits and then gives up', async () => {
    const send = vi.fn(async () => 'failed' as const);
    const outbox = new Outbox(send, () => false);

    const result = outbox.push(hello('one'));
    await vi.advanceTimersByTimeAsync(5_000);

    await expect(result).resolves.toBe('failed');
    expect(send).toHaveBeenCalledTimes(3);
  });

  it('stops retrying as soon as a request goes through', async () => {
    const answers: SendResult[] = ['failed', 'ok'];
    const send = vi.fn(async () => answers.shift() ?? 'ok');
    const outbox = new Outbox(send, () => false);

    const result = outbox.push(hello('one'));
    await vi.advanceTimersByTimeAsync(600);

    await expect(result).resolves.toBe('ok');
    expect(send).toHaveBeenCalledTimes(2);
  });

  it('does not send a signal that belongs to a session already replaced', async () => {
    const send = vi.fn(async () => 'ok' as const);
    const outbox = new Outbox(send, (body) => body.session === 'old');

    const result = await outbox.push(hello('old'));

    expect(result).toBe('failed');
    expect(send).not.toHaveBeenCalled();
  });

  it('sends nothing once closed', async () => {
    const send = vi.fn(async () => 'ok' as const);
    const outbox = new Outbox(send, () => false);
    outbox.close();

    await outbox.push(hello('one'));

    expect(send).not.toHaveBeenCalled();
  });
});
