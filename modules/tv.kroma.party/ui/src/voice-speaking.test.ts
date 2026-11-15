import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fakeMic, settle, SimServer } from './voice-sim.fixture';
import { VoiceSession } from './voice-session';
import { watchSpeech, type MeterAudio } from './voice-meter';

const MAMIE = { userId: 'mamie', name: 'Mamie' };
const PAPI = { userId: 'papi', name: 'Papi' };

function loudnessMeter(level: { value: number }): MeterAudio {
  const analyser = {
    fftSize: 0,
    getByteTimeDomainData: (samples: Uint8Array) =>
      samples.fill(Math.round(128 + level.value * 127)),
  };
  const source = { connect: vi.fn(), disconnect: vi.fn() };
  const context = {
    resume: vi.fn(async () => undefined),
    close: vi.fn(async () => undefined),
    createAnalyser: () => analyser,
    createMediaStreamSource: () => source,
  };
  return { createContext: () => context as unknown as AudioContext };
}

function sessionOf(server: SimServer, user: typeof MAMIE, level: { value: number }) {
  const audio = loudnessMeter(level);
  const session = new VoiceSession({
    api: server.api(user),
    deviceId: `device-${user.userId}`,
    watching: false,
    acquireMic: async () => fakeMic(),
    watchSpeech: (stream, onChange) => watchSpeech(stream, onChange, audio),
    createPeer: () => {
      throw new Error('no peers expected');
    },
    newSession: () => 's',
    now: Date.now,
    onChange: () => undefined,
  });
  server.sessions.set(user.userId, [...(server.sessions.get(user.userId) ?? []), session]);
  return session;
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.spyOn(performance, 'now').mockImplementation(() => Date.now());
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('the speaking indicator driven by the real speech detector', () => {
  it('reaches the room and everyone else from a loud microphone, and clears when it falls quiet', async () => {
    const server = new SimServer();
    const level = { value: 0 };
    const mamie = sessionOf(server, MAMIE, level);
    const papi = sessionOf(server, PAPI, { value: 0 });
    await mamie.join();
    await settle(10);

    level.value = 0.5;
    await settle(400);
    const loud = server.speaking;
    const heardByPapi = papi.snapshot.speaking.has(mamie.snapshot.peerId ?? '');
    level.value = 0;
    await settle(1_200);

    expect(mamie.snapshot.peerId).toBeTruthy();
    expect(loud).toEqual([mamie.snapshot.peerId]);
    expect(heardByPapi).toBe(true);
    expect(server.speaking).toEqual([]);
    expect(mamie.snapshot.talking).toBe(false);
  });

  it('keeps the flag alive past the server lapse while speech lasts', async () => {
    const server = new SimServer();
    const level = { value: 0.5 };
    const mamie = sessionOf(server, MAMIE, level);
    await mamie.join();

    await settle(7_000);

    expect(server.speaking).toEqual([mamie.snapshot.peerId]);
  });

  it('never reports a microphone that stays under the speech threshold', async () => {
    const server = new SimServer();
    const mamie = sessionOf(server, MAMIE, { value: 0.01 });
    await mamie.join();

    await settle(3_000);

    expect(server.speaking).toEqual([]);
    expect(mamie.snapshot.talking).toBe(false);
  });
});
