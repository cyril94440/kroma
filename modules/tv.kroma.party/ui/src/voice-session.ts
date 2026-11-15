import { MicError, type MicFailure, stopStream } from './voice-mic';
import { watchSpeech } from './voice-meter';
import { Talk } from './voice-talk';
import { VoiceMesh } from './voice-mesh';
import type { LinkState, PeerFactory } from './voice-peer';
import {
  type JoinAnswer,
  type SignalEvent,
  type VoiceEvent,
  type VoiceMember,
  type VoiceState,
} from './voice-schemas';
import { SESSION_TIMING, Timers } from './voice-timing';
import { VoiceRequestError, type VoiceApi } from './voice-api';
import { IDLE, type VoiceView } from './voice-view';

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

/** One person's voice session: observes the room from the moment it exists,
 *  and once `join` has run it holds the microphone, the heartbeat, and the mesh
 *  of connections to everyone else. Every message it sends may be lost: the
 *  roster is whole state with a revision, so a lost one is repaired by the next
 *  or by a read, and a server that forgot us is answered with a rejoin. */
export class VoiceSession {
  private view: VoiceView = IDLE;
  private revision = 0;
  private mesh: VoiceMesh | null = null;
  private mic: MediaStream | null = null;
  private readonly talk: Talk;
  private readonly streams = new Map<string, MediaStream>();
  private readonly links = new Map<string, LinkState>();
  private readonly timers = new Timers();
  private rejoining = false;
  private rejoins: number[] = [];
  private disposed = false;

  constructor(private readonly deps: SessionDeps) {
    this.talk = new Talk({
      watch: deps.watchSpeech ?? watchSpeech,
      report: (speaking) => this.reportSpeaking(speaking),
    });
  }

  get snapshot(): VoiceView {
    return this.view;
  }

  async observe(): Promise<void> {
    this.disposed = false;
    await this.refresh();
  }

  async join(): Promise<void> {
    if (this.view.phase !== 'idle' || this.disposed) return;
    this.patch({ phase: 'joining', failure: null });
    let micFailure: MicFailure | null = null;
    try {
      this.mic = await this.deps.acquireMic();
    } catch (error) {
      micFailure = error instanceof MicError ? error.failure : 'unavailable';
    }
    if (this.disposed) {
      stopStream(this.mic);
      this.mic = null;
      this.patch({ ...IDLE, participants: this.view.participants });
      return;
    }
    const muted = this.mic === null;
    const joined = await this.register(muted);
    if (!joined) return;
    this.patch({ muted, failure: micFailure });
    this.startMeter();
  }

  async leave(): Promise<void> {
    const peerId = this.view.peerId;
    this.teardown();
    this.patch({ ...IDLE, participants: this.view.participants, speaking: this.view.speaking });
    if (peerId) await this.deps.api.leave(peerId).catch(() => undefined);
  }

  leaveOnUnload(): void {
    const peerId = this.view.peerId;
    if (peerId) this.deps.api.leaveOnUnload(peerId);
  }

  dispose(): void {
    this.disposed = true;
    const peerId = this.view.peerId;
    this.teardown();
    if (peerId) void this.deps.api.leave(peerId).catch(() => undefined);
  }

  async setMuted(muted: boolean): Promise<void> {
    const peerId = this.view.peerId;
    if (!peerId || muted === this.view.muted) return;
    if (!muted && !this.mic) {
      if (!(await this.reacquireMic())) return;
    }
    this.mic?.getAudioTracks().forEach((track) => (track.enabled = !muted));
    if (muted) this.talk.silence();
    this.patch({ muted, failure: null });
    await this.deps.api.mute(peerId, muted).catch(() => undefined);
  }

  handleEvent(event: VoiceEvent): void {
    if (event.type === 'party.voice.signal') this.signal(event);
    else if (event.type === 'party.voice.speaking')
      this.patch({ speaking: new Set(event.speaking) });
    else if (event.revision > this.revision) this.adopt(event.revision, event.participants);
  }

  async refresh(): Promise<void> {
    try {
      this.adoptState(await this.deps.api.state());
    } catch {
      return;
    }
  }

  private async register(muted: boolean): Promise<boolean> {
    try {
      const answer = await this.deps.api.join({
        deviceId: this.deps.deviceId,
        muted,
        watching: this.deps.watching,
      });
      if (this.disposed) {
        void this.deps.api.leave(answer.peerId).catch(() => undefined);
        throw new VoiceRequestError('failed');
      }
      this.start(answer, muted);
      return true;
    } catch (error) {
      stopStream(this.mic);
      this.mic = null;
      const failure = error instanceof VoiceRequestError ? error.failure : 'failed';
      this.patch({ ...IDLE, participants: this.view.participants, failure });
      return false;
    }
  }

  private start(answer: JoinAnswer, muted: boolean): void {
    this.mic?.getAudioTracks().forEach((track) => (track.enabled = !muted));
    const mesh = new VoiceMesh({
      self: { peerId: answer.peerId, userId: this.userIdOf(answer) },
      createPeer: this.deps.createPeer,
      send: (body) => this.deps.api.signal(body),
      onStream: (peerId, stream) => this.stream(peerId, stream),
      onLinkState: (peerId, state) => this.linkState(peerId, state),
      onSelfUnknown: () => void this.rejoin(),
      onNeedRoster: () => void this.refresh(),
      newSession: this.deps.newSession,
      now: this.deps.now,
    });
    this.mesh = mesh;
    mesh.setIceServers(answer.iceServers);
    mesh.setTrack(this.mic?.getAudioTracks()[0] ?? null);
    this.revision = answer.revision;
    this.patch({
      phase: 'live',
      peerId: answer.peerId,
      participants: answer.participants,
      speaking: new Set(answer.speaking),
      muted,
    });
    mesh.setRoster(answer.participants);
    this.timers.arm('beat', answer.beatSeconds * 1000, () => void this.beat(answer.beatSeconds));
    this.timers.arm('ice', SESSION_TIMING.iceRefreshMs, () => void this.refreshIce());
  }

  private async refreshIce(): Promise<void> {
    const servers = await this.deps.api.ice().catch(() => null);
    if (servers?.length) this.mesh?.setIceServers(servers);
    if (this.view.phase === 'live') {
      this.timers.arm('ice', SESSION_TIMING.iceRefreshMs, () => void this.refreshIce());
    }
  }

  private userIdOf(answer: JoinAnswer): string {
    return answer.participants.find((member) => member.peerId === answer.peerId)?.userId ?? '';
  }

  private async beat(everySeconds: number): Promise<void> {
    const peerId = this.view.peerId;
    if (!peerId || this.view.phase !== 'live') return;
    const answer = await this.deps.api.beat(peerId).catch(() => null);
    if (answer === 'rejoin') await this.rejoin();
    else if (answer !== null && answer > this.revision) await this.refresh();
    if (this.view.phase === 'live') {
      this.timers.arm('beat', everySeconds * 1000, () => void this.beat(everySeconds));
    }
  }

  private async rejoin(): Promise<void> {
    if (this.rejoining || this.view.phase !== 'live') return;
    const now = this.deps.now();
    this.rejoins = [...this.rejoins.filter((at) => now - at < SESSION_TIMING.rejoinWindowMs), now];
    if (this.rejoins.length > SESSION_TIMING.rejoinsMax) {
      this.teardown();
      this.patch({ ...IDLE, participants: this.view.participants, failure: 'failed' });
      return;
    }
    this.rejoining = true;
    this.timers.clear('beat');
    this.mesh?.close();
    this.mesh = null;
    this.streams.clear();
    this.links.clear();
    const muted = this.view.muted;
    this.patch({ phase: 'joining', peerId: null, streams: new Map(), links: new Map() });
    const joined = await this.register(muted);
    if (!joined && !this.disposed) {
      this.timers.arm('rejoin', SESSION_TIMING.joinRetryMs, () => {
        this.patch({ phase: 'live', peerId: null });
        void this.rejoin();
      });
    }
    this.rejoining = false;
  }

  private signal(event: SignalEvent): void {
    this.mesh?.receive(event);
  }

  private adopt(revision: number, participants: VoiceMember[]): void {
    this.revision = revision;
    const peerId = this.view.peerId;
    this.patch({ participants });
    if (peerId && this.view.phase === 'live' && !participants.some((m) => m.peerId === peerId)) {
      void this.rejoin();
      return;
    }
    this.mesh?.setRoster(participants);
  }

  private adoptState(state: VoiceState): void {
    if (state.revision > this.revision) this.adopt(state.revision, state.participants);
    this.patch({ speaking: new Set(state.speaking) });
  }

  private stream(peerId: string, stream: MediaStream | null): void {
    if (stream) {
      this.streams.set(peerId, stream);
    } else {
      this.streams.delete(peerId);
      this.links.delete(peerId);
    }
    this.patch({ streams: new Map(this.streams), links: new Map(this.links) });
  }

  private linkState(peerId: string, state: LinkState): void {
    this.links.set(peerId, state);
    this.patch({ links: new Map(this.links) });
  }

  private startMeter(): void {
    if (this.mic) this.talk.start(this.mic, () => this.view.muted);
  }

  private reportSpeaking(speaking: boolean): void {
    const peerId = this.view.peerId;
    this.patch({ talking: speaking });
    if (peerId) void this.deps.api.speaking(peerId, speaking).catch(() => undefined);
  }

  private async reacquireMic(): Promise<boolean> {
    try {
      this.mic = await this.deps.acquireMic();
    } catch (error) {
      this.patch({ failure: error instanceof MicError ? error.failure : 'unavailable' });
      return false;
    }
    this.mesh?.setTrack(this.mic.getAudioTracks()[0] ?? null);
    this.startMeter();
    return true;
  }

  private teardown(): void {
    this.timers.clearAll();
    this.talk.stop();
    this.mesh?.close();
    this.mesh = null;
    stopStream(this.mic);
    this.mic = null;
    this.streams.clear();
    this.links.clear();
  }

  private patch(change: Partial<VoiceView>): void {
    this.view = { ...this.view, ...change };
    this.deps.onChange(this.view);
  }
}
