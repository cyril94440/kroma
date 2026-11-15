import { lazy } from 'react';

/** The live screen as its own chunk, so the browse-first bundle stays lean. */
export const BROWSER_LIVE_SCREEN = lazy(() =>
  import('#tv/features/live/browser/BrowserLive').then((m) => ({ default: m.BrowserLive })),
);
