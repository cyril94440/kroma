// @vitest-environment jsdom

import type { WatchNowState } from '@kromatv/core/react';
import { I18nProvider } from '@kromatv/ui';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterAll, afterEach, describe, expect, it, vi } from 'vitest';
import { WatchNowCta, watchNowSettled } from './watch-now-cta';

vi.hoisted(() => {
  vi.stubEnv('NODE_ENV', 'development');
});

afterAll(() => {
  vi.unstubAllEnvs();
});

afterEach(cleanup);

function cta(state: WatchNowState, onWatch = () => {}) {
  return render(
    <I18nProvider locale="en">
      <WatchNowCta state={state} onWatch={onWatch} />
    </I18nProvider>,
  );
}

describe('WatchNowCta', () => {
  it('offers watch now at rest and asks when pressed', () => {
    const onWatch = vi.fn();
    cta({ phase: 'idle' }, onWatch);

    fireEvent.click(screen.getByText('Watch now'));

    expect(onWatch).toHaveBeenCalledTimes(1);
  });

  it('says it is finding the best copy while it searches, with no percentage', () => {
    cta({ phase: 'searching' });

    expect(screen.getByText('Finding the best copy')).toBeTruthy();
    expect(screen.queryByText(/%/)).toBeNull();
  });

  it('says the title was requested and will be announced', () => {
    cta({ phase: 'requested' });

    expect(screen.getByText(/Requested\. We will let you know/)).toBeTruthy();
  });

  it('gives the servers own words when it cannot', () => {
    cta({ phase: 'unavailable', reason: 'noRelease', message: 'Nothing fits your settings.' });

    expect(screen.getByText('Nothing fits your settings.')).toBeTruthy();
  });

  it('says the connection dropped and lets the viewer ask again', () => {
    const onWatch = vi.fn();
    cta({ phase: 'error' }, onWatch);

    fireEvent.click(screen.getByText('Watch now'));

    expect(screen.getByText(/The connection dropped/)).toBeTruthy();
    expect(onWatch).toHaveBeenCalledTimes(1);
  });
});

describe('watchNowSettled', () => {
  it('is true once the title was requested or the server explained', () => {
    expect(watchNowSettled({ phase: 'requested' })).toBe(true);
    expect(watchNowSettled({ phase: 'unavailable', reason: 'noEngine', message: 'x' })).toBe(true);
    expect(watchNowSettled({ phase: 'idle' })).toBe(false);
    expect(watchNowSettled({ phase: 'error' })).toBe(false);
  });
});
