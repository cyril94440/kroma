import { describe, it } from 'vitest';
import { checkEndpoint, type Endpoint } from '../../endpoints.fixture';

const swallow = (call: Promise<unknown>) => call.catch(() => undefined);

describe('the party endpoints', () => {
  it.each<Endpoint>([
    {
      name: 'state',
      call: (c) => swallow(c.party.state()),
      method: 'GET',
      path: '/module/tv.kroma.party/state',
    },
    {
      name: 'chat',
      call: (c) => swallow(c.party.chat('salut')),
      method: 'POST',
      path: '/module/tv.kroma.party/chat',
      body: { text: 'salut' },
    },
    {
      name: 'vote',
      call: (c) => swallow(c.party.vote(true)),
      method: 'POST',
      path: '/module/tv.kroma.party/vote',
      body: { approve: true },
    },
    {
      name: 'requestSwitch',
      call: (c) => swallow(c.party.requestSwitch('p:1', 'TF1')),
      method: 'POST',
      path: '/module/tv.kroma.party/request-switch',
      body: { channelId: 'p:1', channelName: 'TF1', force: false },
    },
    {
      name: 'pollVote',
      call: (c) => swallow(c.party.pollVote('Oui')),
      method: 'POST',
      path: '/module/tv.kroma.party/poll/vote',
      body: { value: 'Oui' },
    },
    {
      name: 'floating',
      call: (c) => swallow(c.party.floating('x')),
      method: 'POST',
      path: '/module/tv.kroma.party/reaction',
      body: { emoji: 'x' },
    },
    {
      name: 'react',
      call: (c) => swallow(c.party.react('a-1', 'x')),
      method: 'POST',
      path: '/module/tv.kroma.party/react',
      body: { id: 'a-1', emoji: 'x' },
    },
    {
      name: 'ideas',
      call: (c) => swallow(c.party.ideas()),
      method: 'GET',
      path: '/module/tv.kroma.party/ideas',
    },
    {
      name: 'submitIdea',
      call: (c) => swallow(c.party.submitIdea('bug', 'no sound')),
      method: 'POST',
      path: '/module/tv.kroma.party/ideas',
      body: { genre: 'bug', text: 'no sound' },
    },
    {
      name: 'beat',
      call: (c) =>
        swallow(
          c.party.beat({ enabled: true, delay: 3, paused: false, pinned: false, blocked: false }),
        ),
      method: 'POST',
      path: '/module/tv.kroma.party/sync/beat',
      body: { enabled: true, delay: 3, paused: false, pinned: false, blocked: false },
    },
    {
      name: 'fineDone',
      call: (c) => swallow(c.party.fineDone({ ms: 12, seeks: 2, abandoned: false, solo: true })),
      method: 'POST',
      path: '/module/tv.kroma.party/sync/fine-done',
      body: { ms: 12, seeks: 2, abandoned: false, solo: true },
    },
    {
      name: 'recalibration',
      call: (c) =>
        swallow(c.party.recalibration({ gap: 2.5, target: 12, arrival: false, blocked: false })),
      method: 'POST',
      path: '/module/tv.kroma.party/sync/recalibration',
      body: { gap: 2.5, target: 12, arrival: false, blocked: false },
    },
    {
      name: 'stats',
      call: (c) =>
        swallow(
          c.party.stats({
            buffer: 4.5,
            bandwidthKbps: 0,
            stalls: 0,
            rebufferMs: 0,
            dropped: 0,
            resolution: '',
            playing: true,
            delay: 12.3,
            paused: false,
          }),
        ),
      method: 'POST',
      path: '/module/tv.kroma.party/stats',
      body: {
        buffer: 4.5,
        bandwidthKbps: 0,
        stalls: 0,
        rebufferMs: 0,
        dropped: 0,
        resolution: '',
        playing: true,
        delay: 12.3,
        paused: false,
      },
    },
  ])('$name', checkEndpoint);
});
