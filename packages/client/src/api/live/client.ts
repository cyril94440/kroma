import type { RequestContext } from '../../core/client';
import modulesApi from '../modules/client';
import type { ModuleApi } from '../modules/module-api';
import { LIVE_MODULE, LIVE_ROUTES } from './ids';
import { BeatAnswer, ChannelPage, Favourites, Status, SwitchOutcome } from './schemas';

export interface ChannelQuery {
  q?: string;
  group?: string;
  provider?: string;
  page?: number;
  perPage?: number;
}

function withQuery(path: string, query: ChannelQuery): string {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value !== undefined && value !== '') params.set(key, String(value));
  }
  const text = params.toString();
  return text ? `${path}?${text}` : path;
}

/** The live-TV module as a viewer reaches it: what is on, who is watching, and
 *  the playlist to hand to a player. Every route answers 404 on a server where
 *  the module is not installed. */
export default function liveApi(ctx: RequestContext) {
  const api: ModuleApi = modulesApi(ctx).viewerApi(LIVE_MODULE);
  return {
    status: () => api.get(LIVE_ROUTES.status, Status),
    channels: (query: ChannelQuery = {}) =>
      api.get(withQuery(LIVE_ROUTES.channels, query), ChannelPage),
    favourites: () => api.get(LIVE_ROUTES.favourites, Favourites),
    beat: (companion = false) => api.post(LIVE_ROUTES.presence, { companion }, BeatAnswer),
    leave: () => api.beacon(LIVE_ROUTES.leave),
    switchTo: (channelId: string) => api.post(LIVE_ROUTES.switchTo, { channelId }, SwitchOutcome),
    playlistUrl: (playlist: string) => modulesApi(ctx).fileUrl(LIVE_MODULE, playlist),
  };
}

declare module '../../core/client' {
  interface Domains {
    live: ReturnType<typeof liveApi>;
  }
}
