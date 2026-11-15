import { describe, expect, it, vi } from 'vitest';
import { recalsOf, run, SERVER_SKEW_MS } from './sync-sim.fixture';
import { party, settle, useSimulatedParties } from './sync-party.fixture';

useSimulatedParties();

describe('absolute latency', () => {
  it('measures the frame on screen against the server clock, not the local one', async () => {
    const p = party({ A: 6 });
    await vi.advanceTimersByTimeAsync(3000);

    expect(p.clients[0]?.engine.absLatency()).toBeCloseTo(6, 2);
    expect(SERVER_SKEW_MS).toBeGreaterThan(0);
  });

  it('keeps the last reading for ten seconds, then has none', async () => {
    const p = party({ A: 6 });
    await vi.advanceTimersByTimeAsync(3000);
    const [a] = p.clients;
    expect(a?.engine.syncDelay()).toBeCloseTo(6, 2);

    a!.player.measurable = false;
    await vi.advanceTimersByTimeAsync(9000);
    expect(a?.engine.syncDelay()).toBeCloseTo(6, 2);
    await vi.advanceTimersByTimeAsync(2000);
    expect(a?.engine.syncDelay()).toBeNull();
  });
});

describe('the beat', () => {
  it('reports delay, pause and the missing safety margin, and skips when nothing is measurable', async () => {
    const p = party({ A: 6, B: 5 });
    p.clients[1]!.player.edge = 2.5;
    await vi.advanceTimersByTimeAsync(2100);

    expect(p.server.members.get('A')).toMatchObject({ paused: false, pinned: true });
    expect(p.server.members.get('B')?.pinned).toBe(true);
    expect(p.server.members.get('A')?.delay).toBeCloseTo(6, 2);

    const before = p.server.members.get('A')!.ts;
    p.clients[0]!.player.measurable = false;
    await vi.advanceTimersByTimeAsync(12_000);
    expect(p.server.members.get('A')!.ts).toBeGreaterThanOrEqual(before);
    expect(p.server.members.get('A')!.ts).toBeLessThan(Date.now() - 1500);
  });

  it('sends paused while the player is paused and withdraws when leaving', async () => {
    const p = party({ A: 6 });
    const [a] = p.clients;
    a!.engine.setPlaying(false);
    await vi.advanceTimersByTimeAsync(10);
    expect(p.server.members.get('A')?.paused).toBe(true);

    a!.engine.leave();
    await vi.advanceTimersByTimeAsync(10);
    expect(p.server.members.has('A')).toBe(false);
    expect(a!.player.rates).toEqual([1]);
  });
});

describe('simulation: the group steers, the clients move only when truly drifting', () => {
  it('a hiccup both players share moves nobody', async () => {
    const p = party({ A: 6, B: 6.2 }, 1);
    await run(120, p.server, p.clients, {
      60: () => p.clients.forEach((c) => (c.player.latency += 3)),
    });

    expect(p.server.recalibrations).toEqual([]);
    expect(p.clients.every((c) => c.player.seeks.every((s) => Math.abs(s) <= 500))).toBe(true);
  });

  it('a return to live shared by the group sends nobody back', async () => {
    const p = party({ A: 11, B: 11 });
    await run(120, p.server, p.clients, {
      60: () => {
        p.clients[0]!.player.latency = 6;
        p.clients[1]!.player.latency = 6.3;
      },
    });

    expect(p.server.recalibrations).toEqual([]);
    expect(p.server.target).toBeLessThanOrEqual(6.9);
  });

  it('players that keep restarting do not dissolve the party nor get recalibrated', async () => {
    const p = party({ A: 6, B: 6 });
    const events: Record<number, () => void> = {};
    for (let i = 0; i < 12; i += 1) {
      const at = 30 + i * 20;
      events[at] = () =>
        p.clients.forEach((c, k) => {
          c.player.measurable = false;
          c.player.ready = false;
          setTimeout(
            () => {
              c.player.latency = 6 + ((i + k) % 2) * 0.5;
              c.player.measurable = true;
              c.player.ready = true;
              c.engine.graceAfterRestart();
            },
            (10 + 2 * k) * 1000,
          );
        });
    }
    await run(300, p.server, p.clients, events);

    expect(p.server.recalibrations).toEqual([]);
    expect(p.server.members.size).toBe(2);
  });

  it('one frozen player alone catches up, once', async () => {
    const p = party({ A: 6, B: 6 });
    p.clients[1]!.player.edge = -10;
    await run(160, p.server, p.clients, { 60: () => (p.clients[1]!.player.latency += 10) });

    expect(recalsOf(p.server, 'A')).toBe(0);
    expect(recalsOf(p.server, 'B')).toBe(1);
    expect(p.clients[1]!.player.latency).toBeCloseTo(p.clients[0]!.player.latency, 0);
  });

  it('a straggler stuck at the edge of its window is not retried forever; the group comes to it', async () => {
    const p = party({ A: 6, B: 6 });
    const [a, b] = p.clients;
    b!.player.edge = 2;
    await run(400, p.server, p.clients, {
      30: () => {
        b!.player.latency = 10.5;
        b!.player.edge = 6.5;
      },
    });

    expect(recalsOf(p.server, 'B')).toBeLessThanOrEqual(2);
    expect(recalsOf(p.server, 'A')).toBe(1);
    expect(Math.abs(a!.player.latency - b!.player.latency)).toBeLessThan(1);
  });

  it('never touches the playback rate while following a group', async () => {
    const p = party({ A: 6, B: 20 });
    await settle(p, 120);

    expect(p.clients.flatMap((c) => c.player.rates)).toEqual([]);
  });
});
