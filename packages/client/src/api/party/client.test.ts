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
  ])('$name', checkEndpoint);
});
