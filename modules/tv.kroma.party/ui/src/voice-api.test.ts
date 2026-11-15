import { KromaApiError } from '@kromatv/client';
import type { ModuleApi } from '@kromatv/module-sdk';
import { describe, expect, it, vi } from 'vitest';
import { createVoiceApi, VoiceRequestError } from './voice-api';

function api(
  post: (path: string, body: unknown, schema: { parse: (v: unknown) => unknown }) => unknown,
) {
  const calls: Array<[string, unknown]> = [];
  const get = vi.fn(async (_path: string, schema: { parse: (v: unknown) => unknown }) =>
    schema.parse({ iceServers: [{ urls: ['stun:s'] }], revision: 2, participants: [] }),
  );
  const beacon = vi.fn();
  const fake = {
    get,
    beacon,
    post: async (path: string, body: unknown, schema: { parse: (v: unknown) => unknown }) => {
      calls.push([path, body]);
      return post(path, body, schema);
    },
  } as unknown as ModuleApi;
  return { voice: createVoiceApi(fake), calls, get, beacon };
}

const refuse = (status: number, error: string) => async () => {
  throw new KromaApiError(status, 'failed', { error });
};

describe('createVoiceApi', () => {
  it('posts each action to its own route with a JSON body', async () => {
    const { voice, calls } = api(async (_p, _b, schema) =>
      schema.parse({ ok: true, revision: 3, n: 1 }),
    );

    await voice.mute('a-1', true);
    await voice.speaking('a-1', false);
    await voice.leave('a-1');

    expect(calls).toEqual([
      ['/voice/mute', { peerId: 'a-1', muted: true }],
      ['/voice/speaking', { peerId: 'a-1', speaking: false }],
      ['/voice/leave', { peerId: 'a-1' }],
    ]);
  });

  it('reads the ICE servers and the state from the module', async () => {
    const { voice } = api(async () => undefined);

    await expect(voice.ice()).resolves.toEqual([{ urls: ['stun:s'] }]);
    await expect(voice.state()).resolves.toMatchObject({ revision: 2 });
  });

  it('sends the goodbye of a closing page as a beacon', () => {
    const { voice, beacon } = api(async () => undefined);

    voice.leaveOnUnload('a-1');

    expect(beacon).toHaveBeenCalledWith('/voice/leave', { peerId: 'a-1', quiet: true });
  });

  it.each([
    [409, 'full', 'full'],
    [429, 'rateLimited', 'rateLimited'],
    [500, 'internal', 'failed'],
  ])('turns a %s on join into the %s failure', async (status, code, failure) => {
    const { voice } = api(refuse(status, code));

    const error = await voice
      .join({ deviceId: 'd', muted: false, watching: false })
      .catch((e: unknown) => e);

    expect(error).toBeInstanceOf(VoiceRequestError);
    expect((error as VoiceRequestError).failure).toBe(failure);
  });

  it('answers a heartbeat with the revision, or a rejoin when the server forgot the peer', async () => {
    const ok = api(async (_p, _b, schema) => schema.parse({ ok: true, revision: 9 }));
    const forgotten = api(refuse(404, 'notJoined'));
    const broken = api(refuse(500, 'internal'));

    await expect(ok.voice.beat('a-1')).resolves.toBe(9);
    await expect(forgotten.voice.beat('a-1')).resolves.toBe('rejoin');
    await expect(broken.voice.beat('a-1')).rejects.toBeInstanceOf(VoiceRequestError);
  });

  it.each([
    [404, 'peerGone', 'gone'],
    [404, 'notJoined', 'rejoin'],
    [429, 'rateLimited', 'failed'],
    [413, 'tooLarge', 'failed'],
  ])('maps a %s %s on a signal to %s', async (status, code, result) => {
    const { voice } = api(refuse(status, code));

    const sent = await voice.signal({ from: 'a', to: 'b', session: 's', kind: 'hello' });

    expect(sent).toBe(result);
  });

  it('answers ok when the signal was relayed and failed when the network broke', async () => {
    const ok = api(async (_p, _b, schema) => schema.parse({ ok: true, n: 1 }));
    const down = api(async () => {
      throw new TypeError('offline');
    });
    const body = { from: 'a', to: 'b', session: 's', kind: 'hello' } as const;

    await expect(ok.voice.signal(body)).resolves.toBe('ok');
    await expect(down.voice.signal(body)).resolves.toBe('failed');
  });
});
