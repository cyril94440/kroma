import { useServerEvents, useViewerApi } from '@kromatv/module-sdk';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { SportEventMessage } from './schemas';
import { createSportApi } from './sport-api';
import { adopt, applyEvent, EMPTY, type Model } from './sport-state';

const SAFETY_REFRESH_MS = 30_000;

export function useNow(everyMs: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), everyMs);
    return () => clearInterval(timer);
  }, [everyMs]);
  return now;
}

export function useSport() {
  const viewer = useViewerApi();
  const api = useMemo(() => createSportApi(viewer), [viewer]);
  const [model, setModel] = useState<Model>(EMPTY);
  const [failed, setFailed] = useState(false);

  const refresh = useCallback(() => {
    api
      .state()
      .then((state) => {
        setModel(adopt(state, Date.now()));
        setFailed(false);
      })
      .catch(() => setFailed(true));
  }, [api]);

  useEffect(() => {
    refresh();
    const timer = setInterval(refresh, SAFETY_REFRESH_MS);
    return () => clearInterval(timer);
  }, [refresh]);

  useServerEvents<{ type: string }>((raw) => {
    const parsed = SportEventMessage.safeParse(raw);
    if (parsed.success) setModel((m) => applyEvent(m, parsed.data, Date.now()));
  }, refresh);

  useEffect(() => {
    if (model.stale) refresh();
  }, [model.stale, refresh]);

  const now = useNow(1000);
  return {
    api,
    model,
    failed,
    refresh,
    now,
    serverNow: now + model.clockOffsetMs,
    postSeenAt: model.postSeen?.at ?? null,
  };
}
