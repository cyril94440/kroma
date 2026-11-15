import type { MicFailure } from './voice-mic';
import type { LinkState } from './voice-peer';
import type { VoiceMember } from './voice-schemas';

export type VoiceFailure = MicFailure | 'full' | 'rateLimited' | 'failed';
export type VoicePhase = 'idle' | 'joining' | 'live';

export interface VoiceView {
  phase: VoicePhase;
  peerId: string | null;
  participants: VoiceMember[];
  speaking: ReadonlySet<string>;
  muted: boolean;
  failure: VoiceFailure | null;
  streams: ReadonlyMap<string, MediaStream>;
  links: ReadonlyMap<string, LinkState>;
  talking: boolean;
}

export const IDLE: VoiceView = {
  phase: 'idle',
  peerId: null,
  participants: [],
  speaking: new Set(),
  muted: false,
  failure: null,
  streams: new Map(),
  links: new Map(),
  talking: false,
};
