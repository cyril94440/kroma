// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { EventHandlers } from './live-events';
import { type LiveClient, useLiveRoom } from './use-live-room';

const STATUS = {
  state: 'running',
  channel: { id: 'p:1', name: 'TF1' },
  provider: 'primary',
  playlist: 'a/index.m3u8',
};

const PARTY = {
  seq: 5,
  now: 1,
  me: { userId: 'u1', name: 'Ana', admin: false },
  messages: [],
  vote: null,
  poll: null,
  lastPoll: null,
  activity: null,
  sync: null,
  presence: [],
  stats: {},
  config: { voteSeconds: 30, agentName: 'Hermes' },
};

function fixture() {
  let handlers: EventHandlers | null = null;
  const closed = vi.fn();
  const live = {
    status: vi.fn(async () => STATUS),
    beat: vi.fn(async () => ({ viewers: 1, heartbeatSeconds: 15 })),
    leave: vi.fn(),
  };
  const party = { state: vi.fn(async () => PARTY) };
  const sport = {
    state: vi.fn(async () => {
      throw new Error('absent');
    }),
  };
  const client = { live, party, sport } as unknown as LiveClient;
  const openEvents = vi.fn((next: EventHandlers) => {
    handlers = next;
    return closed;
  });
  return {
    client,
    live,
    party,
    sport,
    openEvents,
    closed,
    emit: (e: { type: string }) => handlers?.onEvent(e),
    reopen: () => handlers?.onOpen(),
  };
}

const ON = { live: true, party: true, sport: false };

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

async function mount(f: ReturnType<typeof fixture>, modules = ON, watching = true) {
  const hook = renderHook(() =>
    useLiveRoom({ client: f.client, openEvents: f.openEvents, modules, watching }),
  );
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
  return hook;
}

describe('a live screen', () => {
  it('loads the status and the party state, and never asks a module that is off', async () => {
    const f = fixture();
    const { result } = await mount(f);

    expect(result.current.status?.channel?.name).toBe('TF1');
    expect(result.current.party.state?.seq).toBe(5);
    expect(f.sport.state).not.toHaveBeenCalled();
  });

  it('announces presence every beat and says goodbye when it stops watching', async () => {
    const f = fixture();
    const { rerender, unmount } = renderHook(
      ({ watching }) =>
        useLiveRoom({ client: f.client, openEvents: f.openEvents, modules: ON, watching }),
      { initialProps: { watching: true } },
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(31_000);
    });
    expect(f.live.beat).toHaveBeenCalledTimes(3);

    rerender({ watching: false });
    expect(f.live.leave).toHaveBeenCalledTimes(1);
    unmount();
    expect(f.closed).toHaveBeenCalled();
  });

  it('does not announce presence when the live module is absent', async () => {
    const f = fixture();
    await mount(f, { live: false, party: false, sport: false });

    expect(f.live.beat).not.toHaveBeenCalled();
  });

  it('folds events in and refetches the party state when the sequence skips', async () => {
    const f = fixture();
    const { result } = await mount(f);
    const message = { id: 'm1', userId: 'u2', name: 'Bo', kind: 'user', text: 'hello', at: 1 };

    act(() => f.emit({ type: 'party.message', seq: 6, ...message } as never));
    expect(result.current.party.state?.messages).toHaveLength(1);

    f.party.state.mockClear();
    await act(async () => {
      f.emit({ type: 'party.message', seq: 9, ...message, id: 'm2' } as never);
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(f.party.state).toHaveBeenCalledTimes(1);
  });

  it('refetches everything when the socket reopens', async () => {
    const f = fixture();
    await mount(f);
    f.live.status.mockClear();

    await act(async () => {
      f.reopen();
      await vi.advanceTimersByTimeAsync(0);
    });

    expect(f.live.status).toHaveBeenCalledTimes(1);
  });

  it('updates the viewers from a presence event', async () => {
    const f = fixture();
    const { result } = await mount(f);

    act(() => f.emit({ type: 'live.presence', viewers: [{ userId: 'u1', name: 'Ana' }] } as never));

    expect(result.current.status?.viewers).toHaveLength(1);
  });
});
