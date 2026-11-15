export type MicFailure = 'insecure' | 'denied' | 'missing' | 'timeout' | 'unavailable';

export class MicError extends Error {
  constructor(readonly failure: MicFailure) {
    super(failure);
    this.name = 'MicError';
  }
}

interface MicEnv {
  secure: boolean;
  devices: Pick<MediaDevices, 'getUserMedia'> | undefined;
}

const MIC_TIMEOUT_MS = 12_000;
const AUDIO = { echoCancellation: true, noiseSuppression: true } as const;

function browserMicEnv(): MicEnv {
  return { secure: globalThis.isSecureContext, devices: globalThis.navigator?.mediaDevices };
}

function failureOf(error: unknown): MicFailure {
  const name = error instanceof Error ? error.name : '';
  if (name === 'NotAllowedError' || name === 'SecurityError') return 'denied';
  if (name === 'NotFoundError' || name === 'OverconstrainedError') return 'missing';
  return 'unavailable';
}

export function voiceSupported(env: MicEnv = browserMicEnv()): boolean {
  return env.secure && Boolean(env.devices?.getUserMedia) && 'RTCPeerConnection' in globalThis;
}

/** The microphone with echo cancellation and noise suppression on. Rejects with
 *  a {@link MicError} naming why: a page outside a secure context never gets a
 *  prompt, and a browser that never answers the prompt is given 12 s. */
export async function acquireMic(env: MicEnv = browserMicEnv()): Promise<MediaStream> {
  if (!env.secure || !env.devices?.getUserMedia) throw new MicError('insecure');
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new MicError('timeout')), MIC_TIMEOUT_MS);
  });
  try {
    return await Promise.race([env.devices.getUserMedia({ audio: AUDIO, video: false }), timeout]);
  } catch (error) {
    throw error instanceof MicError ? error : new MicError(failureOf(error));
  } finally {
    clearTimeout(timer);
  }
}

export function stopStream(stream: MediaStream | null): void {
  stream?.getTracks().forEach((track) => track.stop());
}
