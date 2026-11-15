import { describe, expect, it, vi } from 'vitest';
import { connect, FakePlayer, publish, run } from './sync-sim.fixture';
import { party, settle, useSimulatedParties } from './sync-party.fixture';

useSimulatedParties();

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
