import { loadSession } from '@kromatv/client/accounts';
import type { KromaHost } from '@kromatv/module-sdk';
import type { z } from 'zod';
import {
  AssistantStatus,
  AuditPage,
  Done,
  Lessons,
  ModelList,
  ModelTest,
  Notes,
  Rules,
  Suggestions,
} from './schemas';

export const MODULE_ID = 'tv.kroma.hermes';

export class HermesError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
  ) {
    super(code);
  }
}

export function apiOrigin(): string {
  const configured = (globalThis as { __KROMA_API__?: string }).__KROMA_API__;
  const base = configured ?? globalThis.location?.origin ?? '';
  let end = base.length;
  while (end > 0 && base[end - 1] === '/') end -= 1;
  return base.slice(0, end);
}

type Method = 'GET' | 'POST' | 'PUT' | 'DELETE';

export function auditPath(genre: string, before?: number): string {
  const params = new URLSearchParams({ limit: '100' });
  if (genre) params.set('genre', genre);
  if (before !== undefined) params.set('before', String(before));
  return `/audit?${params.toString()}`;
}

export function createHermesApi(host: Pick<KromaHost, 'api'>) {
  async function call<S extends z.ZodType>(
    method: Method,
    path: string,
    body: unknown,
    schema: S,
  ): Promise<z.output<S>> {
    const send = () =>
      fetch(`${apiOrigin()}/api/module/${MODULE_ID}${path}`, {
        method,
        headers: {
          authorization: `Bearer ${loadSession()?.accessToken ?? ''}`,
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
      throw new HermesError(response.status, typeof code === 'string' ? code : 'unknown');
    }
    return schema.parse(json);
  }

  return {
    status: () => call('GET', '/status', undefined, AssistantStatus),
    pause: (paused: boolean) => call('POST', '/pause', { paused }, Done),
    audit: (genre: string, before?: number) =>
      call('GET', auditPath(genre, before), undefined, AuditPage),
    rules: () => call('GET', '/rules', undefined, Rules),
    saveRules: (text: string) => call('PUT', '/rules', { text }, Done),
    resetRules: () => call('POST', '/rules/reset', {}, Done),
    lessons: () => call('GET', '/lessons', undefined, Lessons),
    removeLesson: (id: number) => call('DELETE', `/lessons/${id}`, undefined, Done),
    suggestions: () => call('GET', '/suggestions', undefined, Suggestions),
    removeSuggestion: (id: number) => call('DELETE', `/suggestions/${id}`, undefined, Done),
    notes: () => call('GET', '/notes', undefined, Notes),
    forget: (userId: string) =>
      call('DELETE', `/notes/${encodeURIComponent(userId)}`, undefined, Done),
    testModel: () => call('POST', '/model/test', {}, ModelTest),
    listModels: () => call('GET', '/model/list', undefined, ModelList),
  };
}

export type HermesApi = ReturnType<typeof createHermesApi>;
