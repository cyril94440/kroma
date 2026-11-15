import type { ModuleApi } from '@kromatv/module-sdk';
import { describe, expect, it, vi } from 'vitest';
import { createPartyApi } from './party-api';

function fake() {
  const get = vi.fn(async (_path: string, schema: { parse: (v: unknown) => unknown }) =>
    schema.parse({ now: 5 }),
  );
  const post = vi.fn(
    async (_path: string, _body?: unknown, schema?: { parse: (v: unknown) => unknown }) =>
      schema?.parse({ ok: true }),
  );
  return { get, post, api: { get, post } as unknown as ModuleApi };
}

describe('createPartyApi', () => {
  it('posts a vote to the module mount with a JSON body', async () => {
    const { api, post } = fake();

    await createPartyApi(api).vote(true);

    expect(post).toHaveBeenCalledWith('/vote', { approve: true }, expect.anything());
  });

  it('reads the server time through the schema', async () => {
    const { api } = fake();

    await expect(createPartyApi(api).time()).resolves.toEqual({ now: 5 });
  });

  it('encodes a channel search into the query string', async () => {
    const { api, get } = fake();

    await createPartyApi(api)
      .channels('canal +')
      .catch(() => undefined);

    expect(get).toHaveBeenCalledWith('/channels?q=canal%20%2B', expect.anything());
  });

  it('sends the sync beat, fine result and recalibration without reading an answer', async () => {
    const { api, post } = fake();
    const party = createPartyApi(api);

    await party.beat({ enabled: true, delay: 5, paused: false, pinned: false, blocked: false });
    await party.fineDone({ ms: 20, seeks: 2, abandoned: false, solo: true });
    await party.recalibration({ gap: -3, target: 8, arrival: false, blocked: false });

    expect(post.mock.calls.map((call) => call[0])).toEqual([
      '/sync/beat',
      '/sync/fine-done',
      '/sync/recalibration',
    ]);
    expect(post.mock.calls.every((call) => call.length === 2)).toBe(true);
  });
});
