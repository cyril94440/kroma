// @vitest-environment jsdom
import type { LiveSync } from '@kromatv/core/react';
import { createInputTestHost, onScreen } from '@kromatv/ui/testing';
import { cleanup, fireEvent, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { LiveActionsBar } from '#tv/features/live/LiveActionsBar';

const { clearPressGuard, render } = createInputTestHost();

afterEach(() => {
  cleanup();
  clearPressGuard();
});

const sync = (on: boolean): LiveSync => ({ on, notice: null, join: vi.fn(), leave: vi.fn() });
const names = () => screen.getAllByRole('button').map((b) => b.getAttribute('aria-label'));

describe('the live action bar', () => {
  it('offers only the channels on a server with the live module alone', () => {
    render(
      onScreen(
        <LiveActionsBar
          modules={{ live: true, party: false, sport: false }}
          hasMatch={false}
          sync={sync(false)}
          onOpen={vi.fn()}
        />,
      ),
    );

    expect(names()).toEqual(['Channels']);
  });

  it('adds the party and sport buttons with their modules', () => {
    render(
      onScreen(
        <LiveActionsBar
          modules={{ live: true, party: true, sport: true }}
          hasMatch
          sync={sync(false)}
          onOpen={vi.fn()}
        />,
      ),
    );

    expect(names()).toEqual([
      'Channels',
      'Chat',
      'Reactions',
      'Sync with the group',
      'Match',
      'Live sport',
      'Ideas box',
    ]);
  });

  it('joins the sync, then offers to leave it', () => {
    const idle = sync(false);
    const { rerender } = render(
      onScreen(
        <LiveActionsBar
          modules={{ live: true, party: true, sport: false }}
          hasMatch={false}
          sync={idle}
          onOpen={vi.fn()}
        />,
      ),
    );
    clearPressGuard();
    fireEvent.click(screen.getByRole('button', { name: 'Sync with the group' }));
    const joined = sync(true);
    rerender(
      onScreen(
        <LiveActionsBar
          modules={{ live: true, party: true, sport: false }}
          hasMatch={false}
          sync={joined}
          onOpen={vi.fn()}
        />,
      ),
    );
    clearPressGuard();
    fireEvent.click(screen.getByRole('button', { name: 'Leave the sync' }));

    expect([idle.join, joined.leave].map((fn) => vi.mocked(fn).mock.calls.length)).toEqual([1, 1]);
  });
});
