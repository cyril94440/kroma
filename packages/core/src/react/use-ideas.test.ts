// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { useIdeas } from './use-ideas';
import type { LiveClient } from './use-live-room';

const idea = {
  id: 'i1',
  ts: 1,
  genre: 'bug' as const,
  text: 'no sound',
  status: 'inProgress' as const,
  reply: null,
  author: { userId: 'u', name: 'Mamie' },
  updatedAt: 2,
};

describe('useIdeas', () => {
  it('reads the box again when its version moves', async () => {
    const ideas = vi.fn(async () => ({ ideas: [idea] }));
    const client = { party: { ideas } } as unknown as LiveClient;
    const view = renderHook((p: { version: number }) => useIdeas(client, p.version, true), {
      initialProps: { version: 0 },
    });
    await act(async () => undefined);

    view.rerender({ version: 4 });
    await act(async () => undefined);

    expect(view.result.current.ideas).toEqual([idea]);
    expect(ideas).toHaveBeenCalledTimes(2);
  });

  it('keeps what it shows when a read fails', async () => {
    const ideas = vi.fn().mockResolvedValueOnce({ ideas: [idea] }).mockRejectedValueOnce(new Error('x'));
    const client = { party: { ideas } } as unknown as LiveClient;
    const view = renderHook((p: { version: number }) => useIdeas(client, p.version, true), {
      initialProps: { version: 0 },
    });
    await act(async () => undefined);

    view.rerender({ version: 1 });
    await act(async () => undefined);

    expect(view.result.current.ideas).toHaveLength(1);
  });

  it('does not read while disabled', async () => {
    const ideas = vi.fn();
    renderHook(() => useIdeas({ party: { ideas } } as unknown as LiveClient, 0, false));
    await act(async () => undefined);

    expect(ideas).not.toHaveBeenCalled();
  });
});
