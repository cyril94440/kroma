import { describe, expect, it } from 'vitest';
import {
  adoptParty as adopt,
  applyPartyEvent as applyEvent,
  EMPTY_PARTY as EMPTY,
  expireFloating,
  HISTORY_MAX,
  type PartyModel as Model,
} from './party-state';
import type { ChatMessage, PartyEvent, PartyState } from '@kromatv/client/party';

const message = (id: string, extra: Partial<ChatMessage> = {}): ChatMessage => ({
  id,
  userId: 'u1',
  name: 'Ana',
  kind: 'user',
  text: id,
  key: null,
  params: {},
  action: null,
  card: null,
  replyTo: null,
  at: 1,
  reactions: {},
  ...extra,
});

const snapshot = (seq: number, messages: ChatMessage[] = []): PartyState => ({
  seq,
  now: 1,
  me: { userId: 'u1', name: 'Ana', admin: false },
  messages,
  vote: null,
  poll: null,
  lastPoll: null,
  activity: null,
  sync: null,
  presence: [],
  stats: {},
  config: { voteSeconds: 30, agentName: 'Hermès' },
});

const said = (seq: number, id: string): PartyEvent => ({
  type: 'party.message',
  seq,
  ...message(id),
});

const joined = (seq = 10): Model => adopt(EMPTY, snapshot(seq, [message('m0')]));

describe('applyEvent', () => {
  it('ignores an event the snapshot already contains', () => {
    const model = joined();
    expect(applyEvent(model, said(10, 'm0'), 0)).toBe(model);
    expect(applyEvent(model, said(9, 'old'), 0)).toBe(model);
  });

  it('applies consecutive events in order without flagging a gap', () => {
    let model = joined();
    model = applyEvent(model, said(11, 'm1'), 0);
    model = applyEvent(model, said(12, 'm2'), 0);
    expect(model.state?.messages.map((m) => m.id)).toEqual(['m0', 'm1', 'm2']);
    expect(model.seq).toBe(12);
    expect(model.stale).toBe(false);
  });

  it('flags a gap so the caller refetches, and still applies what arrived', () => {
    const model = applyEvent(joined(), said(14, 'm3'), 0);
    expect(model.stale).toBe(true);
    expect(model.state?.messages.at(-1)?.id).toBe('m3');
  });

  it('converges after a gap once the fresh state is adopted', () => {
    const gapped = applyEvent(joined(), said(14, 'm3'), 0);
    const healed = adopt(
      gapped,
      snapshot(14, [message('m0'), message('m1'), message('m2'), message('m3')]),
    );
    expect(healed.stale).toBe(false);
    expect(healed.seq).toBe(14);
    expect(applyEvent(healed, said(14, 'm3'), 0)).toBe(healed);
  });

  it('never duplicates a message and keeps only the last hundred', () => {
    let model = joined(0);
    for (let i = 1; i <= HISTORY_MAX + 5; i += 1) model = applyEvent(model, said(i, `m${i}`), 0);
    expect(model.state?.messages).toHaveLength(HISTORY_MAX);
    expect(model.state?.messages.at(-1)?.id).toBe(`m${HISTORY_MAX + 5}`);
  });

  it('puts reactions on the right message', () => {
    const model = applyEvent(
      joined(),
      {
        type: 'party.reactions',
        seq: 11,
        id: 'm0',
        reactions: { x: [{ userId: 'u2', name: 'Bo' }] },
      },
      0,
    );
    expect(model.state?.messages[0]?.reactions.x).toHaveLength(1);
  });

  it('follows a vote from open to closed and remembers its result', () => {
    const open = applyEvent(
      joined(),
      {
        type: 'party.vote',
        seq: 11,
        vote: {
          id: 1,
          kind: 'channel',
          label: 'TF1',
          note: '',
          by: null,
          endsAt: 1,
          endsInS: 1,
          required: [],
          votes: {},
          tally: { yes: 1, no: 0, blank: 0 },
          channelId: 'tf1',
        },
        closed: null,
      },
      5,
    );
    expect(open.state?.vote?.label).toBe('TF1');
    const closed = applyEvent(
      open,
      {
        type: 'party.vote',
        seq: 12,
        vote: null,
        closed: { id: 1, verdict: 'passed', yes: 1, no: 0, blank: 0 },
      },
      7,
    );
    expect(closed.state?.vote).toBeNull();
    expect(closed.lastClosed).toMatchObject({ verdict: 'passed', at: 7 });
  });

  it('keeps the last poll when one closes and clears the sync card when the party ends', () => {
    const poll = {
      id: 1,
      question: 'Q',
      kind: 'yesno' as const,
      options: [],
      votes: {},
      byAnswer: {},
      total: 0,
      endsAt: 1,
      endsInS: 0,
    };
    const model = applyEvent(
      joined(),
      { type: 'party.poll', seq: 11, poll: null, closed: poll },
      0,
    );
    expect(model.state?.lastPoll?.question).toBe('Q');
    const live = applyEvent(
      model,
      {
        type: 'party.sync',
        seq: 12,
        state: {
          target: 1,
          by: null,
          capped: false,
          members: [{ userId: 'a', name: 'A' }],
          active: 1,
        },
      },
      0,
    );
    expect(live.state?.sync?.target).toBe(1);
    const over = applyEvent(
      live,
      {
        type: 'party.sync',
        seq: 13,
        state: { target: 0, by: null, capped: false, members: [], active: 0 },
      },
      0,
    );
    expect(over.state?.sync).toBeNull();
  });

  it('applies an addressed event without a sequence', () => {
    const model = applyEvent(
      joined(),
      { type: 'party.sync.kicked', reason: 'requested', count: 0 },
      0,
    );
    expect(model.kicked).toBe('requested');
    expect(model.seq).toBe(10);
  });

  it('waits for the first snapshot before applying anything', () => {
    expect(applyEvent(EMPTY, said(1, 'x'), 0)).toBe(EMPTY);
  });
});

describe('floating reactions', () => {
  it('appear and then expire', () => {
    const model = applyEvent(
      joined(),
      { type: 'party.reaction', seq: 11, userId: 'u', name: 'U', emoji: 'x', at: 100 },
      100,
    );
    expect(model.floating).toHaveLength(1);
    expect(expireFloating(model, 2000, 4000).floating).toHaveLength(1);
    expect(expireFloating(model, 5000, 4000).floating).toHaveLength(0);
  });
});
