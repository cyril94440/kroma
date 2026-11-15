import type { ModuleApi } from '@kromatv/module-sdk';
import {
  Calibrated,
  Leaderboard,
  Ok,
  Opened,
  SportState,
  StandingsReply,
  Switched,
} from './schemas';

export function createSportApi(api: ModuleApi) {
  return {
    state: () => api.get('/state', SportState),
    open: (force = false) => api.post('/open', { force }, Opened),
    watch: (channelId: string) => api.post('/watch', { channelId }, Switched),
    vote: (id: string, score: string) => api.post('/prono/vote', { id, score }, Ok),
    leaderboard: () => api.get('/leaderboard', Leaderboard),
    standings: (competition: string) =>
      api.get(`/standings?competition=${encodeURIComponent(competition)}`, StandingsReply),
    calibrate: (clock: string, capturedAtMs: number | null) =>
      api.post('/calibration', { clock, capturedAtMs }, Calibrated),
  };
}

export type SportApi = ReturnType<typeof createSportApi>;
