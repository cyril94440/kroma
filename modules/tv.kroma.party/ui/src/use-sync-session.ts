import { useEffect, useMemo, useRef, useState } from 'react';
import type { PartyApi } from './party-api';
import type { Model } from './party-state';
import { SyncEngine, type SyncNotice } from './sync-engine';
import type { SyncPlayer } from './sync-player';

const NOTICE_MS = 4000;
const STATS_EVERY_MS = 8000;

export interface SyncSession {
  on: boolean;
  notice: SyncNotice | null;
  join: () => void;
  leave: () => void;
}

export function useSyncSession(
  player: SyncPlayer,
  playing: boolean,
  channelId: string | null,
  api: PartyApi,
  model: Model,
): SyncSession {
  const [on, setOn] = useState(false);
  const [notice, setNotice] = useState<SyncNotice | null>(null);
  const latest = useRef(model);
  useEffect(() => {
    latest.current = model;
  });
  const clear = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const engine = useMemo(() => {
    const created = new SyncEngine({
      player,
      transport: {
        beat: (body) => api.beat(body),
        fineDone: (body) => api.fineDone(body),
        recalibration: (body) => api.recalibration(body),
        serverNow: async () => (await api.time()).now,
      },
      notify: (next) => {
        setNotice(next);
        clearTimeout(clear.current);
        clear.current = setTimeout(() => setNotice(null), next.ms ?? NOTICE_MS);
      },
      peerDelays: async () => {
        const fresh = await api.state().catch(() => latest.current.state);
        if (!fresh) return [];
        const inParty = new Set(fresh.sync?.members.map((m) => m.userId));
        return Object.entries(fresh.stats)
          .filter(([id, s]) => id !== fresh.me.userId && inParty.has(id) && s.playing && !s.paused)
          .flatMap(([, s]) => (s.delayS === null ? [] : [s.delayS]));
      },
      onChange: () => setOn(created.on),
    });
    return created;
  }, [player, api]);

  useEffect(() => {
    engine.start();
    const report = () => {
      const delay = engine.absLatency();
      void api
        .stats({
          buffer: Math.round(player.getBufferAheadMs()) / 1000,
          bandwidthKbps: 0,
          stalls: 0,
          rebufferMs: 0,
          dropped: 0,
          resolution: '',
          playing: engine.playing,
          delay,
          paused: !engine.playing,
        })
        .catch(() => undefined);
    };
    const timer = setInterval(report, STATS_EVERY_MS);
    return () => {
      clearInterval(timer);
      engine.leave();
      engine.stop();
    };
  }, [engine, api, player]);

  const group = model.state?.sync ?? null;
  useEffect(() => engine.setGroup(group), [engine, group]);
  useEffect(() => engine.setPlaying(playing), [engine, playing]);
  useEffect(() => engine.graceAfterRestart(), [engine, channelId]);
  useEffect(() => {
    if (model.goLiveAt > 0) engine.goLive();
  }, [engine, model.goLiveAt]);
  useEffect(() => {
    if (model.fineSyncAt > 0) engine.runFineSync();
  }, [engine, model.fineSyncAt]);
  useEffect(() => {
    if (model.kickedAt > 0) {
      engine.kicked(model.kicked ?? '');
    }
  }, [engine, model.kickedAt, model.kicked]);

  return useMemo(
    () => ({
      on,
      notice,
      join: () => {
        engine.join();
        setNotice({ key: 'party.sync.toast.joined' });
      },
      leave: () => {
        engine.leave();
        setNotice({ key: 'party.sync.toast.left' });
      },
    }),
    [engine, on, notice],
  );
}
