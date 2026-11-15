import { apiErrorText, KromaApiError } from '@kromatv/client';
import type { ModuleApi } from '@kromatv/module-sdk';
import { z } from 'zod';
import type { SendResult } from './voice-peer';
import {
  BeatAnswer,
  IceAnswer,
  JoinAnswer,
  type IceServer,
  type SignalBody,
  SignalAnswer,
  VoiceState,
} from './voice-schemas';

export type VoiceRequestFailure = 'full' | 'rateLimited' | 'failed';

export class VoiceRequestError extends Error {
  constructor(readonly failure: VoiceRequestFailure) {
    super(failure);
    this.name = 'VoiceRequestError';
  }
}

export interface JoinBody {
  deviceId: string;
  muted: boolean;
  watching: boolean;
}

export interface VoiceApi {
  state(): Promise<VoiceState>;
  join(body: JoinBody): Promise<JoinAnswer>;
  leave(peerId: string): Promise<void>;
  leaveOnUnload(peerId: string): void;
  beat(peerId: string): Promise<number | 'rejoin'>;
  mute(peerId: string, muted: boolean): Promise<void>;
  speaking(peerId: string, speaking: boolean): Promise<void>;
  signal(body: SignalBody): Promise<SendResult>;
  ice(): Promise<IceServer[]>;
}

const Ok = z.object({ ok: z.boolean().catch(true) });

function failureOf(error: unknown): VoiceRequestFailure {
  if (error instanceof KromaApiError && error.status === 409) return 'full';
  if (error instanceof KromaApiError && error.status === 429) return 'rateLimited';
  return 'failed';
}

async function guarded<T>(run: Promise<T>): Promise<T> {
  try {
    return await run;
  } catch (error) {
    throw new VoiceRequestError(failureOf(error));
  }
}

function refusal(error: unknown): SendResult {
  if (!(error instanceof KromaApiError) || error.status === 429) return 'failed';
  const code = apiErrorText(error, '');
  if (code === 'peerGone') return 'gone';
  if (code === 'notJoined') return 'rejoin';
  return 'failed';
}

export function createVoiceApi(api: ModuleApi): VoiceApi {
  return {
    state: () => guarded(api.get('/voice/state', VoiceState)),
    join: (body) => guarded(api.post('/voice/join', body, JoinAnswer)),
    leave: async (peerId) => {
      await guarded(api.post('/voice/leave', { peerId }, Ok));
    },
    leaveOnUnload: (peerId) => api.beacon('/voice/leave', { peerId, quiet: true }),
    beat: async (peerId) => {
      try {
        return (await api.post('/voice/beat', { peerId }, BeatAnswer)).revision;
      } catch (error) {
        if (refusal(error) === 'rejoin') return 'rejoin';
        throw new VoiceRequestError(failureOf(error));
      }
    },
    mute: async (peerId, muted) => {
      await guarded(api.post('/voice/mute', { peerId, muted }, Ok));
    },
    speaking: async (peerId, speaking) => {
      await guarded(api.post('/voice/speaking', { peerId, speaking }, Ok));
    },
    signal: async (body) => {
      try {
        await api.post('/voice/signal', body, SignalAnswer);
        return 'ok';
      } catch (error) {
        return refusal(error);
      }
    },
    ice: async () => (await guarded(api.get('/voice/ice', IceAnswer))).iceServers,
  };
}
