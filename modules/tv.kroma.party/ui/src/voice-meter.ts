const SAMPLE_EVERY_MS = 60;

interface SpeechGateOptions {
  threshold: number;
  attackMs: number;
  releaseMs: number;
}

const SPEECH_GATE: SpeechGateOptions = { threshold: 0.045, attackMs: 80, releaseMs: 550 };

/** Turns a stream of RMS samples into speaking on and off, with a fast attack
 *  and a slow release so a pause between words does not flicker the indicator.
 *  `push` answers the new state when it changed and null otherwise. */
export class SpeechGate {
  private speaking = false;
  private aboveSince = 0;
  private belowSince = 0;

  constructor(private readonly options: SpeechGateOptions = SPEECH_GATE) {}

  push(rms: number, nowMs: number): boolean | null {
    if (rms > this.options.threshold) {
      this.belowSince = 0;
      this.aboveSince ||= nowMs;
      if (!this.speaking && nowMs - this.aboveSince >= this.options.attackMs) return this.set(true);
      return null;
    }
    this.aboveSince = 0;
    this.belowSince ||= nowMs;
    if (this.speaking && nowMs - this.belowSince >= this.options.releaseMs) return this.set(false);
    return null;
  }

  reset(): boolean | null {
    this.aboveSince = 0;
    this.belowSince = 0;
    return this.speaking ? this.set(false) : null;
  }

  private set(next: boolean): boolean {
    this.speaking = next;
    return next;
  }
}

export function rmsOf(samples: Uint8Array): number {
  let sum = 0;
  for (const byte of samples) {
    const value = (byte - 128) / 128;
    sum += value * value;
  }
  return Math.sqrt(sum / samples.length);
}

export interface MeterAudio {
  createContext: () => AudioContext;
}

interface PrimedAudio extends MeterAudio {
  prime: () => void;
}

/** Creates the audio context during the click that joins, because Safari only
 *  lets a context start inside a user gesture, and hands it to the meter once
 *  the microphone is there. */
export function createMeterAudio(make: () => AudioContext = () => new AudioContext()): PrimedAudio {
  let primed: AudioContext | null = null;
  return {
    prime: () => {
      primed ??= make();
      void primed.resume().catch(() => undefined);
    },
    createContext: () => {
      const context = primed ?? make();
      primed = null;
      return context;
    },
  };
}

const browserAudio: MeterAudio = { createContext: () => new AudioContext() };

/** Watches a microphone stream and calls `onChange` when its owner starts or
 *  stops speaking. A plain interval rather than an animation frame, which a
 *  background tab freezes. Returns the function that stops it. */
export function watchSpeech(
  stream: MediaStream,
  onChange: (speaking: boolean) => void,
  audio: MeterAudio = browserAudio,
): () => void {
  const context = audio.createContext();
  void context.resume().catch(() => undefined);
  const source = context.createMediaStreamSource(stream);
  const analyser = context.createAnalyser();
  analyser.fftSize = 512;
  source.connect(analyser);
  const samples = new Uint8Array(analyser.fftSize);
  const gate = new SpeechGate();
  const timer = setInterval(() => {
    analyser.getByteTimeDomainData(samples);
    const changed = gate.push(rmsOf(samples), performance.now());
    if (changed !== null) onChange(changed);
  }, SAMPLE_EVERY_MS);
  return () => {
    clearInterval(timer);
    source.disconnect();
    void context.close().catch(() => undefined);
    if (gate.reset() !== null) onChange(false);
  };
}
