// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { EMPTY_PARTY, type PartyModel } from '../live/party-state';
import { type SyncSource, useLiveSync } from './use-live-sync';

const group = {
  target: 12,
  by: { userId: 'papi', name: 'Papi' },
  capped: false,
  members: [{ userId: 'papi', name: 'Papi' }],
  active: 1,
};

function setup(party: PartyModel = EMPTY_PARTY) {
  const delay = { seconds: 12 };
  const source: SyncSource = {
    frameTimeMs: () => Date.now() - delay.seconds * 1000,
    isReady: () => true,
    bufferAheadMs: () => 9000,
    moveRange: () => ({ backMs: 20_000, forwardMs: 8000 }),
    goLive: vi.fn(),
    seekByMs: vi.fn(),
    setRate: vi.fn(),
  };
  const party$ = {
    time: vi.fn(async () => ({ now: Date.now() })),
    beat: vi.fn(async () => ({ ok: true })),
    fineDone: vi.fn(async () => ({ ok: true })),
    recalibration: vi.fn(async () => ({ ok: true })),
    stats: vi.fn(async () => ({ ok: true })),
    state: vi.fn(async () => {
      throw new Error('offline');
    }),
  };
  const client = { party: party$ } as unknown as Parameters<typeof useLiveSync>[0]['client'];
  const ref = { current: source };
  const view = renderHook(
    (props: { party: PartyModel; playing: boolean }) =>
      useLiveSync({ client, source: ref, channelId: 'tf1', ...props }),
    { initialProps: { party, playing: true } },
  );
  return { view, source, party$ };
}

const withGroup = (extra: Partial<PartyModel> = {}): PartyModel => ({
  ...EMPTY_PARTY,
  state: {
    seq: 1,
    now: 0,
    me: { userId: 'me', name: 'Me', admin: false },
    messages: [],
    vote: null,
    poll: null,
    lastPoll: null,
    activity: null,
    sync: group,
    presence: [],
    stats: {},
    config: { voteSeconds: 30, agentName: 'Hermès' },
  },
  ...extra,
});

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('the watch party sync', () => {
  it('stays off until the viewer joins, then beats as a member', async () => {
    const { view, party$ } = setup(withGroup());

    expect(view.result.current.on).toBe(false);
    act(() => view.result.current.join());
    await act(() => vi.advanceTimersByTimeAsync(10));

    expect(view.result.current.on).toBe(true);
    expect(view.result.current.notice?.key).toBe('live.sync.toast.joined');
    expect(party$.beat).toHaveBeenCalledWith(expect.objectContaining({ enabled: true, paused: false }));
  });

  it('says goodbye to the room when the viewer leaves', async () => {
    const { view, party$ } = setup(withGroup());
    act(() => view.result.current.join());
    await act(() => vi.advanceTimersByTimeAsync(10));

    act(() => view.result.current.leave());

    expect(view.result.current.on).toBe(false);
    expect(view.result.current.notice?.key).toBe('live.sync.toast.left');
    expect(party$.beat).toHaveBeenLastCalledWith({
      enabled: false,
      delay: 0,
      paused: false,
      pinned: false,
      blocked: false,
    });
  });

  it('puts the picture back on the live edge when the room goes live', async () => {
    const { view, source } = setup(withGroup());
    act(() => view.result.current.join());
    await act(() => vi.advanceTimersByTimeAsync(10));

    view.rerender({ party: withGroup({ goLiveAt: 5 }), playing: true });

    expect(source.goLive).toHaveBeenCalled();
    expect(view.result.current.notice?.key).toBe('live.sync.toast.allLive');
  });

  it('leaves the sync and says why when the room removes the viewer', async () => {
    const { view } = setup(withGroup());
    act(() => view.result.current.join());
    await act(() => vi.advanceTimersByTimeAsync(10));

    view.rerender({ party: withGroup({ kicked: 'requested', kickedAt: 7 }), playing: true });

    expect(view.result.current.on).toBe(false);
    expect(view.result.current.notice).toMatchObject({
      key: 'live.sync.toast.kicked',
      vars: { reason: 'requested' },
    });
  });

  it('shows the fine alignment the engine starts a few seconds after joining', async () => {
    const { view } = setup(withGroup());
    act(() => view.result.current.join());

    await act(() => vi.advanceTimersByTimeAsync(3500));

    expect(view.result.current.notice?.key).toBe('live.sync.fine.running');
  });

  it('reports the viewer stats on a timer', async () => {
    const { party$ } = setup(withGroup());

    await act(() => vi.advanceTimersByTimeAsync(8100));

    expect(party$.stats).toHaveBeenCalledWith(expect.objectContaining({ playing: true, buffer: 9 }));
  });
});
