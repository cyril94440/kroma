import { KromaEvents } from '@kromatv/client/events';
import type { KromaHost } from '@kromatv/module-sdk';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { apiOrigin, createLiveApi } from './live-api';
import { applyEvent } from './live-state';
import type { Status } from './schemas';

const SAFETY_REFRESH_MS = 30_000;
const DEFAULT_HEARTBEAT_MS = 15_000;

export function useNow(everyMs: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), everyMs);
    return () => clearInterval(timer);
  }, [everyMs]);
  return now;
}

export function useLive(host: KromaHost, watching: boolean) {
  const api = useMemo(() => createLiveApi(host), [host]);
  const [status, setStatus] = useState<Status | null>(null);
  const [failed, setFailed] = useState(false);

  const refresh = useCallback(() => {
    api
      .status()
      .then((next) => {
        setStatus(next);
        setFailed(false);
      })
      .catch(() => setFailed(true));
  }, [api]);

  useEffect(() => {
    refresh();
    const timer = setInterval(refresh, SAFETY_REFRESH_MS);
    return () => clearInterval(timer);
  }, [refresh]);

  useEffect(() => {
    const events = new KromaEvents<{ type: string }>(apiOrigin(), {
      token: () => host.api.sessionToken(),
      refresh: async () => {
        await host.api.get('/modules').catch(() => undefined);
        return host.api.sessionToken();
      },
      onOpen: refresh,
      onEvent: (raw) => setStatus((current) => applyEvent(current, raw)),
    });
    events.connect();
    return () => events.close();
  }, [host, refresh]);

  useEffect(() => {
    if (!watching) return;
    let interval = DEFAULT_HEARTBEAT_MS;
    let timer: ReturnType<typeof setTimeout>;
    const beat = () => {
      api
        .beat()
        .then((answer) => {
          interval = answer.heartbeatSeconds * 1000;
        })
        .catch(() => undefined)
        .finally(() => {
          timer = setTimeout(beat, interval);
        });
    };
    beat();
    const leave = () => void api.leave().catch(() => undefined);
    window.addEventListener('pagehide', leave);
    return () => {
      clearTimeout(timer);
      window.removeEventListener('pagehide', leave);
      leave();
    };
  }, [api, watching]);

  return { api, status, failed, refresh };
}
