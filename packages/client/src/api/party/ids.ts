import { ModuleId } from '../modules/ids';

/** The module this domain talks to; see `LIVE_MODULE` for what that coupling is. */
export const PARTY_MODULE: ModuleId = ModuleId.parse('tv.kroma.party');

export const PARTY_ROUTES = {
  state: '/state',
  time: '/time',
  chat: '/chat',
  vote: '/vote',
  requestSwitch: '/request-switch',
  channels: '/channels',
  pollVote: '/poll/vote',
  pollStart: '/poll/start',
  react: '/react',
  floating: '/reaction',
  ideas: '/ideas',
  syncBeat: '/sync/beat',
  syncFineDone: '/sync/fine-done',
  syncRecalibration: '/sync/recalibration',
  stats: '/stats',
} as const;

export const PARTY_EVENT_PREFIX = 'party.';
