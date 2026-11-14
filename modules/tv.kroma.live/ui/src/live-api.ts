import type { KromaHost } from '@kromatv/module-sdk';
import type { z } from 'zod';
import {
  BeatAnswer,
  ChannelPage,
  Favourites,
  History,
  Metrics,
  Overview,
  Refreshed,
  ScanState,
  Started,
  Status,
  SwitchOutcome,
} from './schemas';

export const MODULE_ID = 'tv.kroma.live';

export class LiveError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
  ) {
    super(code);
  }
}

function origin(): string {
  const configured = (globalThis as { __KROMA_API__?: string }).__KROMA_API__;
  const base = configured ?? globalThis.location?.origin ?? '';
  let end = base.length;
  while (end > 0 && base[end - 1] === '/') end -= 1;
  return base.slice(0, end);
}

export function apiOrigin(): string {
  return origin();
}

type Query = Record<string, string | number | undefined>;

function withQuery(path: string, query?: Query): string {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query ?? {})) {
    if (value !== undefined && value !== '') params.set(key, String(value));
  }
  const text = params.toString();
  return text ? `${path}?${text}` : path;
}

export function createLiveApi(host: Pick<KromaHost, 'api'>) {
  async function call<S extends z.ZodType>(
    method: 'GET' | 'POST',
    path: string,
    body: unknown,
    schema: S | null,
  ): Promise<z.output<S>> {
    const send = () =>
      fetch(`${origin()}/api/module/${MODULE_ID}${path}`, {
        method,
        headers: {
          authorization: `Bearer ${host.api.sessionToken() ?? ''}`,
          ...(body === undefined ? {} : { 'content-type': 'application/json' }),
        },
        body: body === undefined ? undefined : JSON.stringify(body),
        keepalive: path === '/presence/leave',
      });
    let response = await send();
    if (response.status === 401) {
      await host.api.get('/modules').catch(() => undefined);
      response = await send();
    }
    if (!response.ok) {
      const failure = (await response.json().catch(() => null)) as { error?: string } | null;
      throw new LiveError(response.status, failure?.error ?? String(response.status));
    }
    if (schema === null || response.status === 204) return undefined as z.output<S>;
    return schema.parse(await response.json());
  }

  return {
    status: () => call('GET', '/status', undefined, Status),
    channels: (query: { q?: string; group?: string; provider?: string; page?: number }) =>
      call('GET', withQuery('/channels', { ...query, perPage: 48 }), undefined, ChannelPage),
    favourites: () => call('GET', '/favourites', undefined, Favourites),
    beat: (companion = false) => call('POST', '/presence', { companion }, BeatAnswer),
    leave: () => call('POST', '/presence/leave', undefined, null),
    switchTo: (channelId: string) => call('POST', '/switch', { channelId }, SwitchOutcome),
    pause: () => call('POST', '/pause', undefined, null),
    resume: () => call('POST', '/resume', undefined, SwitchOutcome),
    overview: () => call('GET', '/overview', undefined, Overview),
    history: (minutes = 60) => call('GET', withQuery('/history', { minutes }), undefined, History),
    metrics: () => call('GET', '/metrics', undefined, Metrics),
    refresh: () => call('POST', '/refresh', undefined, Refreshed),
    startScan: (query: string, provider: string) =>
      call('POST', '/scan', { query, provider }, Started),
    scan: () => call('GET', '/scan', undefined, ScanState),
    fileUrl: (path: string) => host.api.moduleFileUrl(MODULE_ID, path),
  };
}

export type LiveApi = ReturnType<typeof createLiveApi>;
