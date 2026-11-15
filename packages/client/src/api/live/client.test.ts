import { describe, expect, it } from 'vitest';
import { checkEndpoint, type Endpoint } from '../../endpoints.fixture';
import { recordingClient, ticketedClient } from '../../kroma-client.fixture';

describe('the live endpoints', () => {
  it.each<Endpoint>([
    {
      name: 'status',
      call: (c) => c.live.status().catch(() => undefined),
      method: 'GET',
      path: '/module/tv.kroma.live/status',
    },
    {
      name: 'channels',
      call: (c) => c.live.channels({ q: 'tf', page: 2 }).catch(() => undefined),
      method: 'GET',
      path: '/module/tv.kroma.live/channels?q=tf&page=2',
    },
    {
      name: 'favourites',
      call: (c) => c.live.favourites().catch(() => undefined),
      method: 'GET',
      path: '/module/tv.kroma.live/favourites',
    },
    {
      name: 'beat',
      call: (c) => c.live.beat(true).catch(() => undefined),
      method: 'POST',
      path: '/module/tv.kroma.live/presence',
      body: { companion: true },
    },
    {
      name: 'switchTo',
      call: (c) => c.live.switchTo('p:9').catch(() => undefined),
      method: 'POST',
      path: '/module/tv.kroma.live/switch',
      body: { channelId: 'p:9' },
    },
  ])('$name', checkEndpoint);

  it('says goodbye with a POST that survives the page', async () => {
    const { client, calls } = recordingClient(() => ({ json: {} }));

    client.live.leave();
    await Promise.resolve();

    expect(calls.at(-1)?.method).toBe('POST');
    expect(calls.at(-1)?.path).toBe('/module/tv.kroma.live/presence/leave');
  });

  it('builds the playlist URL under the module files with the ticket', async () => {
    const client = await ticketedClient('tick.1.sig');

    const url = client.live.playlistUrl('run-1/index.m3u8');

    expect(url).toMatch(/\/api\/module-files\/tv\.kroma\.live\/run-1\/index\.m3u8\?t=/);
  });
});
