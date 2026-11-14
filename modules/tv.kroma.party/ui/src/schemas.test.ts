import { describe, expect, it } from 'vitest';
import {
  ChatMessage,
  PartyEvent,
  PartyState,
  PollView,
  SwitchOutcome,
  SyncState,
  VoteView,
} from './schemas';

const message = {
  id: 'a-1',
  userId: 'u1',
  name: 'Ana',
  kind: 'user',
  text: 'Bonsoir',
  key: null,
  params: {},
  action: null,
  card: null,
  replyTo: null,
  at: 10,
  reactions: { x: [{ userId: 'u2', name: 'Bo' }] },
};

const vote = {
  id: 1,
  kind: 'channel',
  label: 'TF1',
  note: '',
  by: { userId: 'cyril', name: 'Cyril' },
  endsAt: 30000,
  endsInS: 20,
  required: [{ userId: 'ced', name: 'Ced' }],
  votes: { ced: true },
  tally: { yes: 2, no: 0, blank: 0 },
  channelId: 'tf1',
};

const poll = {
  id: 1,
  question: 'Match ?',
  kind: 'yesno',
  options: ['Oui', 'Non'],
  votes: { mamie: 'Oui' },
  byAnswer: { Non: [], Oui: ['Mamie'] },
  total: 1,
  endsAt: 120000,
  endsInS: 110,
};

const sync = {
  target: 12,
  by: { userId: 'papi', name: 'Papi' },
  capped: true,
  members: [{ userId: 'mamie', name: 'Mamie' }],
  active: 1,
};

describe('wire shapes the server pins', () => {
  it('reads a chat message and tolerates a field the server adds later', () => {
    const parsed = ChatMessage.parse({ ...message, futureField: 1 });
    expect(parsed.reactions.x?.[0]?.name).toBe('Bo');
  });

  it('falls back instead of failing on a message of a kind it does not know', () => {
    const parsed = ChatMessage.parse({ ...message, kind: 'robot', params: null });
    expect(parsed.kind).toBe('user');
    expect(parsed.params).toEqual({});
  });

  it('reads the vote, poll and sync views exactly as the module sends them', () => {
    expect(VoteView.parse(vote).tally).toEqual({ yes: 2, no: 0, blank: 0 });
    expect(PollView.parse(poll).byAnswer.Oui).toEqual(['Mamie']);
    expect(SyncState.parse(sync).capped).toBe(true);
  });

  it('reads the switch outcome of every case of the contract', () => {
    for (const outcome of ['voteStarted', 'switchedDirectly', 'refused']) {
      expect(SwitchOutcome.parse({ outcome, voteId: null, reason: null }).outcome).toBe(outcome);
    }
  });

  it('reads the state a joining client fetches', () => {
    const state = PartyState.parse({
      seq: 5,
      now: 1,
      me: { userId: 'mamie', name: 'Mamie', admin: false },
      messages: [message],
      vote: null,
      poll: null,
      lastPoll: null,
      activity: null,
      sync: null,
      presence: [{ userId: 'mamie', name: 'Mamie', since: 1 }],
      stats: {},
      config: { voteSeconds: 30, agentName: 'Hermès' },
    });
    expect(state.messages).toHaveLength(1);
    expect(state.config.voteSeconds).toBe(30);
  });
});

describe('events', () => {
  it('reads every topic the module publishes', () => {
    const events = [
      { type: 'party.message', seq: 1, ...message },
      { type: 'party.reactions', seq: 2, id: 'a-1', reactions: {} },
      { type: 'party.reaction', seq: 3, userId: 'u', name: 'U', emoji: 'x', at: 1 },
      { type: 'party.vote', seq: 4, vote, closed: null },
      {
        type: 'party.vote',
        seq: 5,
        vote: null,
        closed: { id: 1, verdict: 'passed', yes: 2, no: 0, blank: 0 },
      },
      { type: 'party.poll', seq: 6, poll, closed: null },
      { type: 'party.activity', seq: 7, label: null },
      { type: 'party.sync', seq: 8, state: sync },
      { type: 'party.sync.golive', seq: 9 },
      { type: 'party.sync.finesync', seq: 10 },
      { type: 'party.sync.kicked', reason: 'requested', count: 0 },
      { type: 'party.ideas', seq: 11, ideas: [] },
    ];
    for (const event of events) expect(PartyEvent.safeParse(event).success).toBe(true);
  });

  it('refuses a topic it does not model', () => {
    expect(PartyEvent.safeParse({ type: 'party.unknown', seq: 1 }).success).toBe(false);
  });
});
