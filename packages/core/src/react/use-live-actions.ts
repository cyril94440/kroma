import { useMemo } from 'react';
import type { LiveModules } from '../live/modules';
import type { LiveClient } from './use-live-room';

export type ZapOutcome = 'voteStarted' | 'switchedDirectly' | 'refused';

export interface LiveActions {
  zap(channel: { id: string; name: string }): Promise<ZapOutcome>;
  vote(approve: boolean): Promise<unknown>;
  pollVote(value: string): Promise<unknown>;
  chat(text: string): Promise<unknown>;
  react(emoji: string): Promise<unknown>;
}

/** What a viewer does on the live screen. A zap goes through the party's vote
 *  when that module is there, which decides between asking the room and
 *  switching at once; without it the live module switches. Each action returns
 *  its promise so the screen decides how a failure is told. */
export function useLiveActions(client: LiveClient, modules: LiveModules): LiveActions {
  return useMemo(
    () => ({
      zap: async (channel) => {
        if (modules.party) {
          return (await client.party.requestSwitch(channel.id, channel.name)).outcome;
        }
        await client.live.switchTo(channel.id);
        return 'switchedDirectly';
      },
      vote: (approve) => client.party.vote(approve),
      pollVote: (value) => client.party.pollVote(value),
      chat: (text) => client.party.chat(text),
      react: (emoji) => client.party.floating(emoji),
    }),
    [client, modules.party],
  );
}
