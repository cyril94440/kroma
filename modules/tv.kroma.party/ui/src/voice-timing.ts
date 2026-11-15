export const LINK_TIMING = {
  connectTimeoutMs: 12_000,
  offerResendMs: 5_000,
  maxOfferResends: 2,
  gapCheckMs: 3_000,
  disconnectGraceMs: 3_000,
  helloAfterMs: 4_000,
  helloCooldownMs: 3_000,
  maxIceRestarts: 2,
  maxRecoveries: 5,
  sendRetryMs: [500, 1_500],
} as const;

export const SESSION_TIMING = {
  speakingRefreshMs: 2_000,
  rosterWaitMs: 3_000,
  pendingSignalsMax: 32,
  joinRetryMs: 2_000,
  rejoinsMax: 3,
  rejoinWindowMs: 60_000,
  iceRefreshMs: 6 * 3_600_000,
} as const;

export class Timers {
  private readonly pending = new Map<string, ReturnType<typeof setTimeout>>();

  arm(name: string, ms: number, run: () => void): void {
    this.clear(name);
    this.pending.set(
      name,
      setTimeout(() => {
        this.pending.delete(name);
        run();
      }, ms),
    );
  }

  has(name: string): boolean {
    return this.pending.has(name);
  }

  clear(name: string): void {
    clearTimeout(this.pending.get(name));
    this.pending.delete(name);
  }

  clearAll(): void {
    for (const timer of this.pending.values()) clearTimeout(timer);
    this.pending.clear();
  }
}
