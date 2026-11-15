import type { LinkState, PeerFactory, Send } from './voice-peer';

export interface LinkDeps {
  self: string;
  remote: string;
  createPeer: PeerFactory;
  send: Send;
  onStream: (stream: MediaStream) => void;
  onState: (state: LinkState) => void;
  newSession: () => string;
  now: () => number;
}
