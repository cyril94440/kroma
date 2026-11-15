// @vitest-environment jsdom

import { fakeClient } from '@kromatv/client/test';
import { createInputTestHost, onScreen } from '@kromatv/ui/testing';
import { act, cleanup, screen } from '@testing-library/react';
import { useEffect } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { TvClientProvider, type TvNav, TvNavProvider, TvOutlet, useNav } from '#tv/app/router';
import { stubScreens } from '#tv/app/router.fixtures';
import { fakeClipBackend } from '#tv/features/playback/player/clipBackend.fixture';
import { TvClipPlayer } from '#tv/features/playback/TvClipPlayer';

const { clearPressGuard, render } = createInputTestHost();

const clips = fakeClipBackend();

function ClipScreen() {
  return <TvClipPlayer backend={clips.backend} />;
}

let nav: TvNav;
function Capture() {
  const current = useNav();
  useEffect(() => {
    nav = current;
  });
  return null;
}

async function open() {
  render(
    onScreen(
      <TvClientProvider client={fakeClient({})}>
        <TvNavProvider screens={stubScreens({ clip: ClipScreen })}>
          <Capture />
          <TvOutlet />
        </TvNavProvider>
      </TvClientProvider>,
      { remote: true },
    ),
  );
  await act(async () =>
    nav.go('clip', { url: 'http://s/clip.mp4', title: "37' Kebbal", subtitle: 'Rennes 1-0 OM' }),
  );
}

afterEach(() => {
  cleanup();
  clearPressGuard();
  sessionStorage.clear();
  clips.built.length = 0;
});

describe('the clip screen', () => {
  it('plays the file it was handed', async () => {
    await open();

    expect(clips.built.map((clip) => clip.url)).toEqual(['http://s/clip.mp4']);
  });

  it('names the goal and its match', async () => {
    await open();

    expect(screen.getAllByText("37' Kebbal").length).toBeGreaterThan(0);
    expect(screen.getByText('Rennes 1-0 OM')).toBeTruthy();
  });

  it('leaves for the previous screen when the clip runs out', async () => {
    await open();

    await act(async () => clips.built[0]?.listeners.onEnded());

    expect(screen.getByText('screen:profiles')).toBeTruthy();
  });
});
