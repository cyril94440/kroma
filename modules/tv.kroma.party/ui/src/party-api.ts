import type { ModuleApi } from '@kromatv/module-sdk';
import {
  Candidates,
  Created,
  Ideas,
  Ok,
  PartyState,
  PollStarted,
  SentMessage,
  ServerTime,
  SwitchOutcome,
} from './schemas';

export function createPartyApi(api: ModuleApi) {
  const get = api.get;
  const post = api.post;
  return {
    state: () => get('/state', PartyState),
    time: () => get('/time', ServerTime),
    chat: (text: string) => post('/chat', { text }, SentMessage),
    vote: (approve: boolean) => post('/vote', { approve }, Ok),
    requestSwitch: (channelId: string, channelName?: string, force = false) =>
      post('/request-switch', { channelId, channelName, force }, SwitchOutcome),
    channels: (query: string) => get(`/channels?q=${encodeURIComponent(query)}`, Candidates),
    pollVote: (value: string) => post('/poll/vote', { value }, Ok),
    pollStart: (question: string, kind: string, options: string[], durationS?: number) =>
      post('/poll/start', { question, kind, options, durationS }, PollStarted),
    pollClose: () => post('/poll/close', {}, Ok.loose()),
    react: (id: string, emoji: string) => post('/react', { id, emoji }, Ok),
    floating: (emoji: string) => post('/reaction', { emoji }, Ok),
    ideas: () => get('/ideas', Ideas),
    submitIdea: (genre: string, text: string) => post('/ideas', { genre, text }, Created),
    updateIdea: (id: string, status?: string, reply?: string) =>
      post(`/ideas/${encodeURIComponent(id)}`, { status, reply }, Ok.loose()),
    goLive: () => post('/sync/go-live', {}, Ok.loose()),
    fineSync: () => post('/sync/fine-sync', {}, Ok),
    beat: (body: {
      enabled: boolean;
      delay: number;
      paused: boolean;
      pinned: boolean;
      blocked: boolean;
    }) => api.post('/sync/beat', body),
    fineDone: (body: { ms: number; seeks: number; abandoned: boolean; solo: boolean }) =>
      api.post('/sync/fine-done', body),
    recalibration: (body: { gap: number; target: number; arrival: boolean; blocked: boolean }) =>
      api.post('/sync/recalibration', body),
    stats: (body: {
      buffer: number;
      bandwidthKbps: number;
      stalls: number;
      rebufferMs: number;
      dropped: number;
      resolution: string;
      playing: boolean;
      delay: number | null;
      paused: boolean;
    }) => api.post('/stats', body),
    kick: (userId: string) => post('/sync/kick', { userId, reason: '' }, Ok.loose()),
  };
}

export type PartyApi = ReturnType<typeof createPartyApi>;
