import { vi } from 'vitest';
import { SyncEngine, type SyncGroupView, type SyncNotice } from './sync-engine';
import type { MoveRange, PlayerSnapshot, SyncPlayer } from './sync-player';

export const SERVER_SKEW_MS = 500;

export class FakePlayer implements SyncPlayer {
  ready = true;
  measurable = true;
  seeks: number[] = [];
  rates: number[] = [];
  liveEdgeJumps = 0;
  landing = 0;
  constructor(
    public latency: number,
    public edge = 3,
    public window = 20,
  ) {}

  getPositionMs(): number | null {
    return this.measurable ? Date.now() + SERVER_SKEW_MS - this.latency * 1000 : null;
  }
  isReady(): boolean {
    return this.ready;
  }
  getBufferAheadMs(): number {
    return (this.latency - this.edge) * 1000;
  }
  getMoveRange(): MoveRange | null {
    return {
      backMs: (this.edge + this.window - this.latency) * 1000,
      forwardMs: (this.latency - (this.edge + 4)) * 1000,
    };
  }
  seekToLiveEdge(): void {
    this.liveEdgeJumps += 1;
    this.latency = this.edge + 4;
  }
  seekByMs(deltaMs: number): void {
    this.seeks.push(deltaMs);
    this.latency -= deltaMs / 1000 + this.landing;
  }
  setPlaybackRate(rate: number): void {
    this.rates.push(rate);
  }
  subscribe(_listener: (snapshot: PlayerSnapshot) => void): () => void {
    return () => {};
  }
}

interface Member {
  delay: number;
  paused: boolean;
  pinned: boolean;
  blocked: boolean;
  ts: number;
  pausedAt: number;
}

export class FakeServer {
  members = new Map<string, Member>();
  target = 0;
  by: string | null = null;
  goLiveAt = 0;
  freezeUntil = 0;
  recalibrations: { user: string; gap: number; arrival: boolean; blocked: boolean }[] = [];
  fineDone: { user: string; ms: number; seeks: number; abandoned: boolean; solo: boolean }[] = [];
  private stuck = new Map<string, number[]>();

  beat(
    user: string,
    body: { enabled: boolean; delay: number; paused: boolean; pinned: boolean; blocked: boolean },
  ) {
    const now = Date.now();
    if (!body.enabled) {
      this.members.delete(user);
      return;
    }
    const previous = this.members.get(user);
    this.members.set(user, {
      delay: body.delay,
      paused: body.paused,
      pinned: body.pinned,
      blocked: body.blocked,
      ts: now,
      pausedAt: body.paused ? now : (previous?.pausedAt ?? 0),
    });
  }

  fresh(m: Member): boolean {
    const now = Date.now();
    return now - m.ts <= 8000 && m.ts >= this.goLiveAt + 4000;
  }

  goLive(): void {
    this.target = 0;
    this.by = null;
    this.goLiveAt = Date.now();
    this.freezeUntil = 0;
  }

  fineSync(): void {
    const max = Math.max(
      0,
      ...[...this.members.values()].filter((m) => !m.paused && this.fresh(m)).map((m) => m.delay),
    );
    if (max > 0) this.target = Math.min(max + 0.15, 12);
    this.freezeUntil = Date.now() + 60_000;
  }

  tick(): SyncGroupView | null {
    const now = Date.now();
    for (const [id, m] of this.members) if (now - m.ts > 45_000) this.members.delete(id);
    if (this.members.size === 0) {
      this.target = 0;
      this.by = null;
      return null;
    }
    const live = [...this.members.entries()].filter(([, m]) => !m.paused && this.fresh(m));
    let maxD = 0;
    let maxUser: string | null = null;
    let minD = Infinity;
    for (const [id, m] of live) {
      if (m.delay > maxD) {
        maxD = m.delay;
        maxUser = id;
      }
      minD = Math.min(minD, m.delay);
    }
    if (maxUser !== null) this.steer(maxUser, maxD, minD, now);
    return this.view();
  }

  private steer(maxUser: string, maxD: number, minD: number, now: number): void {
    const recentResume = [...this.members.values()].some(
      (m) => !m.paused && this.fresh(m) && m.pausedAt > 0 && now - m.pausedAt < 30_000,
    );
    if (this.target >= 1 && now >= this.freezeUntil) {
      if (this.target < minD - 0.5 && !recentResume) this.target = Math.max(minD - 0.5, maxD - 1.5);
      else if (this.target > maxD + 0.5) this.target = maxD + 0.5;
    }
    if (maxD > this.target + 2.5) this.laggard(maxUser, maxD, now);
    else if (now >= this.freezeUntil) this.drain(maxD, minD, now);
  }

  private laggard(user: string, maxD: number, now: number): void {
    const m = this.members.get(user);
    const raised = Math.min(maxD, 12);
    const recentPause = (m?.pausedAt ?? 0) > 0 && now - (m?.pausedAt ?? 0) < 30_000;
    if (this.target < 1 || recentPause) {
      this.target = raised;
      this.by = user;
    } else if (raised - this.target >= 0.5 && this.isStuck(user, now)) {
      this.target = raised;
      this.by = user;
      this.stuck.delete(user);
    }
  }

  private isStuck(user: string, now: number): boolean {
    const m = this.members.get(user);
    const seen = (this.stuck.get(user) ?? []).filter((t) => now - t <= 20_000);
    if (m?.pinned || m?.blocked) seen.push(now);
    this.stuck.set(user, seen);
    return seen.length >= 5;
  }

  private drain(maxD: number, minD: number, now: number): void {
    const floor = [...this.members.values()]
      .filter((m) => !m.paused && this.fresh(m) && m.pinned)
      .reduce((acc, m) => Math.max(acc, m.delay), 0);
    let next = Math.max(Math.min(floor, 12), this.target - 0.1);
    if (next < maxD - 1.5) next = Math.min(this.target, maxD - 1.5);
    if (next < minD - 0.5) next = Math.min(this.target, minD - 0.5);
    this.target = next;
    if (this.by !== null && (this.target <= floor + 0.3 || this.target >= maxD - 1.6))
      this.by = null;
    void now;
  }

  view(): SyncGroupView {
    const by = this.by;
    return {
      target: Math.round(this.target * 100) / 100,
      by: by ? { name: by } : null,
      members: [...this.members.keys()],
      active: [...this.members.values()].filter((m) => this.fresh(m)).length,
    };
  }
}

interface Client {
  name: string;
  player: FakePlayer;
  engine: SyncEngine;
  notices: SyncNotice[];
}

export function connect(server: FakeServer, name: string, player: FakePlayer): Client {
  const notices: SyncNotice[] = [];
  const engine = new SyncEngine({
    player,
    transport: {
      beat: async (body) => server.beat(name, body),
      fineDone: async (body) => {
        server.fineDone.push({ user: name, ...body });
      },
      recalibration: async (body) => {
        server.recalibrations.push({ user: name, ...body });
      },
      serverNow: async () => Date.now() + SERVER_SKEW_MS,
    },
    notify: (n) => notices.push(n),
    peerDelays: async () =>
      [...server.members.entries()]
        .filter(([id, m]) => id !== name && !m.paused && server.fresh(m))
        .map(([, m]) => m.delay),
  });
  engine.start();
  return { name, player, engine, notices };
}

export function publish(server: FakeServer, clients: Client[]): void {
  const view = server.tick();
  for (const c of clients) c.engine.setGroup(view);
}

export async function run(
  seconds: number,
  server: FakeServer,
  clients: Client[],
  events: Record<number, () => void> = {},
): Promise<void> {
  for (let s = 0; s < seconds; s += 1) {
    const event = events[s];
    if (event) {
      event();
      for (const c of clients) c.engine.ping();
      publish(server, clients);
    }
    await vi.advanceTimersByTimeAsync(1000);
    if (s % 2 === 0) publish(server, clients);
  }
}

export const recalsOf = (server: FakeServer, user: string) =>
  server.recalibrations.filter((r) => r.user === user && !r.arrival).length;
