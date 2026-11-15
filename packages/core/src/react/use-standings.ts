import type { StandingsReply } from '@kromatv/client/sport';
import { useEffect, useState } from 'react';
import type { LiveClient } from './use-live-room';

export const STANDINGS_POLL_MS = 2500;

export type StandingsState = 'loading' | 'ready' | 'unavailable';

export interface Standings {
  state: StandingsState;
  table: NonNullable<StandingsReply['standings']> | null;
}

/** One competition's table. The module computes it in the background and
 *  answers `pending` until it is there, so this asks again every 2.5 s then. */
export function useStandings(
  client: Pick<LiveClient, 'sport'>,
  competition: string,
  enabled: boolean,
): Standings {
  const [reply, setReply] = useState<{ competition: string; reply: StandingsReply | null } | null>(
    null,
  );
  useEffect(() => {
    if (!enabled) return;
    let alive = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const read = () => {
      client.sport.standings(competition).then(
        (next) => {
          if (!alive) return;
          setReply({ competition, reply: next });
          if (next.pending) timer = setTimeout(read, STANDINGS_POLL_MS);
        },
        () => alive && setReply({ competition, reply: null }),
      );
    };
    read();
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [client, competition, enabled]);
  const current = reply?.competition === competition ? reply.reply : undefined;
  if (current === undefined) return { state: 'loading', table: null };
  if (current === null) return { state: 'unavailable', table: null };
  if (current.standings) return { state: 'ready', table: current.standings };
  return { state: current.pending ? 'loading' : 'unavailable', table: null };
}
