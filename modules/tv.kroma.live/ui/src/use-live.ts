import { useModuleFileUrl, useServerEvents, useViewerApi } from '@kromatv/module-sdk';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { createLiveApi } from './live-api';
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

export function useLive(watching: boolean) {
  const viewer = useViewerApi();
  const fileUrl = useModuleFileUrl();
  const api = useMemo(() => createLiveApi(viewer, fileUrl), [viewer, fileUrl]);
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

  useServerEvents<{ type: string }>(
    (raw) => setStatus((current) => applyEvent(current, raw)),
    refresh,
  );

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
    const leave = () => api.leave();
    window.addEventListener('pagehide', leave);
    return () => {
      clearTimeout(timer);
      window.removeEventListener('pagehide', leave);
      leave();
    };
  }, [api, watching]);

  return { api, status, failed, refresh };
}
