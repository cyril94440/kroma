import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { join, settle } from './voice-sim.fixture';
import { MAMIE, PAPI, room } from './voice-mesh-room.fixture';

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('a voice mesh whose connections or members change', () => {
  it('restarts ICE on a failed connection and comes back without rebuilding', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);
    const papi = join(server, net, PAPI);
    await mamie.session.join();
    await papi.session.join();
    await settle();

    mamie.peers[0]?.fail();
    await settle(500);

    expect(mamie.peers[0]?.restarts).toBe(1);
    expect(mamie.sessionIds.length).toBe(1);
    expect([...mamie.session.snapshot.links.values()]).toEqual(['connected']);
    expect(net.created).toBe(2);
  });

  it('waits out a short disconnect and only then restarts ICE', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);
    const papi = join(server, net, PAPI);
    await mamie.session.join();
    await papi.session.join();
    await settle();

    mamie.peers[0]?.drop();
    await settle(2_000);
    const early = mamie.peers[0]?.restarts;
    await settle(2_000);

    expect(early).toBe(0);
    expect(mamie.peers[0]?.restarts).toBe(1);
  });

  it('rebuilds the connection when an ICE restart does not bring it back', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);
    const papi = join(server, net, PAPI);
    await mamie.session.join();
    await papi.session.join();
    await settle();

    net.blocked = true;
    mamie.peers[0]?.fail();
    await settle(13_000);
    net.unblock();
    await settle(500);

    expect(mamie.sessionIds.length).toBe(2);
    expect(mamie.peers[0]?.restarts).toBe(1);
    expect([...mamie.session.snapshot.links.values()]).toEqual(['connected']);
  });

  it('closes the connection and its stream when the other person leaves mid-negotiation', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);
    const papi = join(server, net, PAPI);
    await mamie.session.join();
    await papi.session.join();

    await papi.session.leave();
    await settle(20_000);

    expect(mamie.session.snapshot.links.size).toBe(0);
    expect(mamie.session.snapshot.streams.size).toBe(0);
    expect(mamie.peers.at(-1)?.closed).toBe(true);
    expect(mamie.session.snapshot.participants.map((m) => m.userId)).toEqual(['mamie']);
  });

  it('does not reopen a connection to a peer the server said is gone', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);
    const papi = join(server, net, PAPI);
    await mamie.session.join();
    await papi.session.join();
    await settle();
    const stale = {
      type: 'party.voice.roster' as const,
      revision: server.revision + 5,
      participants: [...server.members],
    };
    server.forget(papi.session.snapshot.peerId ?? '');

    mamie.peers[0]?.fail();
    await settle(500);
    mamie.session.handleEvent(stale);
    await settle(20_000);

    expect(mamie.peers[0]?.closed).toBe(true);
    expect(net.created).toBe(2);
    expect(mamie.session.snapshot.streams.size).toBe(0);
  });

  it('repairs a lost roster event from the next heartbeat', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);
    const papi = join(server, net, PAPI);
    await mamie.session.join();
    await settle();
    server.dropIf = (_to, event) => event.type === 'party.voice.roster';

    await papi.session.join();
    await settle(11_000);

    expect(mamie.session.snapshot.participants.map((m) => m.userId)).toEqual(['mamie', 'papi']);
    expect([...mamie.session.snapshot.links.values()]).toEqual(['connected']);
  });

  it('parks a signal from an unknown peer, reads the roster and replays it', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);
    const papi = join(server, net, PAPI);
    await mamie.session.join();
    await settle();
    server.dropIf = (to, event) => to === 'mamie' && event.type === 'party.voice.roster';

    await papi.session.join();
    await settle(5_000);

    expect(mamie.session.snapshot.participants.map((m) => m.userId)).toEqual(['mamie', 'papi']);
    expect([...mamie.session.snapshot.links.values()]).toEqual(['connected']);
  });

  it('rejoins under a new peer id when the server forgot the participant', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);
    const papi = join(server, net, PAPI);
    await mamie.session.join();
    await papi.session.join();
    await settle();
    const before = mamie.session.snapshot.peerId;

    server.forget(before ?? '');
    await settle(11_000);

    expect(mamie.session.snapshot.phase).toBe('live');
    expect(mamie.session.snapshot.peerId).not.toBe(before);
    expect([...mamie.session.snapshot.links.values()]).toEqual(['connected']);
    expect([...papi.session.snapshot.links.values()]).toContain('connected');
  });

  it('replaces the entry of a tab that reloads instead of listing it twice', async () => {
    const { server, net } = room();
    const first = join(server, net, MAMIE, { device: 'device-tab' });
    await first.session.join();
    first.session.dispose();
    const reloaded = join(server, net, MAMIE, { device: 'device-tab' });

    await reloaded.session.join();

    expect(server.members.map((m) => m.userId)).toEqual(['mamie']);
  });
});

describe('two live tabs that share one device id', () => {
  it('stops fighting over the entry after a few rejoins instead of hammering the server', async () => {
    const { server, net } = room();
    const first = join(server, net, MAMIE, { device: 'device-tab' });
    const second = join(server, net, MAMIE, { device: 'device-tab' });

    await first.session.join();
    await second.session.join();
    await settle(30_000);

    const phases = [first.session.snapshot.phase, second.session.snapshot.phase].sort();
    expect(phases).toContain('idle');
    expect(first.session.snapshot.failure ?? second.session.snapshot.failure).toBe('failed');
  });
});
