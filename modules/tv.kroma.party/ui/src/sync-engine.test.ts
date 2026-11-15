import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  connect,
  FakePlayer,
  FakeServer,
  publish,
  recalsOf,
  run,
  SERVER_SKEW_MS,
} from './sync-sim.fixture';

beforeEach(() => {
  vi.useFakeTimers({ now: 1_000_000_000_000 });
});
const parties: { clients: { engine: { stop(): void } }[] }[] = [];

afterEach(() => {
  for (const p of parties.splice(0)) for (const c of p.clients) c.engine.stop();
  vi.useRealTimers();
});

function party(latencies: Record<string, number>, edge = 3) {
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

async function settle(p: ReturnType<typeof party>, seconds = 30) {
  await run(seconds, p.server, p.clients);
}

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
    const p = party({ A: 6, B: 6.2 });
    await run(120, p.server, p.clients, {
      60: () => p.clients.forEach((c) => (c.player.latency += 3)),
    });

    expect(p.server.recalibrations).toEqual([]);
    expect(p.clients.every((c) => c.player.seeks.length === 0)).toBe(true);
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

describe('recalibration', () => {
  it('seeks back by exactly the measured gap and tells the server why', async () => {
    const p = party({ A: 6, B: 6 });
    await settle(p, 20);
    p.clients[1]!.player.edge = -10;
    p.clients[1]!.player.latency += 9;
    p.clients[1]!.engine.graceAfterRestart();
    await run(60, p.server, p.clients);

    const b = p.clients[1]!;
    expect(b.player.seeks.length).toBe(1);
    expect(b.player.seeks[0]).toBeGreaterThan(7000);
    expect(p.server.recalibrations.at(-1)).toMatchObject({
      user: 'B',
      arrival: false,
      blocked: false,
    });
    expect(p.server.recalibrations.at(-1)!.gap).toBeLessThan(-7);
    expect(b.notices.some((n) => n.key === 'party.sync.toast.recalibrated')).toBe(true);
  });

  it('decides nothing while rebuffering, during the grace after a restart, or while paused', async () => {
    const p = party({ A: 6, B: 6 });
    await settle(p, 20);
    const b = p.clients[1]!;
    b.player.edge = -10;
    b.player.latency += 9;

    b.player.ready = false;
    await run(20, p.server, p.clients);
    b.player.ready = true;
    b.engine.setPlaying(false);
    await run(20, p.server, p.clients);
    expect(b.player.seeks).toEqual([]);
  });

  it('waits for the group to come to it right after resuming from a pause', async () => {
    const p = party({ A: 6, B: 6 });
    await settle(p, 20);
    const b = p.clients[1]!;
    b.engine.setPlaying(false);
    await run(6, p.server, p.clients);
    b.player.latency += 5;
    b.engine.setPlaying(true);
    p.server.target = 6;
    publish(p.server, p.clients);
    b.engine.group = { ...b.engine.group!, target: 6 };
    b.engine.control();

    expect(b.player.seeks).toEqual([]);
  });

  it('signals a blocked recalibration instead of seeking when the window forbids it', async () => {
    const p = party({ A: 6, B: 6 });
    await settle(p, 20);
    const b = p.clients[1]!;
    b.player.edge = 9;
    b.player.latency = 13;
    b.engine.graceAfterRestart();
    await run(40, p.server, p.clients);

    expect(b.player.seeks).toEqual([]);
    expect(p.server.recalibrations.some((r) => r.user === 'B' && r.blocked)).toBe(true);
    expect(p.server.members.get('B')?.blocked).toBe(true);
  });

  it('doubles the wait when a recalibration comes back within a minute', async () => {
    const p = party({ A: 6, B: 6 });
    await settle(p, 20);
    const b = p.clients[1]!;
    b.player.edge = -50;
    for (let i = 0; i < 3; i += 1) {
      b.player.latency += 9;
      b.engine.graceAfterRestart();
      await run(i === 0 ? 30 : 45, p.server, p.clients);
    }

    expect(b.player.seeks.length).toBeLessThanOrEqual(2);
  });
});

describe('joining', () => {
  it('snaps to the known target at once instead of converging for a minute', async () => {
    const p = party({ A: 8, B: 8 });
    await settle(p, 20);
    const late = connect(p.server, 'C', new FakePlayer(3, -10));
    await vi.advanceTimersByTimeAsync(3000);
    p.clients.push(late);
    late.engine.setGroup(p.server.view());
    late.engine.join();

    expect(late.player.latency).toBeCloseTo(p.server.target, 0);
    expect(p.server.recalibrations.at(-1)).toMatchObject({ user: 'C', arrival: true });
    late.engine.stop();
  });

  it('does not snap when already within a third of a second, or when the group is not formed', async () => {
    const p = party({ A: 8 });
    await settle(p, 10);
    const near = connect(p.server, 'C', new FakePlayer(p.server.target + 0.2, -10));
    near.engine.setGroup(p.server.view());
    near.engine.join();
    expect(near.player.seeks).toEqual([]);
    near.engine.stop();

    const fresh = connect(p.server, 'D', new FakePlayer(3));
    fresh.engine.setGroup({ target: 0, by: null, members: [], active: 0 });
    fresh.engine.join();
    expect(fresh.player.seeks).toEqual([]);
    fresh.engine.stop();
  });

  it('ignores a target that describes players that are gone', async () => {
    const p = party({ A: 8 });
    const lone = connect(p.server, 'C', new FakePlayer(3));
    lone.engine.setGroup({ target: 9, by: null, members: ['x'], active: 0 });
    lone.engine.join();
    expect(lone.player.seeks).toEqual([]);
    lone.engine.stop();
  });
});

describe('group commands', () => {
  it('go live jumps every member to its own live edge and abandons a running fine sync', async () => {
    const p = party({ A: 8, B: 8 });
    await settle(p, 20);
    const [a] = p.clients;
    a!.engine.goLive();

    expect(a!.player.liveEdgeJumps).toBe(1);
    expect(a!.notices.at(-1)?.key).toBe('party.sync.toast.allLive');
  });

  it('being kicked leaves the group and stops sending beats', async () => {
    const p = party({ A: 8, B: 8 });
    await settle(p, 10);
    const [a] = p.clients;
    a!.engine.kicked('requested');
    await vi.advanceTimersByTimeAsync(10_000);

    expect(a!.engine.on).toBe(false);
    expect(p.server.members.has('A')).toBe(false);
  });
});

describe('fine sync', () => {
  it('brings two players within a frame of each other, learning the seek landing bias', async () => {
    const p = party({ A: 6, B: 6.3 }, 1);
    for (const c of p.clients) c.player.landing = 0.06;
    await settle(p, 10);
    p.server.fineSync();
    publish(p.server, p.clients);
    for (const c of p.clients) c.engine.runFineSync();
    await run(60, p.server, p.clients);

    const [a, b] = p.clients;
    expect(Math.abs(a!.player.latency - b!.player.latency)).toBeLessThan(0.03);
    const results = p.server.fineDone.filter((r) => !r.abandoned);
    expect(results.length).toBe(2);
    expect(results.every((r) => r.ms <= 30 && r.seeks >= 1 && !r.solo)).toBe(true);
    expect(a!.player.rates).toEqual([]);
  });

  it('a player already in place confirms on two windows without moving', async () => {
    const p = party({ A: 6.45, B: 6.45 }, 1);
    await settle(p, 10);
    p.server.target = 6.45;
    p.server.freezeUntil = Date.now() + 60_000;
    publish(p.server, p.clients);
    p.clients[0]!.engine.runFineSync();
    await run(15, p.server, p.clients);

    expect(p.clients[0]!.player.seeks).toEqual([]);
    expect(p.server.fineDone.find((r) => r.user === 'A')).toMatchObject({
      seeks: 0,
      abandoned: false,
    });
  });

  it('reports an abandon, without a made-up gap, when the player pauses mid-campaign', async () => {
    const p = party({ A: 6, B: 6.3 }, 1);
    await settle(p, 10);
    p.server.fineSync();
    publish(p.server, p.clients);
    p.clients[1]!.engine.runFineSync();
    await vi.advanceTimersByTimeAsync(1500);
    p.clients[1]!.engine.setPlaying(false);
    await vi.advanceTimersByTimeAsync(5000);

    expect(p.server.fineDone.filter((r) => !r.solo)).toEqual([
      expect.objectContaining({ user: 'B', abandoned: true, solo: false }),
    ]);
  });

  it('a go live in the middle of a campaign cancels it without a report', async () => {
    const p = party({ A: 6, B: 6.3 });
    await settle(p, 10);
    p.server.fineSync();
    publish(p.server, p.clients);
    p.clients[1]!.engine.runFineSync();
    await vi.advanceTimersByTimeAsync(1500);
    p.clients[1]!.engine.goLive();
    await vi.advanceTimersByTimeAsync(20_000);

    expect(p.server.fineDone.filter((r) => !r.solo)).toEqual([]);
    expect(p.clients[1]!.engine.fine.active).toBe(false);
  });

  it('on arrival, aligns on its own only and reports as solo', async () => {
    const p = party({ A: 6, B: 6.3 });
    await settle(p, 10);
    const [a, b] = p.clients;
    const before = a!.player.seeks.length;
    await run(30, p.server, p.clients);

    expect(a!.player.seeks.length).toBe(before);
    expect(p.server.fineDone.every((r) => r.solo)).toBe(true);
    expect(b!.engine.fine.active).toBe(false);
  });

  it('gives up with the best gap when the target lies outside the player window', async () => {
    const p = party({ A: 6, B: 6 }, 1);
    await settle(p, 10);
    const b = p.clients[1]!;
    b.player.edge = 2.5;
    b.player.latency = 6.5;
    p.server.fineSync();
    p.server.target = 3;
    b.engine.setGroup({ ...p.server.view(), target: 3 });
    b.engine.runFineSync();
    await vi.advanceTimersByTimeAsync(30_000);

    expect(b.notices.some((n) => n.key === 'party.sync.fine.outOfWindow')).toBe(true);
    expect(p.server.fineDone.find((r) => r.user === 'B')).toMatchObject({ abandoned: false });
  });
});
