// @vitest-environment jsdom

import { I18nProvider } from '@kromatv/ui';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterAll, afterEach, describe, expect, it, vi } from 'vitest';
import { WebRuntimeProvider } from '#web/shared/lib/web-runtime';
import { createWebTestRuntime } from '#web/shared/lib/web-runtime.fixture';
import { MissingEpisodeRow } from './episode-row';

vi.hoisted(() => {
  vi.stubEnv('NODE_ENV', 'development');
});

afterAll(() => {
  vi.unstubAllEnvs();
});

afterEach(cleanup);

function row(
  listed: Parameters<typeof MissingEpisodeRow>[0]['listed'],
  pending = false,
  watchNow?: Parameters<typeof MissingEpisodeRow>[0]['watchNow'],
) {
  return render(
    <WebRuntimeProvider value={createWebTestRuntime({})}>
      <I18nProvider locale="en">
        <MissingEpisodeRow
          season={1}
          episode={9}
          listed={listed}
          pending={pending}
          selected={false}
          onToggle={() => {}}
          watchNow={watchNow}
        />
      </I18nProvider>
    </WebRuntimeProvider>,
  );
}

describe('MissingEpisodeRow', () => {
  it('names a gap nothing is listed for by its number alone', () => {
    row(null);

    expect(screen.getByText('Episode 9')).toBeTruthy();
    expect(screen.getByText('9')).toBeTruthy();
    expect(screen.queryByText(/Unknown date/)).toBeNull();
  });

  it('shows the listed title, synopsis and the day it airs', () => {
    row({
      episode: 9,
      title: 'The Wedding',
      overview: 'Bert finds love.',
      airDate: '2031-03-14',
      stillUrl: '/api/images/still.webp',
    });

    expect(screen.getByText('9. The Wedding')).toBeTruthy();
    expect(screen.getByText('Bert finds love.')).toBeTruthy();
    expect(screen.getByText(/Mar 14, 2031/)).toBeTruthy();
    expect(screen.queryByText('9')).toBeNull();
  });

  it('shows the runtime the provider lists next to the day it airs', () => {
    row({
      episode: 9,
      title: 'The Wedding',
      overview: null,
      airDate: '2031-03-14',
      stillUrl: null,
      runtimeMin: 48,
    });

    expect(screen.getByText(/Mar 14, 2031 · 48min/)).toBeTruthy();
  });

  it('says the date is unknown when the listing has none', () => {
    row({ episode: 9, title: 'Untitled', airDate: null });

    expect(screen.getByText('Unknown date')).toBeTruthy();
  });

  it('keeps the listing on a row already asked for', () => {
    row({ episode: 9, title: 'The Wedding', airDate: '2031-03-14' }, true);

    expect(screen.getByText('9. The Wedding')).toBeTruthy();
    expect(screen.getByText(/2031/)).toBeTruthy();
  });

  it('offers no watch-now button unless the server does', () => {
    row(null);

    expect(screen.queryByLabelText('Watch this episode now')).toBeNull();
  });

  it('watches the episode now when its button is pressed', () => {
    const onWatch = vi.fn();
    row(null, false, { onWatch, disabled: false });

    fireEvent.click(screen.getByLabelText('Watch this episode now'));

    expect(onWatch).toHaveBeenCalledTimes(1);
  });

  it('holds the button while another search is running', () => {
    const onWatch = vi.fn();
    row(null, false, { onWatch, disabled: true });

    fireEvent.click(screen.getByLabelText('Watch this episode now'));

    expect(onWatch).not.toHaveBeenCalled();
  });
});
