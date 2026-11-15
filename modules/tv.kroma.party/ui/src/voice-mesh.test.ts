import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { join, settle } from './voice-sim.fixture';
import { CYRIL, MAMIE, PAPI, room } from './voice-mesh-room.fixture';

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
