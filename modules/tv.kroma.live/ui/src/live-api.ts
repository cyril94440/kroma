import type { ModuleApi } from '@kromatv/module-sdk';
import {
  BeatAnswer,
  ChannelPage,
  Favourites,
  History,
  Metrics,
  Overview,
  Refreshed,
  ScanState,
  Started,
  Status,
  SwitchOutcome,
} from './schemas';

type Query = Record<string, string | number | undefined>;

function withQuery(path: string, query?: Query): string {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query ?? {})) {
    if (value !== undefined && value !== '') params.set(key, String(value));
  }
  const text = params.toString();
  return text ? `${path}?${text}` : path;
}

export function createLiveApi(api: ModuleApi, fileUrl: (path: string) => string) {
  return {
    status: () => api.get('/status', Status),
    channels: (query: { q?: string; group?: string; provider?: string; page?: number }) =>
      api.get(withQuery('/channels', { ...query, perPage: 48 }), ChannelPage),
    favourites: () => api.get('/favourites', Favourites),
    beat: (companion = false) => api.post('/presence', { companion }, BeatAnswer),
    leave: () => api.beacon('/presence/leave'),
    switchTo: (channelId: string) => api.post('/switch', { channelId }, SwitchOutcome),
    pause: () => api.post('/pause'),
    resume: () => api.post('/resume', undefined, SwitchOutcome),
    overview: () => api.get('/overview', Overview),
    history: (minutes = 60) => api.get(withQuery('/history', { minutes }), History),
    metrics: () => api.get('/metrics', Metrics),
    refresh: () => api.post('/refresh', undefined, Refreshed),
    startScan: (query: string, provider: string) => api.post('/scan', { query, provider }, Started),
    scan: () => api.get('/scan', ScanState),
    fileUrl,
  };
}

export type LiveApi = ReturnType<typeof createLiveApi>;
