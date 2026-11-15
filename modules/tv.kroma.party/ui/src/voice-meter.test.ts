import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMeterAudio, rmsOf, SpeechGate, watchSpeech } from './voice-meter';

describe('SpeechGate', () => {
  it('turns on only after the signal stays loud for the attack time', () => {
    const gate = new SpeechGate();

    const early = gate.push(0.2, 1000);
    const later = gate.push(0.2, 1080);

    expect(early).toBeNull();
    expect(later).toBe(true);
  });

  it('keeps speaking through a short pause and turns off after the release time', () => {
    const gate = new SpeechGate();
    gate.push(0.2, 1000);
    gate.push(0.2, 1100);

    const pause = gate.push(0.0, 1200);
    const over = gate.push(0.0, 1750);

    expect(pause).toBeNull();
    expect(over).toBe(false);
  });

  it('ignores a blip shorter than the attack time', () => {
    const gate = new SpeechGate();

    gate.push(0.2, 1000);
    const quiet = gate.push(0.0, 1040);
    const again = gate.push(0.2, 1060);

    expect(quiet).toBeNull();
    expect(again).toBeNull();
  });

  it('reports the end of speech on reset and nothing when it was silent', () => {
    const gate = new SpeechGate();
    expect(gate.reset()).toBeNull();
    gate.push(0.2, 1000);
    gate.push(0.2, 1100);

    expect(gate.reset()).toBe(false);
  });
});

describe('rmsOf', () => {
  it('is zero for silence and one for a full-scale square wave', () => {
    expect(rmsOf(new Uint8Array([128, 128, 128, 128]))).toBe(0);
    expect(rmsOf(new Uint8Array([0, 255, 0, 255]))).toBeCloseTo(1, 1);
  });
});

function fakeContext(level: { value: number }) {
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
  return { context: context as unknown as AudioContext, source, raw: context };
}

describe('watchSpeech', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.spyOn(performance, 'now').mockImplementation(() => Date.now());
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('reports the start and the end of speech and releases the audio on stop', () => {
    const level = { value: 0 };
    const { context, source, raw } = fakeContext(level);
    const changes: boolean[] = [];

    const stop = watchSpeech({} as MediaStream, (on) => changes.push(on), {
      createContext: () => context,
    });
    level.value = 0.5;
    vi.advanceTimersByTime(300);
    stop();

    expect(changes).toEqual([true, false]);
    expect(source.disconnect).toHaveBeenCalled();
    expect(raw.close).toHaveBeenCalled();
  });

  it('reports nothing for a quiet microphone', () => {
    const { context } = fakeContext({ value: 0.001 });
    const changes: boolean[] = [];

    const stop = watchSpeech({} as MediaStream, (on) => changes.push(on), {
      createContext: () => context,
    });
    vi.advanceTimersByTime(1000);
    stop();

    expect(changes).toEqual([]);
  });
});

describe('createMeterAudio', () => {
  it('hands over the context created in the gesture, once', () => {
    const made: AudioContext[] = [];
    const make = () => {
      const context = fakeContext({ value: 0 }).context;
      made.push(context);
      return context;
    };
    const audio = createMeterAudio(make);

    audio.prime();
    const first = audio.createContext();
    const second = audio.createContext();

    expect(first).toBe(made[0]);
    expect(second).toBe(made[1]);
    expect(made).toHaveLength(2);
  });
});
