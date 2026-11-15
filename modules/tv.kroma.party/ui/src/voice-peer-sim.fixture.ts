import type { LinkPeer, PeerFactory, PeerHandlers } from './voice-peer';
import type { IceCandidate, SessionDescription } from './voice-schemas';

export class SimNet {
  readonly peers = new Map<string, SimPeer>();
  created = 0;
  dropCandidatesFrom = new Set<string>();
  blocked = false;

  next(): string {
    this.created += 1;
    return `rtc${this.created}`;
  }

  unblock(): void {
    this.blocked = false;
    for (const peer of this.peers.values()) {
      const remote = this.peers.get(peer.remoteId);
      if (remote) this.evaluate(peer, remote);
    }
  }

  evaluate(a: SimPeer, b: SimPeer): void {
    if (this.blocked || !a.ready || !b.ready || a.epoch !== b.epoch) return;
    a.connect();
    b.connect();
  }
}

export class SimPeer implements LinkPeer {
  signalingState = 'stable';
  connectionState = 'new';
  localDescription: SessionDescription | null = null;
  remoteDescription: SessionDescription | null = null;
  epoch = 0;
  candidatesIn = 0;
  negotiated = false;
  remoteId = '';
  closed = false;
  restarts = 0;
  rollbacks = 0;
  track: MediaStreamTrack | null = null;
  readonly id: string;

  constructor(
    private readonly net: SimNet,
    private readonly handlers: PeerHandlers,
    readonly label: string,
  ) {
    this.id = net.next();
    net.peers.set(this.id, this);
    queueMicrotask(() => this.needNegotiation());
  }

  get ready(): boolean {
    return this.negotiated && this.candidatesIn > 0;
  }

  async setLocalDescription(): Promise<void> {
    if (this.closed) throw new Error('closed');
    if (this.signalingState === 'have-remote-offer') {
      this.localDescription = { type: 'answer', sdp: `answer|${this.id}|${this.epoch}` };
      this.signalingState = 'stable';
      this.negotiated = true;
    } else {
      this.localDescription = { type: 'offer', sdp: `offer|${this.id}|${this.epoch}` };
      this.signalingState = 'have-local-offer';
    }
    queueMicrotask(() => this.emitCandidates());
  }

  async setRemoteDescription(description: SessionDescription): Promise<void> {
    if (this.closed) throw new Error('closed');
    const [kind = '', remoteId = '', epoch = '0'] = description.sdp.split('|');
    if (description.type === 'offer') {
      if (this.signalingState === 'have-local-offer') {
        this.rollbacks += 1;
        this.localDescription = null;
      }
      this.signalingState = 'have-remote-offer';
      this.remoteId = remoteId;
      this.epoch = Number(epoch);
      this.candidatesIn = 0;
      this.negotiated = false;
      return;
    }
    if (this.signalingState !== 'have-local-offer' || kind !== 'answer') throw new Error('state');
    this.signalingState = 'stable';
    this.remoteId = remoteId;
    this.negotiated = true;
    this.candidatesIn = 0;
    this.reevaluate();
  }

  async addIceCandidate(candidate: IceCandidate): Promise<void> {
    if (!this.remoteDescription && !this.remoteId) throw new Error('no remote description');
    if (candidate.candidate.split('|')[2] !== String(this.epoch)) return;
    this.candidatesIn += 1;
    this.reevaluate();
  }

  restartIce(): void {
    this.restarts += 1;
    this.epoch += 1;
    this.negotiated = false;
    this.connectionState = 'connecting';
    queueMicrotask(() => this.needNegotiation());
  }

  setIceServers(): void {}

  setTrack(track: MediaStreamTrack | null): void {
    this.track = track;
  }

  close(): void {
    this.closed = true;
    this.connectionState = 'closed';
    this.net.peers.delete(this.id);
  }

  fail(): void {
    this.connectionState = 'failed';
    this.handlers.onConnectionState('failed');
  }

  drop(): void {
    this.connectionState = 'disconnected';
    this.handlers.onConnectionState('disconnected');
  }

  connect(): void {
    if (this.connectionState === 'connected') return;
    this.connectionState = 'connected';
    this.handlers.onConnectionState('connected');
    this.handlers.onStream({ id: this.id } as unknown as MediaStream);
  }

  private needNegotiation(): void {
    if (!this.closed && this.signalingState === 'stable') this.handlers.onNegotiationNeeded();
  }

  private emitCandidates(): void {
    if (this.closed || this.net.dropCandidatesFrom.has(this.label)) return;
    for (let i = 0; i < 2; i += 1) {
      this.handlers.onCandidate({
        candidate: `cand|${this.id}|${this.epoch}|${i}`,
        sdpMid: '0',
        sdpMLineIndex: 0,
      });
    }
  }

  private reevaluate(): void {
    const remote = this.net.peers.get(this.remoteId);
    if (remote) this.net.evaluate(this, remote);
  }
}

export function simFactory(net: SimNet, label: string): { create: PeerFactory; made: SimPeer[] } {
  const made: SimPeer[] = [];
  return {
    made,
    create: (handlers) => {
      const peer = new SimPeer(net, handlers, label);
      made.push(peer);
      return peer;
    },
  };
}
