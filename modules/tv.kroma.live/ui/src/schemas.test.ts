import { describe, expect, it } from 'vitest';
import { LiveEvent, ScanState, Status } from './schemas';

const RUNNING = {
  state: 'running',
  channel: { id: 'p:1', name: 'TF1' },
  provider: 'primary',
  playlist: 'abc/index.m3u8',
};

describe('the status the server sends', () => {
  it('fills what a newer or older server leaves out', () => {
    const status = Status.parse(RUNNING);

    expect(status.viewers).toEqual([]);
    expect(status.health.sourceKbps).toBe(0);
    expect(status.onBackup).toBe(false);
  });

  it('ignores fields it does not know and refuses an unknown state', () => {
    expect(Status.parse({ ...RUNNING, somethingNew: 1 }).state).toBe('running');
    expect(Status.safeParse({ ...RUNNING, state: 'exploded' }).success).toBe(false);
  });
});

describe('the events', () => {
  it('reads a status event as the status plus its type', () => {
    const event = LiveEvent.parse({ ...RUNNING, type: 'live.status' });

    expect(event.type).toBe('live.status');
  });

  it('refuses an event of another module', () => {
    expect(LiveEvent.safeParse({ type: 'party.chat' }).success).toBe(false);
  });
});

describe('a scan', () => {
  it('keeps each captured channel with its picture and verdict', () => {
    const scan = ScanState.parse({
      running: true,
      total: 2,
      items: [{ id: 'p:1', name: 'A', ok: true, frame: 'scan-x/1.jpg' }],
    });

    expect(scan.items[0]?.black).toBe(false);
    expect(scan.items[0]?.frame).toBe('scan-x/1.jpg');
  });
});
