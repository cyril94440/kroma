import { describe, expect, it } from 'vitest';
import { ChannelView, LiveEvent, Overview, ScanItem, ScanState, Status } from './schemas';

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

describe('the 0.2 additions', () => {
  it('reads the source resolution and defaults it to unknown', () => {
    expect(
      Status.parse({
        state: 'running',
        channel: null,
        provider: 'primary',
        sourceResolution: '1920x1080@25',
      }).sourceResolution,
    ).toBe('1920x1080@25');
    expect(
      Status.parse({ state: 'running', channel: null, provider: 'primary' }).sourceResolution,
    ).toBe('');
  });

  it('reads the favourites coverage and tolerates a gap code it does not know', () => {
    const overview = Overview.parse({
      providers: [],
      coverage: [
        { favourite: 'TF1', primary: 'FR: TF1', backup: ['TF1 HD'], guide: true, gap: null },
        { favourite: 'M6', primary: null, backup: [], guide: false, gap: 'absentFromBackup' },
        { favourite: 'Arte', primary: null, backup: [], guide: false, gap: 'somethingNew' },
      ],
    });
    expect(overview.coverage.map((row) => row.gap)).toEqual([null, 'absentFromBackup', null]);
  });

  it('keeps every captured frame of a scan item and the event a channel carries', () => {
    expect(
      ScanItem.parse({ id: 'a', name: 'A', frames: ['x/1.jpg', 'x/2.jpg'], still: true }).frames,
    ).toHaveLength(2);
    expect(
      ChannelView.parse({ id: 'a', name: 'A', event: { search: 'psg', competition: 'Ligue 1' } })
        .event?.competition,
    ).toBe('Ligue 1');
    expect(ChannelView.parse({ id: 'a', name: 'A' }).event).toBeNull();
  });
});
