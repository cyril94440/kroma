import { describe, expect, it } from 'vitest';
import { ChannelPage, LiveEvent, Status } from './schemas';

const RUNNING = {
  state: 'running',
  channel: { id: 'p:1', name: 'TF1' },
  provider: 'primary',
  playlist: 'abc/index.m3u8',
};

describe('the status the live module sends', () => {
  it('fills what a newer or older server leaves out', () => {
    const status = Status.parse(RUNNING);

    expect(status.viewers).toEqual([]);
    expect(status.health.sourceKbps).toBe(0);
    expect(status.onBackup).toBe(false);
    expect(status.sourceResolution).toBe('');
  });

  it('ignores fields it does not know and refuses an unknown state', () => {
    expect(Status.parse({ ...RUNNING, somethingNew: 1 }).state).toBe('running');
    expect(Status.safeParse({ ...RUNNING, state: 'exploded' }).success).toBe(false);
  });

  it('keeps the guide slots in milliseconds', () => {
    const status = Status.parse({
      ...RUNNING,
      now: { title: 'Journal', start: 1_700_000_000_000, end: 1_700_000_600_000 },
    });

    expect(status.now?.end).toBe(1_700_000_600_000);
    expect(status.next).toBeNull();
  });
});

describe('the channel list', () => {
  it('reads a page with its groups and tolerates a missing guide event', () => {
    const page = ChannelPage.parse({
      total: 1,
      page: 1,
      perPage: 48,
      channels: [{ id: 'p:1', name: 'TF1', group: 'FR', event: 'nope' }],
    });

    expect(page.channels[0]?.event).toBeNull();
    expect(page.groups).toEqual([]);
  });
});

describe('the live events', () => {
  it('reads a status event as the status plus its type', () => {
    expect(LiveEvent.parse({ ...RUNNING, type: 'live.status' }).type).toBe('live.status');
  });

  it('reads a presence event as the viewers present', () => {
    const event = LiveEvent.parse({
      type: 'live.presence',
      viewers: [{ userId: 'u1', name: 'Ana' }],
    });

    expect(event.type === 'live.presence' && event.viewers[0]?.since).toBe(0);
  });

  it('refuses an event of another module', () => {
    expect(LiveEvent.safeParse({ type: 'party.message' }).success).toBe(false);
  });
});
