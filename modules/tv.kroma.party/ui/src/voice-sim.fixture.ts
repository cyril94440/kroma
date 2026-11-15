import { vi } from 'vitest';
import { VoiceRequestError, type VoiceApi } from './voice-api';
import { simFactory, SimNet, type SimPeer } from './voice-peer-sim.fixture';
import type { SendResult } from './voice-peer';
import {
  type JoinAnswer,
  type SignalBody,
  VoiceEvent,
  type VoiceMember,
  type VoiceState,
} from './voice-schemas';
import { VoiceSession } from './voice-session';

export interface SimUser {
  userId: string;
  name: string;
}

export class SimServer {
  readonly members: VoiceMember[] = [];
  readonly sessions = new Map<string, VoiceSession[]>();
  readonly counters = new Map<string, number>();
  readonly delivered: Array<{ to: string; event: Record<string, unknown> }> = [];
  revision = 1000;
  speaking: string[] = [];
  dropIf: (to: string, event: Record<string, unknown>) => boolean = () => false;
  full = false;
  private issued = 0;

  state(): VoiceState {
    return { revision: this.revision, participants: [...this.members], speaking: this.speaking };
  }

  forget(peerId: string): void {
    this.drop(peerId);
  }

  broadcastRoster(): void {
    for (const user of this.sessions.keys()) {
      this.push(user, {
        type: 'party.voice.roster',
        revision: this.revision,
        participants: [...this.members],
      });
    }
  }

  push(userId: string, event: Record<string, unknown>): void {
    this.delivered.push({ to: userId, event });
    if (this.dropIf(userId, event)) return;
    const parsed = VoiceEvent.safeParse(event);
    if (!parsed.success)
      throw new Error(`the module emitted an event the client rejects: ${parsed.error.message}`);
    for (const session of this.sessions.get(userId) ?? []) {
      queueMicrotask(() => session.handleEvent(parsed.data));
    }
  }

  private drop(peerId: string): void {
    const at = this.members.findIndex((m) => m.peerId === peerId);
    if (at < 0) return;
    this.members.splice(at, 1);
    this.revision += 1;
    for (const key of [...this.counters.keys()]) {
      if (key.split('>').includes(peerId)) this.counters.delete(key);
    }
  }

  api(user: SimUser): VoiceApi {
    const owns = (peerId: string) =>
      this.members.some((m) => m.peerId === peerId && m.userId === user.userId);
    return {
      state: async () => this.state(),
      join: async (body): Promise<JoinAnswer> => {
        if (this.full) throw new VoiceRequestError('full');
        const previous = this.members.find(
          (m) => m.userId === user.userId && m.peerId.endsWith(`.${body.deviceId}`),
        );
        if (previous) this.drop(previous.peerId);
        this.issued += 1;
        const peerId = `p${this.issued}.${body.deviceId}`;
        this.members.push({ peerId, userId: user.userId, name: user.name, muted: body.muted });
        this.revision += 1;
        queueMicrotask(() => this.broadcastRoster());
        return {
          peerId,
          revision: this.revision,
          participants: [...this.members],
          speaking: this.speaking,
          iceServers: [{ urls: ['stun:stun.example:3478'] }],
          beatSeconds: 10,
          expireSeconds: 30,
        };
      },
      leave: async (peerId) => {
        this.drop(peerId);
        this.broadcastRoster();
      },
      leaveOnUnload: (peerId) => {
        this.drop(peerId);
        this.broadcastRoster();
      },
      beat: async (peerId) => (owns(peerId) ? this.revision : 'rejoin'),
      mute: async (peerId, muted) => {
        const member = this.members.find((m) => m.peerId === peerId);
        if (member) member.muted = muted;
        this.revision += 1;
        this.broadcastRoster();
      },
      speaking: async (peerId, speaking) => {
        this.speaking = speaking ? [peerId] : [];
        for (const key of this.sessions.keys())
          this.push(key, { type: 'party.voice.speaking', speaking: this.speaking });
      },
      signal: async (body: SignalBody): Promise<SendResult> => {
        if (!owns(body.from)) return 'rejoin';
        const target = this.members.find((m) => m.peerId === body.to);
        if (!target) return 'gone';
        const key = `${body.from}>${body.to}`;
        const n = (this.counters.get(key) ?? 0) + 1;
        this.counters.set(key, n);
        this.push(target.userId, {
          type: 'party.voice.signal',
          fromUserId: user.userId,
          n,
          ...body,
        });
        return 'ok';
      },
      ice: async () => [{ urls: ['stun:stun.example:3478'] }],
    };
  }
}

export interface Rig {
  server: SimServer;
  net: SimNet;
  session: VoiceSession;
  peers: SimPeer[];
  user: SimUser;
  mic: MediaStream;
  failures: string[];
  sessionIds: string[];
}

let sessionCounter = 0;

export function fakeMic(): MediaStream {
  const track = { enabled: true, stop: vi.fn(), kind: 'audio' } as unknown as MediaStreamTrack;
  return { getTracks: () => [track], getAudioTracks: () => [track] } as unknown as MediaStream;
}

export function join(
  server: SimServer,
  net: SimNet,
  user: SimUser,
  options: { device?: string; mic?: () => Promise<MediaStream> } = {},
): Rig {
  const factory = simFactory(net, user.userId);
  const mic = fakeMic();
  const sessionIds: string[] = [];
  const session = new VoiceSession({
    api: server.api(user),
    deviceId: options.device ?? `device-${user.userId}`,
    watching: false,
    acquireMic: options.mic ?? (async () => mic),
    watchSpeech: () => () => undefined,
    createPeer: factory.create,
    newSession: () => {
      sessionCounter += 1;
      const id = `s${sessionCounter}`;
      sessionIds.push(id);
      return id;
    },
    now: Date.now,
    onChange: () => undefined,
  });
  server.sessions.set(user.userId, [...(server.sessions.get(user.userId) ?? []), session]);
  return { server, net, session, peers: factory.made, user, mic, failures: [], sessionIds };
}

export async function settle(ms = 50): Promise<void> {
  await vi.advanceTimersByTimeAsync(ms);
}
