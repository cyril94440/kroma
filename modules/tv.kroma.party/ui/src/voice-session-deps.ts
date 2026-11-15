import type { VoiceApi } from './voice-api';
import type { watchSpeech } from './voice-meter';
import type { PeerFactory } from './voice-peer';
import type { VoiceView } from './voice-view';

export interface SessionDeps {
  api: VoiceApi;
  deviceId: string;
  watching: boolean;
  acquireMic: () => Promise<MediaStream>;
  watchSpeech?: typeof watchSpeech;
  createPeer: PeerFactory;
  newSession: () => string;
  now: () => number;
  onChange: (view: VoiceView) => void;
}
