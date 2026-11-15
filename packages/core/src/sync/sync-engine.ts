import type { MessageKey } from '@kromatv/i18n';
import { type ClockSample, measureOffset } from './clock-offset';
import * as K from './sync-constants';
import { FineSync } from './sync-fine';
import { clampMove, type SyncPlayer } from './sync-player';

export interface SyncGroupView {
  target: number;
  by: { name: string } | null;
  members: readonly unknown[];
  active: number;
}

export interface SyncTransport {
  beat(body: {
    enabled: boolean;
    delay: number;
    paused: boolean;
    pinned: boolean;
    blocked: boolean;
  }): Promise<unknown>;
  fineDone(body: {
    ms: number;
    seeks: number;
    abandoned: boolean;
    solo: boolean;
  }): Promise<unknown>;
  recalibration(body: {
    gap: number;
    target: number;
    arrival: boolean;
    blocked: boolean;
  }): Promise<unknown>;
  serverNow(): Promise<number>;
}

export interface SyncNotice {
  key: MessageKey;
  vars?: Record<string, string | number>;
  ms?: number;
}

export interface SyncDeps {
  player: SyncPlayer;
  transport: SyncTransport;
  notify: (notice: SyncNotice) => void;
  peerDelays?: () => Promise<number[]>;
  onChange?: () => void;
}

export class SyncEngine {
  on = false;
  group: SyncGroupView | null = null;
  playing = true;
  readonly fine: FineSync;
  errEma = 0;
  lastCorrectionAt = -1e9;
  blockedUntil = 0;
  private clockOffset = 0;
  private clockBestRtt = Infinity;
  private lastLatency: number | null = null;
  private lastLatencyAt = 0;
  private waitMs: number = K.WAIT_BASE_MS;
  private graceUntil = 0;
  private resumeGraceUntil = 0;
  private timers: ReturnType<typeof setInterval>[] = [];
  private pending: ReturnType<typeof setTimeout>[] = [];

  constructor(readonly deps: SyncDeps) {
    this.fine = new FineSync(this);
  }

  get player(): SyncPlayer {
    return this.deps.player;
  }

  start(): void {
    this.timers = [
      setInterval(() => this.ping(), K.PING_EVERY_MS),
      setInterval(() => this.control(), K.CONTROL_EVERY_MS),
      setInterval(() => this.refreshClock(), K.CLOCK_REFRESH_MS),
    ];
    void this.measureClock();
  }

  stop(): void {
    for (const timer of this.timers) clearInterval(timer);
    for (const timer of this.pending) clearTimeout(timer);
    this.timers = [];
    this.pending = [];
    this.fine.cancel();
  }

  serverNow(): number {
    return Date.now() + this.clockOffset;
  }

  async measureClock(): Promise<void> {
    const best = await measureOffset(
      () => this.deps.transport.serverNow(),
      K.CLOCK_BURST,
      Date.now,
      K.CLOCK_BURST_GAP_MS,
    );
    if (best) this.adoptClock(best);
  }

  private adoptClock(sample: ClockSample): void {
    if (sample.rtt > this.clockBestRtt * 1.2 + 5) return;
    this.clockOffset = sample.offset;
    this.clockBestRtt = Math.min(this.clockBestRtt, sample.rtt);
  }

  private refreshClock(): void {
    if (this.fine.active) return;
    this.clockBestRtt = Math.min(this.clockBestRtt * 1.5, 3000);
    void this.measureClock();
  }

  absLatency(): number | null {
    const position = this.player.getPositionMs();
    return position === null ? null : Math.max(0, (this.serverNow() - position) / 1000);
  }

  syncDelay(): number | null {
    const latency = this.absLatency();
    if (latency !== null) {
      this.lastLatency = latency;
      this.lastLatencyAt = performance.now();
      return latency;
    }
    return performance.now() - this.lastLatencyAt < K.LATENCY_CACHE_MS ? this.lastLatency : null;
  }

  setGroup(group: SyncGroupView | null): void {
    this.group = group;
    this.deps.onChange?.();
  }

  setPlaying(playing: boolean): void {
    if (playing === this.playing) return;
    this.playing = playing;
    if (!this.on) return;
    if (playing) this.resumeGraceUntil = performance.now() + K.RESUME_GRACE_MS;
    this.ping();
  }

  graceAfterRestart(): void {
    this.graceUntil = performance.now() + K.GRACE_MS;
    this.errEma = 0;
  }

  ping(): void {
    if (!this.on) return;
    const delay = this.syncDelay();
    if (delay === null) return;
    void this.deps.transport
      .beat({
        enabled: true,
        delay: Math.round(delay * 1000) / 1000,
        paused: !this.playing,
        pinned: this.player.getBufferAheadMs() < K.BUFFER_PINNED_MS,
        blocked: performance.now() < this.blockedUntil,
      })
      .catch(() => undefined);
  }

  join(): void {
    if (this.on) return;
    this.on = true;
    this.snapToGroup();
    this.ping();
    this.later(() => void this.fine.run(true), K.FINE_SOLO_FIRST_MS);
    this.later(() => void this.fine.run(true), K.FINE_SOLO_SECOND_MS);
    this.deps.onChange?.();
  }

  leave(): void {
    if (!this.on) return;
    this.on = false;
    this.fine.cancel();
    this.player.setPlaybackRate(1);
    void this.deps.transport
      .beat({ enabled: false, delay: 0, paused: false, pinned: false, blocked: false })
      .catch(() => undefined);
    this.deps.onChange?.();
  }

  kicked(reason: string): void {
    this.leave();
    this.deps.notify({ key: 'live.sync.toast.kicked', vars: { reason }, ms: 8000 });
  }

  goLive(): void {
    this.fine.cancel();
    if (!this.on) return;
    this.player.seekToLiveEdge();
    this.errEma = 0;
    this.deps.notify({ key: 'live.sync.toast.allLive' });
  }

  runFineSync(): void {
    if (this.on) void this.fine.run(false);
  }

  report(gap: number, target: number, arrival: boolean, blocked: boolean): void {
    void this.deps.transport
      .recalibration({ gap: Math.round(gap * 100) / 100, target, arrival, blocked })
      .catch(() => undefined);
  }

  seekPlan(gapS: number): { applied: number; wanted: number; blocked: boolean; moves: boolean } {
    const wanted = -gapS * 1000;
    const applied = clampMove(wanted, this.player.getMoveRange());
    return {
      applied,
      wanted,
      blocked: Math.abs(applied - wanted) > K.MIN_SEEK_S * 1000,
      moves: Math.abs(applied) >= K.MIN_SEEK_S * 1000 && Math.sign(applied) === Math.sign(wanted),
    };
  }

  snapToGroup(): void {
    const group = this.group;
    if (!group || group.members.length === 0 || group.target <= K.TARGET_FLOOR_S) return;
    if (group.active === 0) return;
    const delay = this.syncDelay();
    if (delay === null) return;
    const gap = group.target - delay;
    if (Math.abs(gap) < K.SNAP_TOLERANCE_S) return;
    const plan = this.seekPlan(gap);
    if (plan.blocked) this.blockedUntil = performance.now() + K.BLOCKED_FLAG_MS;
    if (!plan.moves) return;
    this.player.seekByMs(plan.applied);
    this.errEma = 0;
    this.lastCorrectionAt = performance.now();
    this.report(gap, group.target, true, plan.blocked);
  }

  control(): void {
    const group = this.group;
    if (!this.on || !group || !this.playing || !this.player.isReady() || this.fine.active) return;
    if (performance.now() < this.graceUntil) return;
    if (group.target <= K.TARGET_FLOOR_S) {
      this.errEma = 0;
      return;
    }
    const delay = this.syncDelay();
    if (delay === null) return;
    const gap = group.target - delay;
    this.errEma = this.errEma * K.EMA_KEEP + gap * (1 - K.EMA_KEEP);
    if (Math.abs(this.errEma) < K.DRIFT_THRESHOLD_S || Math.abs(gap) < K.DRIFT_THRESHOLD_S) return;
    if (gap < 0 && performance.now() < this.resumeGraceUntil) return;
    const elapsed = performance.now() - this.lastCorrectionAt;
    if (elapsed < this.waitMs) return;
    const fresh = this.absLatency();
    if (fresh === null) return;
    const freshGap = group.target - fresh;
    if (Math.abs(freshGap) < K.DRIFT_THRESHOLD_S) return;
    const plan = this.seekPlan(freshGap);
    if (plan.blocked) this.blockedUntil = performance.now() + K.BLOCKED_FLAG_MS;
    this.waitMs =
      elapsed < this.waitMs + K.WAIT_RESET_MS
        ? Math.min(this.waitMs * 2, K.WAIT_MAX_MS)
        : K.WAIT_BASE_MS;
    this.lastCorrectionAt = performance.now();
    this.errEma = 0;
    if (!plan.moves) {
      this.report(freshGap, group.target, false, true);
      return;
    }
    this.player.seekByMs(plan.applied);
    this.report(freshGap, group.target, false, plan.blocked);
    this.deps.notify({
      key: 'live.sync.toast.recalibrated',
      vars: { name: group.by?.name ?? '' },
    });
  }

  later(run: () => void, ms: number): void {
    this.pending.push(setTimeout(run, ms));
  }
}
