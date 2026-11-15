import { describe, expect, it } from 'vitest';
import { JoinAnswer, SignalBody, VoiceEvent } from './voice-schemas';

describe('VoiceEvent', () => {
  it('reads the roster event the module emits', () => {
    const event = {
      type: 'party.voice.roster',
      revision: 7,
      participants: [{ peerId: 'a-1', userId: 'mamie', name: 'Mamie', muted: false }],
    };

    expect(VoiceEvent.parse(event)).toEqual(event);
  });

  it('reads the speaking set the module emits and ignores its timestamp', () => {
    const event = { type: 'party.voice.speaking', speaking: ['a-1'], at: 5 };

    expect(VoiceEvent.parse(event)).toEqual({ type: 'party.voice.speaking', speaking: ['a-1'] });
  });

  it('reads a relayed description exactly as the module pins it', () => {
    const event = {
      to: 'b-1',
      from: 'a-1',
      fromUserId: 'mamie',
      session: 's1',
      n: 3,
      kind: 'description',
      description: { type: 'offer', sdp: 'v=0' },
      type: 'party.voice.signal',
    };

    expect(VoiceEvent.parse(event)).toEqual(event);
  });

  it('reads a relayed candidate whose optional fields are null', () => {
    const event = {
      to: 'b-1',
      from: 'a-1',
      fromUserId: 'mamie',
      session: 's1',
      n: 4,
      kind: 'candidate',
      candidate: {
        candidate: 'candidate:1',
        sdpMid: '0',
        sdpMLineIndex: 0,
        usernameFragment: null,
      },
      type: 'party.voice.signal',
    };

    expect(VoiceEvent.safeParse(event).success).toBe(true);
  });

  it('rejects an event of another module and a signal of an unknown kind', () => {
    expect(VoiceEvent.safeParse({ type: 'party.vote', seq: 1 }).success).toBe(false);
    expect(
      VoiceEvent.safeParse({ type: 'party.voice.signal', kind: 'teleport', n: 1 }).success,
    ).toBe(false);
  });
});

describe('JoinAnswer', () => {
  it('falls back to the default schedule and no ICE servers when the module omits them', () => {
    const answer = JoinAnswer.parse({ peerId: 'a-1', revision: 1, participants: [] });

    expect(answer).toMatchObject({
      beatSeconds: 10,
      expireSeconds: 30,
      iceServers: [],
      speaking: [],
    });
  });

  it('keeps an ICE server with its credentials', () => {
    const answer = JoinAnswer.parse({
      peerId: 'a-1',
      revision: 1,
      participants: [],
      iceServers: [{ urls: ['turn:t:3478'], username: 'u', credential: 'c' }],
    });

    expect(answer.iceServers[0]).toEqual({ urls: ['turn:t:3478'], username: 'u', credential: 'c' });
  });
});

describe('SignalBody', () => {
  it('requires the description of a description and refuses a rollback', () => {
    const base = { from: 'a', to: 'b', session: 's' };

    expect(SignalBody.safeParse({ ...base, kind: 'description' }).success).toBe(false);
    expect(
      SignalBody.safeParse({
        ...base,
        kind: 'description',
        description: { type: 'rollback', sdp: '' },
      }).success,
    ).toBe(false);
  });
});
