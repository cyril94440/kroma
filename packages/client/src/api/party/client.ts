import type { RequestContext } from '../../core/client';
import modulesApi from '../modules/client';
import type { ModuleApi } from '../modules/module-api';
import { PARTY_MODULE, PARTY_ROUTES as R } from './ids';
import {
  Candidates,
  Created,
  type IdeaGenre,
  Ideas,
  Ok,
  PartyState,
  PollStarted,
  SentMessage,
  ServerTime,
  SwitchOutcome,
  type SyncBeatBody,
  type SyncFineBody,
  type SyncRecalibrationBody,
  type ViewerStatsBody,
} from './schemas';

/** The watch-party module as a viewer reaches it: the shared chat, the zapping
 *  vote, polls and reactions, and the clock the whole room syncs on. */
export default function partyApi(ctx: RequestContext) {
  const api: ModuleApi = modulesApi(ctx).viewerApi(PARTY_MODULE);
  return {
    state: () => api.get(R.state, PartyState),
    time: () => api.get(R.time, ServerTime),
    chat: (text: string) => api.post(R.chat, { text }, SentMessage),
    vote: (approve: boolean) => api.post(R.vote, { approve }, Ok),
    requestSwitch: (channelId: string, channelName?: string, force = false) =>
      api.post(R.requestSwitch, { channelId, channelName, force }, SwitchOutcome),
    channels: (query: string) =>
      api.get(`${R.channels}?q=${encodeURIComponent(query)}`, Candidates),
    pollVote: (value: string) => api.post(R.pollVote, { value }, Ok),
    pollStart: (question: string, kind: string, options: string[], durationS?: number) =>
      api.post(R.pollStart, { question, kind, options, durationS }, PollStarted),
    react: (id: string, emoji: string) => api.post(R.react, { id, emoji }, Ok),
    floating: (emoji: string) => api.post(R.floating, { emoji }, Ok),
    ideas: () => api.get(R.ideas, Ideas),
    submitIdea: (genre: IdeaGenre, text: string) => api.post(R.ideas, { genre, text }, Created),
    beat: (body: SyncBeatBody) => api.post(R.syncBeat, body, Ok),
    fineDone: (body: SyncFineBody) => api.post(R.syncFineDone, body, Ok),
    recalibration: (body: SyncRecalibrationBody) => api.post(R.syncRecalibration, body, Ok),
    stats: (body: ViewerStatsBody) => api.post(R.stats, body, Ok),
  };
}

declare module '../../core/client' {
  interface Domains {
    party: ReturnType<typeof partyApi>;
  }
}
