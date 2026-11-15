import { describe, expect, it } from 'vitest';
import { publish, run } from './sync-sim.fixture';
import { party, settle, useSimulatedParties } from './sync-party.fixture';

useSimulatedParties();

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
    const drift = async (seconds: number) => {
      b.player.latency += 9;
      b.engine.graceAfterRestart();
      await run(seconds, p.server, p.clients);
      return b.player.seeks.length;
    };

    const counts = [await drift(30), await drift(30), await drift(8), await drift(15)];
    expect(counts).toEqual([1, 2, 2, 3]);
  });
});
