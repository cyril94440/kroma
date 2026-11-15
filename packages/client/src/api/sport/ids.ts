import { ModuleId } from '../modules/ids';

/** The module this domain talks to; see `LIVE_MODULE` for what that coupling is. */
export const SPORT_MODULE: ModuleId = ModuleId.parse('tv.kroma.sport');

export const SPORT_ROUTES = {
  state: '/state',
  open: '/open',
  watch: '/watch',
  pronoVote: '/prono/vote',
  leaderboard: '/leaderboard',
  standings: '/standings',
} as const;

export const SPORT_EVENT_PREFIX = 'sport.';
