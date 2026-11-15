import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SimNet } from './voice-peer-sim.fixture';
import { join, settle, SimServer } from './voice-sim.fixture';

const MAMIE = { userId: 'mamie', name: 'Mamie' };
const PAPI = { userId: 'papi', name: 'Papi' };
const CYRIL = { userId: 'cyril', name: 'Cyril' };

function room() {
  const server = new SimServer();
  const net = new SimNet();
  return { server, net };
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('a voice mesh over a lossless signalling channel', () => {
  it('connects two people and hands each the other as a stream', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);
    const papi = join(server, net, PAPI);

    await mamie.session.join();
    await papi.session.join();
    await settle();

    expect([...mamie.session.snapshot.links.values()]).toEqual(['connected']);
    expect([...papi.session.snapshot.links.values()]).toEqual(['connected']);
    expect(mamie.session.snapshot.streams.size).toBe(1);
    expect(papi.session.snapshot.streams.size).toBe(1);
  });

  it('resolves the offer collision of a simultaneous join with one rollback', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);
    const papi = join(server, net, PAPI);

    await Promise.all([mamie.session.join(), papi.session.join()]);
    await settle();

    expect([...mamie.session.snapshot.links.values()]).toEqual(['connected']);
    expect([...papi.session.snapshot.links.values()]).toEqual(['connected']);
    expect(net.created).toBe(2);
  });

  it('connects three people into a full mesh of three links', async () => {
    const { server, net } = room();
    const rigs = [MAMIE, PAPI, CYRIL].map((user) => join(server, net, user));

    for (const rig of rigs) await rig.session.join();
    await settle();

    for (const rig of rigs) {
      expect([...rig.session.snapshot.links.values()]).toEqual(['connected', 'connected']);
    }
    expect(net.created).toBe(6);
  });

  it('never links the two devices of one account to each other', async () => {
    const { server, net } = room();
    const phone = join(server, net, MAMIE, { device: 'device-phone' });
    const laptop = join(server, net, MAMIE, { device: 'device-laptop' });
    const papi = join(server, net, PAPI);

    await phone.session.join();
    await laptop.session.join();
    await papi.session.join();
    await settle();

    expect(phone.session.snapshot.links.size).toBe(1);
    expect(laptop.session.snapshot.links.size).toBe(1);
    expect(papi.session.snapshot.links.size).toBe(2);
    expect(server.members.map((m) => m.userId)).toEqual(['mamie', 'mamie', 'papi']);
  });

  it('answers a signal meant for the other device of the account with silence', async () => {
    const { server, net } = room();
    const phone = join(server, net, MAMIE, { device: 'device-phone' });
    const laptop = join(server, net, MAMIE, { device: 'device-laptop' });
    const papi = join(server, net, PAPI);
    await phone.session.join();
    await laptop.session.join();
    await papi.session.join();
    await settle();

    const toPhone = server.delivered.filter(
      (d) => d.event.type === 'party.voice.signal' && d.event.to === phone.session.snapshot.peerId,
    );

    expect(toPhone.length).toBeGreaterThan(0);
    expect(laptop.session.snapshot.links.size).toBe(1);
  });
});

describe('a voice mesh that loses signalling messages', () => {
  it('resends an offer nobody answered until the pair connects', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);
    const papi = join(server, net, PAPI);
    let dropped = 0;
    server.dropIf = (_to, event) => event.kind === 'description' && dropped++ === 0;

    await mamie.session.join();
    await papi.session.join();
    await settle(6_000);

    expect(dropped).toBeGreaterThan(1);
    expect([...mamie.session.snapshot.links.values()]).toEqual(['connected']);
    expect([...papi.session.snapshot.links.values()]).toEqual(['connected']);
  });

  it('recovers from a lost answer without anyone leaving', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);
    const papi = join(server, net, PAPI);
    let answers = 0;
    server.dropIf = (_to, event) => {
      const description = event.description as { type?: string } | undefined;
      return description?.type === 'answer' && answers++ === 0;
    };

    await mamie.session.join();
    await papi.session.join();
    await settle(13_000);

    expect([...mamie.session.snapshot.links.values()]).toEqual(['connected']);
    expect([...papi.session.snapshot.links.values()]).toEqual(['connected']);
  });

  it('rebuilds the connection under a new session when every candidate is lost', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);
    const papi = join(server, net, PAPI);
    server.dropIf = (_to, event) => event.kind === 'candidate' && mamie.sessionIds.length < 2;

    await mamie.session.join();
    await papi.session.join();
    await settle(13_000);

    expect(mamie.sessionIds.length).toBe(2);
    expect([...mamie.session.snapshot.links.values()]).toEqual(['connected']);
    expect([...papi.session.snapshot.links.values()]).toEqual(['connected']);
  });

  it('keeps a connection that completed despite one lost candidate', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);
    const papi = join(server, net, PAPI);
    let candidates = 0;
    server.dropIf = (_to, event) => event.kind === 'candidate' && candidates++ === 0;

    await mamie.session.join();
    await papi.session.join();
    await settle(8_000);

    expect(mamie.sessionIds.length).toBe(1);
    expect(net.created).toBe(2);
    expect([...papi.session.snapshot.links.values()]).toEqual(['connected']);
  });

  it('asks the other side to start over when it never receives an offer', async () => {
    const { server, net } = room();
    const mamie = join(server, net, MAMIE);
    const papi = join(server, net, PAPI);
    server.dropIf = (_to, event) => event.kind === 'description' && mamie.sessionIds.length < 2;

    await mamie.session.join();
    await papi.session.join();
    await settle(5_000);

    expect(mamie.sessionIds.length).toBe(2);
    expect([...papi.session.snapshot.links.values()]).toEqual(['connected']);
  });
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
