import { KromaEvents } from '@kromatv/client/events';
import type { KromaHost } from '@kromatv/module-sdk';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { apiOrigin, bearer, createPartyApi } from './party-api';
import { adopt, applyEvent, EMPTY, expireFloating, type Model } from './party-state';
import { PartyEvent } from './schemas';

const SAFETY_REFRESH_MS = 30_000;
const FLOATING_TTL_MS = 4000;

export function useNow(everyMs: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), everyMs);
    return () => clearInterval(timer);
  }, [everyMs]);
  return now;
}

export function useParty(host: KromaHost) {
  const api = useMemo(() => createPartyApi(host), [host]);
  const [model, setModel] = useState<Model>(EMPTY);
  const [failed, setFailed] = useState(false);

  const refresh = useCallback(() => {
    api
      .state()
      .then((state) => {
        setModel((m) => adopt(m, state));
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
      token: bearer,
      refresh: async () => {
        await host.api.get('/modules').catch(() => undefined);
        return bearer();
      },
      onOpen: refresh,
      onEvent: (raw) => {
        const parsed = PartyEvent.safeParse(raw);
        if (parsed.success) setModel((m) => applyEvent(m, parsed.data, Date.now()));
      },
    });
    events.connect();
    return () => events.close();
  }, [host, refresh]);

  useEffect(() => {
    if (model.stale) refresh();
  }, [model.stale, refresh]);

  useEffect(() => {
    const timer = setInterval(
      () => setModel((m) => expireFloating(m, Date.now(), FLOATING_TTL_MS)),
      1000,
    );
    return () => clearInterval(timer);
  }, []);

  const now = useNow(1000);

  return { api, model, failed, refresh, now };
}
