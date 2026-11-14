import { loadSession } from '@kromatv/client/accounts';
import type { KromaHost } from '@kromatv/module-sdk';
import type { z } from 'zod';
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

export const MODULE_ID = 'tv.kroma.party';

export class PartyError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
  ) {
    super(code);
  }
}

export function apiOrigin(): string {
  const configured = (globalThis as { __KROMA_API__?: string }).__KROMA_API__;
  const origin = configured ?? globalThis.location?.origin ?? '';
  let end = origin.length;
  while (end > 0 && origin[end - 1] === '/') end -= 1;
  return origin.slice(0, end);
}

export function bearer(): string | undefined {
  return loadSession()?.accessToken;
}

/** The module's HTTP surface for a page that is not under the admin shell. The
 *  main-shell host hands a page only `host.api.get`, so the bearer comes from the
 *  stored session; a 401 triggers one authenticated host read, which makes the
 *  client refresh its token, and the call is retried once. */
export function createPartyApi(host: Pick<KromaHost, 'api'>) {
  async function call<S extends z.ZodType>(
    method: 'GET' | 'POST',
    path: string,
    body: unknown,
    schema: S,
  ): Promise<z.output<S>> {
    const send = () =>
      fetch(`${apiOrigin()}/api/module/${MODULE_ID}${path}`, {
        method,
        headers: {
          authorization: `Bearer ${bearer() ?? ''}`,
          ...(body === undefined ? {} : { 'content-type': 'application/json' }),
        },
        body: body === undefined ? undefined : JSON.stringify(body),
      });
    let response = await send();
    if (response.status === 401) {
      await host.api.get('/modules').catch(() => undefined);
      response = await send();
    }
    const json: unknown = await response.json().catch(() => null);
    if (!response.ok) {
      const code = (json as { error?: unknown } | null)?.error;
      throw new PartyError(response.status, typeof code === 'string' ? code : 'unknown');
    }
    return schema.parse(json);
  }

  const get = <S extends z.ZodType>(path: string, schema: S) =>
    call('GET', path, undefined, schema);
  const post = <S extends z.ZodType>(path: string, body: unknown, schema: S) =>
    call('POST', path, body, schema);

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
    kick: (userId: string) => post('/sync/kick', { userId, reason: '' }, Ok.loose()),
  };
}

export type PartyApi = ReturnType<typeof createPartyApi>;
