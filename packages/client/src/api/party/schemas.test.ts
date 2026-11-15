import { describe, expect, it } from 'vitest';
import {
  Card,
  ChatMessage,
  Ideas,
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

describe('wire shapes the party module pins', () => {
  it('reads a chat message and tolerates a field the server adds later', () => {
    expect(ChatMessage.parse({ ...message, futureField: 1 }).reactions.x?.[0]?.name).toBe('Bo');
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

describe('the party events', () => {
  it('reads every topic a viewer follows', () => {
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
    expect(PartyEvent.safeParse({ type: 'party.voice.roster', seq: 1 }).success).toBe(false);
  });
});

describe('ideas and cards', () => {
  it('reads the ideas box with its statuses and tolerates one the build does not know', () => {
    const ideas = Ideas.parse({
      ideas: [
        {
          id: 'i1',
          ts: 1,
          genre: 'bug',
          text: 'no sound',
          status: 'inProgress',
          reply: null,
          author: { userId: 'mamie', name: 'mamie' },
          updatedAt: 2,
        },
        {
          id: 'i2',
          ts: 3,
          genre: 'wish',
          text: 'x',
          status: 'later',
          author: { userId: 'papi', name: 'papi' },
        },
      ],
    });

    expect(ideas.ideas.map((i) => [i.genre, i.status])).toEqual([
      ['bug', 'inProgress'],
      ['other', 'received'],
    ]);
  });

  it('reads the three cards of the closed catalogue as the module validates them', () => {
    const cards = [
      {
        kind: 'table',
        title: 'Classement Ligue 1',
        columns: ['#', 'Club', 'Pts'],
        rows: [
          ['1', 'PSG', 28],
          ['2', 'Lens', 25],
        ],
        note: 'après la 11e journée',
        live: {
          rows: [
            { live: true, outcome: 'win', move: 1 },
            { live: false, outcome: null, move: 0 },
          ],
          points_col: 2,
        },
      },
      {
        kind: 'chart',
        title: 'Débit du flux (1 h)',
        style: 'line',
        unit: 'kb/s',
        points: [
          { label: '20:00', value: 3800 },
          { label: '20:10', value: 3650 },
        ],
      },
      {
        kind: 'match',
        home: 'PSG',
        away: 'Liverpool',
        competition: 'Ligue des champions',
        start: 1_793_300_400,
        score: '1 - 0',
        status: 'live',
        channel: 'Canal+ Foot',
        action: { id: 'watch', label: 'Mettre cette chaîne' },
      },
    ];

    expect(cards.map((card) => Card.safeParse(card).success)).toEqual([true, true, true]);
  });

  it('refuses a card outside the catalogue and a watch action that is not watch', () => {
    expect(Card.safeParse({ kind: 'html' }).success).toBe(false);
    expect(
      Card.safeParse({ kind: 'match', home: 'a', away: 'b', action: { id: 'delete', label: 'x' } })
        .success,
    ).toBe(false);
  });
});
