import { ModuleId } from '../modules/ids';

/** The module this domain talks to. The coupling between a first-party screen
 *  and a module is this id and the route table beside it, and nothing else. */
export const LIVE_MODULE: ModuleId = ModuleId.parse('tv.kroma.live');

export const LIVE_ROUTES = {
  status: '/status',
  channels: '/channels',
  favourites: '/favourites',
  presence: '/presence',
  leave: '/presence/leave',
  switchTo: '/switch',
} as const;

export const LIVE_EVENT_PREFIX = 'live.';
