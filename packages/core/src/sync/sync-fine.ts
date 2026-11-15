import * as K from './sync-constants';
import type { SyncEngine } from './sync-engine';
import { clampMove } from './sync-player';

const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

type Step = 'continue' | 'stop';

class Campaign {
  seeks = 0;
  finished = false;
  private confirmed = 0;
  private blocked = 0;
  private best = Infinity;
  private landBias = 0;
  private justSeeked = false;
  private lastErr: number | null = null;

  constructor(
    private readonly engine: SyncEngine,
    private readonly solo: boolean,
    private readonly reference: number | null,
    private readonly locked: number | null,
    private readonly isCurrent: () => boolean,
  ) {}

  get abandonable(): boolean {
    return !this.finished && !this.solo && this.isCurrent();
  }

  async run(): Promise<void> {
    await this.engine.measureClock();
    const started = performance.now();
    while (performance.now() - started < K.FINE_BUDGET_MS) {
      if (this.referenceLost()) return;
      const err = await this.sample();
      if (err === 'aborted') return;
      if (err === null) continue;
      if ((await this.step(err)) === 'stop') return;
    }
    if (this.lastErr === null) this.engine.deps.notify({ key: 'live.sync.fine.noMeasure' });
    else this.finish('live.sync.fine.budget', Math.min(this.best, this.lastErr));
  }

  private aborted(): boolean {
    return !this.engine.on || !this.engine.playing || !this.isCurrent();
  }

  private referenceLost(): boolean {
    const group = this.engine.group;
    if (!this.solo) return false;
    return (
      !group ||
      this.reference === null ||
      Math.abs(group.target - this.reference) > K.FINE_REFERENCE_DRIFT_S
    );
  }

  private goal(): number | null {
    const group = this.engine.group;
    if (!group || group.target <= K.TARGET_FLOOR_S) return null;
    return this.solo ? this.locked : group.target;
  }

  private async sample(): Promise<number | null | 'aborted'> {
    const samples: number[] = [];
    for (let i = 0; i < K.FINE_SAMPLES; i += 1) {
      await sleep(K.FINE_SAMPLE_GAP_MS);
      if (this.aborted()) return 'aborted';
      const latency = this.engine.player.isReady() ? this.engine.absLatency() : null;
      const goal = this.goal();
      if (latency !== null && goal !== null) samples.push(goal - latency);
    }
    if (samples.length < K.FINE_MIN_SAMPLES) return null;
    samples.sort((a, b) => a - b);
    return samples[Math.floor(samples.length / 2)] ?? 0;
  }

  private async step(err: number): Promise<Step> {
    this.lastErr = Math.abs(err);
    this.learnLanding(err);
    if (this.lastErr <= K.FINE_FRAME_S) {
      this.confirmed += 1;
      if (this.confirmed < K.FINE_CONFIRMATIONS) return 'continue';
      this.finish('live.sync.fine.done', this.lastErr);
      return 'stop';
    }
    this.confirmed = 0;
    if (this.seeks > 0 && this.lastErr < this.best - 0.01) this.best = this.lastErr;
    return this.correct(err);
  }

  private learnLanding(err: number): void {
    if (!this.justSeeked) return;
    const next = this.landBias - err * K.FINE_BIAS_GAIN;
    this.landBias = Math.max(-K.FINE_BIAS_CAP_S, Math.min(K.FINE_BIAS_CAP_S, next));
    this.justSeeked = false;
  }

  private async correct(err: number): Promise<Step> {
    const wanted = (-err + this.landBias) * 1000;
    const applied = clampMove(wanted, this.engine.player.getMoveRange());
    if (Math.abs(applied - wanted) <= 50) {
      this.blocked = 0;
      return this.seek(applied);
    }
    if (applied > wanted) {
      if (applied < -100) return this.seek(applied);
      await sleep(1500);
      return 'continue';
    }
    this.blocked += 1;
    if (this.blocked >= K.FINE_BLOCKED_LIMIT) {
      this.finish('live.sync.fine.outOfWindow', Math.min(this.best, this.lastErr ?? this.best));
      return 'stop';
    }
    await sleep(1000);
    return 'continue';
  }

  private async seek(deltaMs: number): Promise<Step> {
    this.engine.player.seekByMs(deltaMs);
    this.seeks += 1;
    this.justSeeked = true;
    this.engine.errEma = 0;
    for (let w = 0; w < K.FINE_READY_POLLS && !this.engine.player.isReady(); w += 1) {
      await sleep(K.FINE_READY_POLL_MS);
      if (this.aborted()) return 'stop';
    }
    await sleep(K.FINE_SETTLE_MS);
    return this.aborted() ? 'stop' : 'continue';
  }

  private finish(key: string, errS: number): void {
    this.finished = true;
    const ms = Math.max(1, Math.round(errS * 1000));
    this.engine.deps.notify({ key, vars: { ms } });
    void this.engine.deps.transport
      .fineDone({ ms, seeks: this.seeks, abandoned: false, solo: this.solo })
      .catch(() => undefined);
  }
}

export class FineSync {
  active = false;
  private generation = 0;
  private lastSoloAt = -1e9;

  constructor(private readonly engine: SyncEngine) {}

  cancel(): void {
    this.generation += 1;
  }

  async run(solo: boolean): Promise<void> {
    if (!this.engine.on || !(await this.admit(solo))) return;
    const group = this.engine.group;
    const reference = group && group.target > K.TARGET_FLOOR_S ? group.target : null;
    const locked = solo ? await this.lockedTarget(reference) : reference;
    if (solo && (locked === null || performance.now() - this.lastSoloAt < K.FINE_SOLO_GAP_MS))
      return;
    if (solo) this.lastSoloAt = performance.now();
    this.active = true;
    this.engine.deps.notify({ key: 'live.sync.fine.running', ms: K.FINE_BUDGET_MS + 1000 });
    const generation = this.generation;
    const campaign = new Campaign(
      this.engine,
      solo,
      reference,
      locked,
      () => generation === this.generation,
    );
    try {
      await campaign.run();
    } finally {
      this.active = false;
      if (campaign.abandonable) {
        void this.engine.deps.transport
          .fineDone({ ms: 0, seeks: campaign.seeks, abandoned: true, solo })
          .catch(() => undefined);
      }
    }
  }

  private async admit(solo: boolean): Promise<boolean> {
    if (!this.active) return true;
    if (solo) return false;
    this.generation += 1;
    await sleep(K.FINE_PREEMPT_WAIT_MS);
    return !this.active;
  }

  private async lockedTarget(reference: number | null): Promise<number | null> {
    const peers = [...((await this.engine.deps.peerDelays?.()) ?? [])].sort((a, b) => a - b);
    return peers.length > 0 ? (peers[Math.floor(peers.length / 2)] ?? reference) : reference;
  }
}
