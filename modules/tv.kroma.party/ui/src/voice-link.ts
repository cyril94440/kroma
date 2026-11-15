import type { LinkPeer, LinkState, PeerFactory, Send } from './voice-peer';
import type {
  IceCandidate,
  IceServer,
  SessionDescription,
  SignalBody,
  SignalEvent,
} from './voice-schemas';
import { Negotiation } from './voice-negotiation';
import { Outbox } from './voice-outbox';
import { LINK_TIMING, Timers } from './voice-timing';

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

type Route = { from: string; to: string; session: string };

/** One audio connection to one remote peer, negotiated with the perfect
 *  negotiation pattern. The impolite side (the smaller peer id) owns the
 *  session: it mints a new one whenever it rebuilds the connection, and the
 *  polite side adopts it from the first offer that carries it. Recovery from a
 *  lost message is always the same move: rebuild under a new session. */
export class PeerLink {
  readonly polite: boolean;
  private peer: LinkPeer | null = null;
  private token: object | null = null;
  private session: string | null = null;
  private sessionAt = 0;
  private lastN = 0;
  private readonly negotiation = new Negotiation();
  private offerResends = 0;
  private iceRestarts = 0;
  private recoveries = 0;
  private hellos = 0;
  private track: MediaStreamTrack | null = null;
  private iceServers: IceServer[] = [];
  private current: LinkState = 'connecting';
  private closed = false;
  private inbox: Promise<void> = Promise.resolve();
  private held: IceCandidate[] | null = null;
  private readonly outbox: Outbox;
  private readonly timers = new Timers();

  constructor(private readonly deps: LinkDeps) {
    this.polite = deps.self > deps.remote;
    this.outbox = new Outbox(deps.send, (body) => body.session !== (this.session ?? 'none'));
  }

  get state(): LinkState {
    return this.current;
  }

  start(track: MediaStreamTrack | null, iceServers: IceServer[]): void {
    this.track = track;
    this.iceServers = iceServers;
    if (this.polite) this.armHello();
    else this.rebuild();
  }

  setTrack(track: MediaStreamTrack | null): void {
    this.track = track;
    this.peer?.setTrack(track);
  }

  setIceServers(servers: IceServer[]): void {
    this.iceServers = servers;
    this.peer?.setIceServers(servers);
  }

  receive(message: SignalEvent): Promise<void> {
    this.inbox = this.inbox.then(() => this.process(message)).catch(() => undefined);
    return this.inbox;
  }

  close(): void {
    this.closed = true;
    this.outbox.close();
    this.timers.clearAll();
    this.token = null;
    this.peer?.close();
    this.peer = null;
  }

  private route(): Route {
    return { from: this.deps.self, to: this.deps.remote, session: this.session ?? 'none' };
  }

  private setState(next: LinkState): void {
    if (next === this.current) return;
    this.current = next;
    this.deps.onState(next);
  }

  private rebuild(): void {
    this.session = this.deps.newSession();
    this.sessionAt = this.deps.now();
    this.build();
  }

  private adopt(session: string): void {
    this.session = session;
    this.sessionAt = this.deps.now();
    this.build();
  }

  private build(): void {
    this.peer?.close();
    this.timers.clear('hello');
    this.timers.clear('offer');
    this.timers.clear('disconnect');
    this.timers.clear('gap');
    this.negotiation.reset();
    this.iceRestarts = 0;
    this.offerResends = 0;
    const token = {};
    this.token = token;
    const live = () => this.token === token && !this.closed;
    const peer = this.deps.createPeer(
      {
        onCandidate: (candidate) => {
          if (live()) this.candidate(candidate);
        },
        onNegotiationNeeded: () => {
          if (live()) void this.negotiate(peer);
        },
        onConnectionState: (state) => {
          if (live()) this.connection(state);
        },
        onStream: (stream) => {
          if (live()) this.deps.onStream(stream);
        },
      },
      this.iceServers,
    );
    this.peer = peer;
    peer.setTrack(this.track);
    this.setState('connecting');
    this.armConnect();
  }

  private armConnect(): void {
    this.timers.arm('connect', LINK_TIMING.connectTimeoutMs, () => {
      if (this.current !== 'connected') this.recover();
    });
  }

  private armHello(): void {
    this.timers.arm('hello', LINK_TIMING.helloAfterMs, () => {
      if (this.peer || this.closed) return;
      this.sayHello();
      this.armHello();
    });
  }

  private sayHello(): void {
    if (++this.hellos > LINK_TIMING.maxRecoveries) {
      this.timers.clearAll();
      this.setState('failed');
      return;
    }
    this.post({ ...this.route(), kind: 'hello' });
  }

  private recover(): void {
    if (this.closed) return;
    if (this.polite) {
      this.sayHello();
      this.armConnect();
      return;
    }
    if (++this.recoveries > LINK_TIMING.maxRecoveries) {
      this.timers.clearAll();
      this.setState('failed');
      return;
    }
    this.rebuild();
  }

  private post(body: SignalBody): void {
    void this.outbox.push(body);
  }

  private candidate(candidate: IceCandidate): void {
    if (this.held) this.held.push(candidate);
    else this.post({ ...this.route(), kind: 'candidate', candidate });
  }

  private release(): void {
    const held = this.held ?? [];
    this.held = null;
    for (const candidate of held) this.post({ ...this.route(), kind: 'candidate', candidate });
  }

  private async negotiate(peer: LinkPeer): Promise<void> {
    this.held ??= [];
    const offer = await this.negotiation.makeOffer(peer);
    if (offer && this.peer === peer) {
      this.post({ ...this.route(), kind: 'description', description: offer });
      this.armOffer(peer, offer);
    }
    this.release();
  }

  private armOffer(peer: LinkPeer, offer: SessionDescription): void {
    this.timers.arm('offer', LINK_TIMING.offerResendMs, () => {
      if (this.peer !== peer || peer.signalingState !== 'have-local-offer') return;
      if (this.offerResends >= LINK_TIMING.maxOfferResends) {
        this.recover();
        return;
      }
      this.offerResends += 1;
      this.post({ ...this.route(), kind: 'description', description: offer });
      this.armOffer(peer, offer);
    });
  }

  private connection(state: string): void {
    if (state === 'connected') {
      this.timers.clear('connect');
      this.timers.clear('disconnect');
      this.timers.clear('gap');
      this.iceRestarts = 0;
      this.recoveries = 0;
      this.hellos = 0;
      this.setState('connected');
    } else if (state === 'disconnected') {
      this.timers.arm('disconnect', LINK_TIMING.disconnectGraceMs, () => this.restartIce());
    } else if (state === 'failed') {
      this.restartIce();
    }
  }

  private restartIce(): void {
    if (!this.peer || this.closed) return;
    if (this.iceRestarts >= LINK_TIMING.maxIceRestarts) {
      this.recover();
      return;
    }
    this.iceRestarts += 1;
    this.setState('connecting');
    this.peer.restartIce();
    this.armConnect();
  }

  private async process(message: SignalEvent): Promise<void> {
    if (this.closed || message.n <= this.lastN) return;
    const gap = message.n > this.lastN + 1;
    this.lastN = message.n;
    if (gap && !this.timers.has('gap')) this.armGapCheck();
    if (message.kind === 'hello') this.onHello();
    else if (message.kind === 'candidate') await this.onCandidate(message);
    else await this.onDescription(message);
  }

  private armGapCheck(): void {
    this.timers.arm('gap', LINK_TIMING.gapCheckMs, () => {
      if (this.current !== 'connected') this.recover();
    });
  }

  private onHello(): void {
    if (this.polite) return;
    const fresh = this.deps.now() - this.sessionAt < LINK_TIMING.helloCooldownMs;
    if (!fresh || this.current === 'failed') this.recover();
  }

  private async onCandidate(message: Extract<SignalEvent, { kind: 'candidate' }>): Promise<void> {
    if (this.peer && message.session === this.session) {
      await this.negotiation.addCandidate(this.peer, message.candidate);
    }
  }

  private async onDescription(
    message: Extract<SignalEvent, { kind: 'description' }>,
  ): Promise<void> {
    const description = message.description;
    if (message.session !== this.session) {
      if (!this.polite || description.type !== 'offer') return;
      this.adopt(message.session);
    }
    const peer = this.peer;
    if (!peer) return;
    if (description.type === 'offer') this.held ??= [];
    const applied = await this.negotiation.apply(peer, description, this.polite);
    if (this.peer !== peer) return;
    if (applied === 'accepted') this.timers.clear('offer');
    const answer = this.negotiation.answer;
    if ((applied === 'answered' || applied === 'resent') && answer) {
      this.post({ ...this.route(), kind: 'description', description: answer });
    }
    this.release();
  }
}
