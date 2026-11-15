import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { acquireMic, MicError, stopStream, voiceSupported } from './voice-mic';

function devices(result: () => Promise<MediaStream>) {
  return { getUserMedia: vi.fn(result) };
}

const stream = { getTracks: () => [] } as unknown as MediaStream;

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

async function failureOf(promise: Promise<unknown>): Promise<string> {
  const error = await promise.catch((e: unknown) => e);
  return error instanceof MicError ? error.failure : 'not a MicError';
}

describe('acquireMic', () => {
  it('asks for echo cancellation and noise suppression on audio only', async () => {
    const mic = devices(async () => stream);

    const got = await acquireMic({ secure: true, devices: mic });

    expect(got).toBe(stream);
    expect(mic.getUserMedia).toHaveBeenCalledWith({
      audio: { echoCancellation: true, noiseSuppression: true },
      video: false,
    });
  });

  it('refuses outside a secure context without asking the browser', async () => {
    const mic = devices(async () => stream);

    const failure = await failureOf(acquireMic({ secure: false, devices: mic }));

    expect(failure).toBe('insecure');
    expect(mic.getUserMedia).not.toHaveBeenCalled();
  });

  it('refuses a browser that has no media devices', async () => {
    expect(await failureOf(acquireMic({ secure: true, devices: undefined }))).toBe('insecure');
  });

  it.each([
    ['NotAllowedError', 'denied'],
    ['SecurityError', 'denied'],
    ['NotFoundError', 'missing'],
    ['OverconstrainedError', 'missing'],
    ['AbortError', 'unavailable'],
  ])('names a %s as %s', async (name, expected) => {
    const rejected = devices(async () => Promise.reject(Object.assign(new Error('x'), { name })));

    expect(await failureOf(acquireMic({ secure: true, devices: rejected }))).toBe(expected);
  });

  it('gives up after twelve seconds without an answer', async () => {
    const silent = devices(() => new Promise<MediaStream>(() => undefined));

    const outcome = failureOf(acquireMic({ secure: true, devices: silent }));
    await vi.advanceTimersByTimeAsync(12_000);

    expect(await outcome).toBe('timeout');
  });
});

describe('voiceSupported', () => {
  it('needs a secure context, media devices and RTCPeerConnection', () => {
    vi.stubGlobal('RTCPeerConnection', class {});
    const mic = devices(async () => stream);

    expect(voiceSupported({ secure: true, devices: mic })).toBe(true);
    expect(voiceSupported({ secure: false, devices: mic })).toBe(false);
    expect(voiceSupported({ secure: true, devices: undefined })).toBe(false);
  });
});

describe('stopStream', () => {
  it('stops every track and tolerates no stream', () => {
    const stop = vi.fn();
    const held = { getTracks: () => [{ stop }, { stop }] } as unknown as MediaStream;

    stopStream(held);
    stopStream(null);

    expect(stop).toHaveBeenCalledTimes(2);
  });
});
