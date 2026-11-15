// @vitest-environment jsdom
import type { SyncState } from '@kromatv/client/party';
import type { LiveSync } from '@kromatv/core/react';
import { onScreen } from '@kromatv/ui/testing';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('#mobile/lib/i18n', async () => {
  const { english } = await import('#mobile/live/i18n.fixture');
  return { useT: () => english };
});

const { LiveSyncPanel } = await import('#mobile/live/LiveSync');

afterEach(cleanup);

const group: SyncState = {
  target: 12,
  by: { userId: 'papi', name: 'Papi' },
  capped: false,
  members: [
    { userId: 'papi', name: 'Papi' },
    { userId: 'me', name: 'Me' },
  ],
  active: 2,
};
const sync = (on: boolean): LiveSync => ({ on, notice: null, join: vi.fn(), leave: vi.fn() });

describe('the phone watch party panel', () => {
  it('offers to join and says the picture is not synchronised', () => {
    const idle = sync(false);
    render(onScreen(<LiveSyncPanel sync={idle} group={null} />));

    fireEvent.click(screen.getByRole('button', { name: 'Sync with the group' }));

    expect([
      vi.mocked(idle.join).mock.calls.length,
      screen.queryByText('Your picture is not synchronised.') !== null,
    ]).toEqual([1, true]);
  });

  it('says who the group waits for once joined, and offers to leave', () => {
    const joined = sync(true);
    render(onScreen(<LiveSyncPanel sync={joined} group={group} />));

    fireEvent.click(screen.getByRole('button', { name: 'Leave the sync' }));

    expect([
      vi.mocked(joined.leave).mock.calls.length,
      screen.queryByText('Synced: 2 in the group · waiting for Papi') !== null,
    ]).toEqual([1, true]);
  });
});
