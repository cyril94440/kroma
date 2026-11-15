import type { KromaClient } from '@kromatv/client';
import type { Status } from '@kromatv/client/live';
import { PartyEvent } from '@kromatv/client/party';
import { SportEventMessage } from '@kromatv/client/sport';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { applyLiveEvent } from '../live/live-state';
import type { LiveModules } from '../live/modules';
import {
  adoptParty,
  applyPartyEvent,
  EMPTY_PARTY,
  expireFloating,
  type PartyModel,
} from '../live/party-state';
import { adoptSport, applySportEvent, EMPTY_SPORT, type SportModel } from '../live/sport-state';
import type { OpenEvents } from './live-events';

export const SAFETY_REFRESH_MS = 30_000;
export const DEFAULT_HEARTBEAT_MS = 15_000;
export const FLOATING_TTL_MS = 4000;

export type LiveClient = Pick<KromaClient, 'live' | 'party' | 'sport'>;

export interface LiveRoomOptions {
  client: LiveClient;
  openEvents: OpenEvents;
  modules: LiveModules;
  watching: boolean;
}

export interface LiveRoom {
  client: LiveClient;
  status: Status | null;
  failed: boolean;
  party: PartyModel;
  sport: SportModel;
  now: number;
  refresh: () => void;
}

function usePresence(client: LiveClient, enabled: boolean): void {
  const latest = useRef(client);
  useEffect(() => {
    latest.current = client;
  });
  useEffect(() => {
    if (!enabled) return;
    let interval = DEFAULT_HEARTBEAT_MS;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let stopped = false;
    const beat = () => {
      latest.current.live
        .beat()
        .then((answer) => {
          interval = answer.heartbeatSeconds * 1000;
        })
        .catch(() => undefined)
        .finally(() => {
          if (!stopped) timer = setTimeout(beat, interval);
        });
    };
    beat();
    return () => {
      stopped = true;
      clearTimeout(timer);
      latest.current.live.leave();
    };
  }, [enabled]);
}

/** Everything one live screen follows, over a single event socket: the live
 *  status, the party's chat, vote and poll, and the sport panel. Each part is
 *  fetched only when its module is on, and refetched when the socket reopens,
 *  when a sequence gap marks it stale and every 30 s as a safety net. While
 *  `watching`, the viewer is announced present every beat and says goodbye when
 *  `watching` turns false or the screen goes away. */
export function useLiveRoom({ client, openEvents, modules, watching }: LiveRoomOptions): LiveRoom {
  const [status, setStatus] = useState<Status | null>(null);
  const [failed, setFailed] = useState(false);
  const [party, setParty] = useState<PartyModel>(EMPTY_PARTY);
  const [sport, setSport] = useState<SportModel>(EMPTY_SPORT);
  const [now, setNow] = useState(() => Date.now());

  const refreshLive = useCallback(() => {
    client.live.status().then(
      (next) => {
        setStatus(next);
        setFailed(false);
      },
      () => setFailed(true),
    );
  }, [client]);

  const refreshParty = useCallback(() => {
    if (!modules.party) return;
    client.party
      .state()
      .then((state) => setParty((model) => adoptParty(model, state)))
      .catch(() => undefined);
  }, [client, modules.party]);

  const refreshSport = useCallback(() => {
    if (!modules.sport) return;
    client.sport
      .state()
      .then((state) => setSport(adoptSport(state, Date.now())))
      .catch(() => undefined);
  }, [client, modules.sport]);

  const refresh = useCallback(() => {
    refreshLive();
    refreshParty();
    refreshSport();
  }, [refreshLive, refreshParty, refreshSport]);

  useEffect(() => {
    refresh();
    const timer = setInterval(refresh, SAFETY_REFRESH_MS);
    return () => clearInterval(timer);
  }, [refresh]);

  useEffect(
    () =>
      openEvents({
        onOpen: refresh,
        onEvent: (raw) => {
          if (raw.type.startsWith('live.')) setStatus((current) => applyLiveEvent(current, raw));
          if (modules.party && raw.type.startsWith('party.')) {
            const parsed = PartyEvent.safeParse(raw);
            if (parsed.success)
              setParty((model) => applyPartyEvent(model, parsed.data, Date.now()));
          }
          if (modules.sport && raw.type.startsWith('sport.')) {
            const parsed = SportEventMessage.safeParse(raw);
            if (parsed.success) setSport((model) => applySportEvent(model, parsed.data));
          }
        },
      }),
    [openEvents, refresh, modules.party, modules.sport],
  );

  useEffect(() => {
    if (party.stale) refreshParty();
  }, [party.stale, refreshParty]);

  useEffect(() => {
    if (sport.stale) refreshSport();
  }, [sport.stale, refreshSport]);

  useEffect(() => {
    const timer = setInterval(() => {
      const at = Date.now();
      setNow(at);
      setParty((model) => expireFloating(model, at, FLOATING_TTL_MS));
    }, 1000);
    return () => clearInterval(timer);
  }, []);

  usePresence(client, watching && modules.live);

  return useMemo(
    () => ({ client, status, failed, party, sport, now, refresh }),
    [client, status, failed, party, sport, now, refresh],
  );
}
