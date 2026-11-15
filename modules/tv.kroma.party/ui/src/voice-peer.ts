import type { IceCandidate, IceServer, SessionDescription, SignalBody } from './voice-schemas';

export interface LinkPeer {
  readonly signalingState: string;
  readonly connectionState: string;
  readonly localDescription: SessionDescription | null;
  setLocalDescription(): Promise<void>;
  setRemoteDescription(description: SessionDescription): Promise<void>;
  addIceCandidate(candidate: IceCandidate): Promise<void>;
  restartIce(): void;
  setIceServers(servers: IceServer[]): void;
  setTrack(track: MediaStreamTrack | null): void;
  close(): void;
}

export interface PeerHandlers {
  onCandidate(candidate: IceCandidate): void;
  onNegotiationNeeded(): void;
  onConnectionState(state: string): void;
  onStream(stream: MediaStream): void;
}

export type PeerFactory = (handlers: PeerHandlers, iceServers: IceServer[]) => LinkPeer;

export type SendResult = 'ok' | 'gone' | 'rejoin' | 'failed';
export type Send = (body: SignalBody) => Promise<SendResult>;

export type LinkState = 'connecting' | 'connected' | 'failed';
