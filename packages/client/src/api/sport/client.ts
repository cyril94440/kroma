import type { RequestContext } from '../../core/client';
import modulesApi from '../modules/client';
import { ModuleId } from '../modules/ids';
import type { ModuleApi } from '../modules/module-api';
import { SPORT_MODULE, SPORT_ROUTES as R } from './ids';
import { Leaderboard, Ok, Opened, SportState, StandingsReply, Switched } from './schemas';

/** The sport module as a viewer reaches it: the matches on air with their
 *  channels, the live match panel, the pronos and the goal clips. */
export default function sportApi(ctx: RequestContext) {
  const api: ModuleApi = modulesApi(ctx).viewerApi(SPORT_MODULE);
  return {
    state: () => api.get(R.state, SportState),
    open: (force = false) => api.post(R.open, { force }, Opened),
    watch: (channelId: string) => api.post(R.watch, { channelId }, Switched),
    vote: (id: string, score: string) => api.post(R.pronoVote, { id, score }, Ok),
    leaderboard: () => api.get(R.leaderboard, Leaderboard),
    standings: (competition: string) =>
      api.get(`${R.standings}?competition=${encodeURIComponent(competition)}`, StandingsReply),
    clipUrl: (clip: { file: { module: string; path: string } }) =>
      modulesApi(ctx).fileUrl(ModuleId.parse(clip.file.module), clip.file.path),
  };
}

declare module '../../core/client' {
  interface Domains {
    sport: ReturnType<typeof sportApi>;
  }
}
