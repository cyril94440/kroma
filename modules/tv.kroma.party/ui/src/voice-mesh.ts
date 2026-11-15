import { PeerLink } from './voice-link';
import type { LinkState, PeerFactory, Send } from './voice-peer';
import type { IceServer, SignalEvent, VoiceMember } from './voice-schemas';
import { SESSION_TIMING, Timers } from './voice-timing';

interface MeshDeps {
  self: { peerId: string; userId: string };
  createPeer: PeerFactory;
  send: Send;
  onStream: (peerId: string, stream: MediaStream | null) => void;
  onLinkState: (peerId: string, state: LinkState) => void;
  onSelfUnknown: () => void;
  onNeedRoster: () => void;
  newSession: () => string;
  now: () => number;
}

/** The full mesh of one participant: one {@link PeerLink} per other person in
 *  the room. Two devices of one account are never linked to each other, they
 *  would only feed back. A signal that arrives before the roster that names
 *  its sender is parked, the roster is asked for, and the signal is replayed
 *  once the sender is known. */
export class VoiceMesh {
  private readonly links = new Map<string, PeerLink>();
  private readonly gone = new Set<string>();
  private readonly parked = new Map<string, SignalEvent[]>();
  private readonly timers = new Timers();
  private track: MediaStreamTrack | null = null;
  private iceServers: IceServer[] = [];

  constructor(private readonly deps: MeshDeps) {}

  get peerIds(): string[] {
    return [...this.links.keys()];
  }

  setIceServers(servers: IceServer[]): void {
    this.iceServers = servers;
    for (const link of this.links.values()) link.setIceServers(servers);
  }

  setTrack(track: MediaStreamTrack | null): void {
    this.track = track;
    for (const link of this.links.values()) link.setTrack(track);
  }

  setRoster(members: VoiceMember[]): void {
    const present = new Set(members.map((m) => m.peerId));
    for (const peerId of [...this.links.keys()]) {
      if (!present.has(peerId)) this.drop(peerId);
    }
    for (const peerId of [...this.gone]) if (!present.has(peerId)) this.gone.delete(peerId);
    for (const member of members) {
      if (this.linkable(member) && !this.links.has(member.peerId)) this.open(member.peerId);
    }
    for (const peerId of [...this.parked.keys()]) {
      if (this.links.has(peerId)) this.replay(peerId);
    }
  }

  receive(event: SignalEvent): void {
    if (event.to !== this.deps.self.peerId || event.fromUserId === this.deps.self.userId) return;
    const link = this.links.get(event.from);
    if (link) {
      void link.receive(event);
      return;
    }
    if (this.gone.has(event.from)) return;
    this.park(event);
  }

  close(): void {
    this.timers.clearAll();
    this.parked.clear();
    for (const peerId of [...this.links.keys()]) this.drop(peerId);
  }

  private linkable(member: VoiceMember): boolean {
    return (
      member.peerId !== this.deps.self.peerId &&
      member.userId !== this.deps.self.userId &&
      !this.gone.has(member.peerId)
    );
  }

  private open(peerId: string): void {
    const link = new PeerLink({
      self: this.deps.self.peerId,
      remote: peerId,
      createPeer: this.deps.createPeer,
      send: async (body) => {
        const result = await this.deps.send(body);
        if (result === 'gone') this.leave(peerId);
        if (result === 'rejoin') this.deps.onSelfUnknown();
        return result;
      },
      onStream: (stream) => this.deps.onStream(peerId, stream),
      onState: (state) => this.deps.onLinkState(peerId, state),
      newSession: this.deps.newSession,
      now: this.deps.now,
    });
    this.links.set(peerId, link);
    link.start(this.track, this.iceServers);
  }

  private leave(peerId: string): void {
    this.gone.add(peerId);
    this.drop(peerId);
  }

  private drop(peerId: string): void {
    this.links.get(peerId)?.close();
    this.links.delete(peerId);
    this.parked.delete(peerId);
    this.deps.onStream(peerId, null);
  }

  private park(event: SignalEvent): void {
    const queue = this.parked.get(event.from) ?? [];
    if (queue.length >= SESSION_TIMING.pendingSignalsMax) return;
    this.parked.set(event.from, [...queue, event]);
    this.timers.arm(`parked:${event.from}`, SESSION_TIMING.rosterWaitMs, () =>
      this.parked.delete(event.from),
    );
    this.deps.onNeedRoster();
  }

  private replay(peerId: string): void {
    const link = this.links.get(peerId);
    const queue = this.parked.get(peerId) ?? [];
    this.parked.delete(peerId);
    this.timers.clear(`parked:${peerId}`);
    for (const event of queue) void link?.receive(event);
  }
}
