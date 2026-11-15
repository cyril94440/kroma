import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MicError } from './voice-mic';
import { SimNet } from './voice-peer-sim.fixture';
import { fakeMic, join, settle, SimServer } from './voice-sim.fixture';
import { VoiceSession } from './voice-session';

const MAMIE = { userId: 'mamie', name: 'Mamie' };
const PAPI = { userId: 'papi', name: 'Papi' };

function room() {
  return { server: new SimServer(), net: new SimNet() };
}

function enabled(stream: MediaStream): boolean {
  return stream.getAudioTracks()[0]?.enabled ?? false;
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('VoiceSession joining', () => {
  it('goes live with the roster the module answered and the microphone on', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);

    await mamie.session.join();

    expect(mamie.session.snapshot).toMatchObject({ phase: 'live', muted: false, failure: null });
    expect(mamie.session.snapshot.participants.map((m) => m.userId)).toEqual(['mamie']);
  });

  it('joins as a muted listener when the microphone is blocked and says why', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE, {
      mic: async () => Promise.reject(new MicError('denied')),
    });

    await mamie.session.join();

    expect(mamie.session.snapshot).toMatchObject({ phase: 'live', muted: true, failure: 'denied' });
    expect(server.members[0]?.muted).toBe(true);
  });

  it('names an unexpected microphone error as unavailable', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE, { mic: async () => Promise.reject(new Error('boom')) });

    await mamie.session.join();

    expect(mamie.session.snapshot.failure).toBe('unavailable');
  });

  it('stays out and releases the microphone when the room is full', async () => {
    const { server, net } = room();
    server.full = true;
    const mamie = join(server, net, MAMIE);

    await mamie.session.join();

    expect(mamie.session.snapshot).toMatchObject({ phase: 'idle', failure: 'full' });
    expect(mamie.mic.getTracks()[0]?.stop).toHaveBeenCalled();
  });

  it('ignores a second join while one is under way', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);

    await Promise.all([mamie.session.join(), mamie.session.join()]);

    expect(server.members).toHaveLength(1);
  });

  it('leaves again when it was disposed while the join was in flight', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);

    const joining = mamie.session.join();
    mamie.session.dispose();
    await joining;
    await settle();

    expect(server.members).toHaveLength(0);
    expect(mamie.session.snapshot.phase).toBe('idle');
  });
});

describe('VoiceSession muting and leaving', () => {
  it('mutes the track and tells the room, then unmutes', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);
    await mamie.session.join();

    await mamie.session.setMuted(true);
    const muted = [enabled(mamie.mic), server.members[0]?.muted];
    await mamie.session.setMuted(false);

    expect(muted).toEqual([false, true]);
    expect(enabled(mamie.mic)).toBe(true);
    expect(server.members[0]?.muted).toBe(false);
  });

  it('asks for the microphone again when unmuting after it was blocked', async () => {
    const { server, net } = room();
    const attempts = [new MicError('denied'), null];
    const mic = fakeMic();
    const mamie = join(server, net, MAMIE, {
      mic: async () => {
        const failure = attempts.shift();
        if (failure) throw failure;
        return mic;
      },
    });
    await mamie.session.join();

    await mamie.session.setMuted(false);

    expect(mamie.session.snapshot).toMatchObject({ muted: false, failure: null });
    expect(enabled(mic)).toBe(true);
  });

  it('stays muted and keeps the failure when the microphone is still blocked', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE, {
      mic: async () => Promise.reject(new MicError('denied')),
    });
    await mamie.session.join();

    await mamie.session.setMuted(false);

    expect(mamie.session.snapshot).toMatchObject({ muted: true, failure: 'denied' });
  });

  it('ignores a mute request when not in the room', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);

    await mamie.session.setMuted(true);

    expect(mamie.session.snapshot.muted).toBe(false);
  });

  it('leaves the room, releases the microphone and keeps showing who is there', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);
    const papi = join(server, net, PAPI);
    await mamie.session.join();
    await papi.session.join();
    await settle();

    await mamie.session.leave();
    await settle();

    expect(mamie.session.snapshot.phase).toBe('idle');
    expect(mamie.mic.getTracks()[0]?.stop).toHaveBeenCalled();
    expect(mamie.session.snapshot.participants.map((m) => m.userId)).toEqual(['papi']);
    expect(server.members.map((m) => m.userId)).toEqual(['papi']);
  });

  it('says goodbye through the beacon when the page unloads', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);
    await mamie.session.join();

    mamie.session.leaveOnUnload();

    expect(server.members).toHaveLength(0);
  });
});

describe('VoiceSession watching the room', () => {
  it('shows who is in the room before joining and follows later roster events', async () => {
    const { server, net } = room();
    const papi = join(server, net, PAPI);
    await papi.session.join();
    const mamie = join(server, net, MAMIE);

    await mamie.session.observe();

    expect(mamie.session.snapshot.phase).toBe('idle');
    expect(mamie.session.snapshot.participants.map((m) => m.userId)).toEqual(['papi']);
  });

  it('keeps the newest roster when an older one arrives late', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);
    await mamie.session.join();
    const stale = { type: 'party.voice.roster' as const, revision: 1, participants: [] };

    mamie.session.handleEvent(stale);

    expect(mamie.session.snapshot.participants).toHaveLength(1);
  });

  it('shows who is speaking from the speaking event', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);
    await mamie.session.join();

    mamie.session.handleEvent({ type: 'party.voice.speaking', speaking: ['p1.device-mamie'] });

    expect(mamie.session.snapshot.speaking.has('p1.device-mamie')).toBe(true);
  });

  it('keeps its state when a refresh cannot reach the module', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);
    await mamie.session.join();
    const api = server.api(MAMIE);
    const session = new VoiceSession({
      api: { ...api, state: async () => Promise.reject(new Error('offline')) },
      deviceId: 'device-x',
      watching: false,
      acquireMic: async () => fakeMic(),
      createPeer: () => {
        throw new Error('unused');
      },
      newSession: () => 's',
      now: Date.now,
      onChange: () => undefined,
    });

    await expect(session.refresh()).resolves.toBeUndefined();
  });
});

describe('VoiceSession speaking', () => {
  function withMeter() {
    const { server, net } = room();
    const meter: { on?: (speaking: boolean) => void; stopped: boolean } = { stopped: false };
    const mamie = join(server, net, MAMIE);
    const session = new VoiceSession({
      api: server.api(MAMIE),
      deviceId: 'device-meter',
      watching: false,
      acquireMic: async () => mamie.mic,
      watchSpeech: (_stream, onChange) => {
        meter.on = onChange;
        return () => {
          meter.stopped = true;
        };
      },
      createPeer: () => {
        throw new Error('no peers expected');
      },
      newSession: () => 's',
      now: Date.now,
      onChange: () => undefined,
    });
    server.sessions.set(MAMIE.userId, [session]);
    return { server, session, meter };
  }

  it('reports speech to the room, repeats it while it lasts and reports the end', async () => {
    const { server, session, meter } = withMeter();
    await session.join();

    meter.on?.(true);
    await settle(4_100);
    const during = server.speaking;
    meter.on?.(false);
    await settle(10);

    expect(session.snapshot.talking).toBe(false);
    expect(during).toEqual([session.snapshot.peerId]);
    expect(server.speaking).toEqual([]);
  });

  it('never reports speech while muted and clears it on muting', async () => {
    const { server, session, meter } = withMeter();
    await session.join();
    meter.on?.(true);
    await settle(10);

    await session.setMuted(true);
    meter.on?.(true);
    await settle(10);

    expect(session.snapshot.talking).toBe(false);
    expect(server.speaking).toEqual([]);
  });

  it('stops the meter when it leaves', async () => {
    const { session, meter } = withMeter();
    await session.join();

    await session.leave();

    expect(meter.stopped).toBe(true);
  });
});
