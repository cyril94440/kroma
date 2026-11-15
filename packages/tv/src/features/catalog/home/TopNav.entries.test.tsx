// @vitest-environment jsdom
import { Focusable } from '@kromatv/ui/kit';
import { onScreen as baseOnScreen, wearsRing } from '@kromatv/ui/testing';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { TvNavProvider, useNav } from '#tv/app/router';
import { type NavEntry, NavEntriesProvider } from '#tv/shared/nav/navEntries';

const onScreen = (ui: Parameters<typeof baseOnScreen>[0]) => baseOnScreen(ui, { remote: true });

vi.mock('#tv/app/providers/auth', () => ({
  useAuth: () => ({ user: { id: 'u1', username: 'Max' } }),
}));
vi.mock('#tv/app/providers/connection', () => ({
  useConnection: () => ({ client: null, online: true }),
}));
vi.mock('#tv/features/cast/CastRemotes', () => ({ CastRemotes: () => null }));

const { TvTopNav } = await import('#tv/features/catalog/home/TopNav');

afterEach(cleanup);

const LIVE: NavEntry = { key: 'live', icon: 'broadcast', label: 'nav.live', route: 'live' };

function Route() {
  return <span data-testid="route">{useNav().route.name}</span>;
}

function bar(entries: readonly NavEntry[]) {
  render(
    onScreen(
      <TvNavProvider screens={{} as never}>
        <NavEntriesProvider entries={entries}>
          <TvTopNav />
          <Route />
        </NavEntriesProvider>
      </TvNavProvider>,
    ),
  );
}

async function press(key: string) {
  await act(async () => {
    document.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
  });
}

describe('the live entry of the browse top bar', () => {
  it('takes its place between Home and Movies even when it arrives after the bar', async () => {
    const view = render(
      onScreen(
        <TvNavProvider screens={{} as never}>
          <NavEntriesProvider entries={[]}>
            <TvTopNav />
            <Focusable label="Content" autoFocus />
          </NavEntriesProvider>
        </TvNavProvider>,
      ),
    );
    view.rerender(
      onScreen(
        <TvNavProvider screens={{} as never}>
          <NavEntriesProvider entries={[LIVE]}>
            <TvTopNav />
            <Focusable label="Content" autoFocus />
          </NavEntriesProvider>
        </TvNavProvider>,
      ),
    );

    await press('ArrowUp');
    await press('ArrowRight');
    await press('ArrowRight');

    expect(wearsRing(screen.getByLabelText('Live'))).toBe(true);
    expect(wearsRing(screen.getByLabelText('Movies'))).toBe(false);
  });

  it('is absent when nothing offers it', () => {
    bar([]);

    expect(screen.queryByLabelText('Live')).toBeNull();
    expect(screen.getByLabelText('Home')).toBeTruthy();
  });

  it('appears when the app offers it and opens the live screen', async () => {
    bar([LIVE]);

    await act(async () => {
      fireEvent.click(screen.getByLabelText('Live'));
    });

    expect(screen.getByTestId('route').textContent).toBe('live');
  });
});
