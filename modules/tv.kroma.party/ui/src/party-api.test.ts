import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('@kromatv/client/accounts', () => {
  let token = 'stale';
  return {
    loadSession: () => ({ accessToken: token }),
    __refresh: () => {
      token = 'fresh';
    },
  };
});

import * as accounts from '@kromatv/client/accounts';
import { createPartyApi, PartyError } from './party-api';

const host = (get: () => Promise<never> = async () => [] as never) => ({
  api: { get, listModules: async () => [] },
});

const respond = (status: number, body: unknown) =>
  new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });

afterEach(() => vi.unstubAllGlobals());

describe('createPartyApi', () => {
  it('posts to the module mount with the bearer and a JSON body', async () => {
    const fetchMock = vi.fn(async () => respond(200, { ok: true }));
    vi.stubGlobal('fetch', fetchMock);
    const api = createPartyApi(host());

    await api.vote(true);

    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toMatch(/\/api\/module\/tv\.kroma\.party\/vote$/);
    expect(init.method).toBe('POST');
    expect((init.headers as Record<string, string>).authorization).toBe('Bearer stale');
    expect(init.body).toBe(JSON.stringify({ approve: true }));
  });

  it('refreshes the session through the host once on a 401 and retries', async () => {
    const calls: string[] = [];
    vi.stubGlobal(
      'fetch',
      vi.fn(async (_url: string, init: RequestInit) => {
        const auth = (init.headers as Record<string, string>).authorization ?? '';
        calls.push(auth);
        return auth === 'Bearer fresh'
          ? respond(200, { now: 5 })
          : respond(401, { error: 'invalid' });
      }),
    );
    const get = vi.fn(async () => {
      (accounts as unknown as { __refresh: () => void }).__refresh();
      return [] as never;
    });
    const api = createPartyApi(host(get));

    await expect(api.time()).resolves.toEqual({ now: 5 });
    expect(calls).toEqual(['Bearer stale', 'Bearer fresh']);
    expect(get).toHaveBeenCalledTimes(1);
  });

  it("turns an error answer into a PartyError carrying the server's code", async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => respond(429, { error: 'rateLimited' })),
    );
    const api = createPartyApi(host());
    const failure = await api.chat('x').catch((e: unknown) => e);
    expect(failure).toBeInstanceOf(PartyError);
    expect(failure).toMatchObject({ status: 429, code: 'rateLimited' });
  });

  it('rejects an answer that does not match the schema', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => respond(200, { now: 'soon' })),
    );
    const api = createPartyApi(host());
    await expect(api.time()).rejects.toThrow();
  });
});
