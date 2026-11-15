import { type RefObject, useEffect, useMemo, useRef, useState } from 'react';
import type { PartyModel } from '../live/party-state';
import { type MoveRange, type SyncNotice, SyncEngine, type SyncPlayer } from '../sync';
import type { LiveClient } from './use-live-room';

export const SYNC_NOTICE_MS = 4000;
export const SYNC_STATS_EVERY_MS = 8000;

/** What a live engine gives the sync: the frame it shows, a window to seek in,
 *  and the three moves. The shells' engines already have these members. */
export interface SyncSource {
  frameTimeMs(): number | null;
  isReady(): boolean;
  bufferAheadMs(): number;
  moveRange(): MoveRange | null;
  goLive(): void;
  seekByMs(deltaMs: number): void;
  setRate(rate: number): void;
}

export interface LiveSync {
  on: boolean;
  notice: SyncNotice | null;
  join: () => void;
  leave: () => void;
}

export interface LiveSyncOptions {
  client: Pick<LiveClient, 'party'>;
  source: RefObject<SyncSource | null>;
  party: PartyModel;
  playing: boolean;
  channelId: string | null;
}

function playerOver(source: RefObject<SyncSource | null>): SyncPlayer {
  return {
    getPositionMs: () => source.current?.frameTimeMs() ?? null,
    isReady: () => source.current?.isReady() ?? false,
    getBufferAheadMs: () => source.current?.bufferAheadMs() ?? 0,
    getMoveRange: () => source.current?.moveRange() ?? null,
    seekToLiveEdge: () => source.current?.goLive(),
    seekByMs: (deltaMs) => source.current?.seekByMs(deltaMs),
    setPlaybackRate: (rate) => source.current?.setRate(rate),
    subscribe: () => () => undefined,
  };
}

/** The viewer's side of the watch party: joins or leaves the group, keeps the
 *  picture on the group's delay and reports it, and follows the room's go-live,
 *  fine-sync and removal orders. The engine measures the server clock and the
 *  frame on screen itself; this hook only wires it to the party routes and to
 *  what the screen shows. */
export function useLiveSync({
  client,
  source,
  party,
  playing,
  channelId,
}: LiveSyncOptions): LiveSync {
  const [on, setOn] = useState(false);
  const [notice, setNotice] = useState<SyncNotice | null>(null);
  const latest = useRef(party);
  useEffect(() => {
    latest.current = party;
  });
  const clear = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const player = useMemo(() => playerOver(source), [source]);

  const engine = useMemo(() => {
    const created = new SyncEngine({
      player,
      transport: {
        beat: (body) => client.party.beat(body),
        fineDone: (body) => client.party.fineDone(body),
        recalibration: (body) => client.party.recalibration(body),
        serverNow: async () => (await client.party.time()).now,
      },
      notify: (next) => {
        setNotice(next);
        clearTimeout(clear.current);
        clear.current = setTimeout(() => setNotice(null), next.ms ?? SYNC_NOTICE_MS);
      },
      peerDelays: async () => {
        const fresh = await client.party.state().catch(() => latest.current.state);
        if (!fresh) return [];
        const inParty = new Set(fresh.sync?.members.map((m) => m.userId));
        return Object.entries(fresh.stats)
          .filter(([id, s]) => id !== fresh.me.userId && inParty.has(id) && s.playing && !s.paused)
          .flatMap(([, s]) => (s.delayS === null ? [] : [s.delayS]));
      },
      onChange: () => setOn(created.on),
    });
    return created;
  }, [player, client]);

  useEffect(() => {
    engine.start();
    const report = () => {
      void client.party
        .stats({
          buffer: Math.round(player.getBufferAheadMs()) / 1000,
          bandwidthKbps: 0,
          stalls: 0,
          rebufferMs: 0,
          dropped: 0,
          resolution: '',
          playing: engine.playing,
          delay: engine.absLatency(),
          paused: !engine.playing,
        })
        .catch(() => undefined);
    };
    const timer = setInterval(report, SYNC_STATS_EVERY_MS);
    return () => {
      clearInterval(timer);
      clearTimeout(clear.current);
      engine.leave();
      engine.stop();
    };
  }, [engine, client, player]);

  const group = party.state?.sync ?? null;
  useEffect(() => engine.setGroup(group), [engine, group]);
  useEffect(() => engine.setPlaying(playing), [engine, playing]);
  useEffect(() => engine.graceAfterRestart(), [engine, channelId]);
  useEffect(() => {
    if (party.goLiveAt > 0) engine.goLive();
  }, [engine, party.goLiveAt]);
  useEffect(() => {
    if (party.fineSyncAt > 0) engine.runFineSync();
  }, [engine, party.fineSyncAt]);
  useEffect(() => {
    if (party.kickedAt > 0) engine.kicked(party.kicked ?? '');
  }, [engine, party.kickedAt, party.kicked]);

  return useMemo(
    () => ({
      on,
      notice,
      join: () => {
        engine.join();
        setNotice({ key: 'live.sync.toast.joined' });
      },
      leave: () => {
        engine.leave();
        setNotice({ key: 'live.sync.toast.left' });
      },
    }),
    [engine, on, notice],
  );
}
