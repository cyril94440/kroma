// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { I18nProvider } from '#ui/services/i18n';
import { LiveBar } from './live-bar';

afterEach(cleanup);

function bar(behind: boolean, onGoLive = vi.fn()) {
  render(
    <I18nProvider locale="en">
      <LiveBar behind={behind} onGoLive={onGoLive} />
    </I18nProvider>,
  );
  return onGoLive;
}

describe('LiveBar', () => {
  it('says the picture is live while it is at the edge', () => {
    bar(false);

    expect(screen.getByText('Live')).toBeTruthy();
    expect(screen.queryByText('Back to live')).toBeNull();
  });

  it('offers the way back once the picture is behind, and takes it on press', () => {
    const onGoLive = bar(true);

    fireEvent.click(screen.getByText('Back to live'));

    expect(onGoLive).toHaveBeenCalledTimes(1);
  });
});
