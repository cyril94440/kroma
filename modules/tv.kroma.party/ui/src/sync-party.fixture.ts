import { afterEach, beforeEach, vi } from 'vitest';
import { connect, FakePlayer, FakeServer, run } from './sync-sim.fixture';

export function useSimulatedParties(): void {
  beforeEach(() => {
    vi.useFakeTimers({ now: 1_000_000_000_000 });
  });
  afterEach(() => {
    for (const p of parties.splice(0)) for (const c of p.clients) c.engine.stop();
    vi.useRealTimers();
  });
}

const parties: { clients: { engine: { stop(): void } }[] }[] = [];

export function party(latencies: Record<string, number>, edge = 3) {
  const server = new FakeServer();
  const clients = Object.entries(latencies).map(([name, latency]) =>
    connect(server, name, new FakePlayer(latency, edge)),
  );
  for (const c of clients) {
    c.engine.graceAfterRestart();
    c.engine.join();
  }
  const made = { server, clients };
  parties.push(made);
  return made;
}

export async function settle(p: ReturnType<typeof party>, seconds = 30) {
  await run(seconds, p.server, p.clients);
}
