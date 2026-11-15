import type { IdeaGenre } from '@kromatv/client/party';
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
  zapByName(channelName: string): Promise<ZapOutcome>;
  pronoVote(id: string, score: string): Promise<unknown>;
  watchSport(channelId: string): Promise<unknown>;
  idea(genre: IdeaGenre, text: string): Promise<unknown>;
}

/** What a viewer does on the live screen. A card's watch button resolves its
 *  channel name through the party's search and then zaps like any other. A zap goes through the party's vote
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
      zapByName: async (channelName) => {
        const { channels } = await client.party.channels(channelName);
        const found = channels[0];
        if (!found) return 'refused';
        return (await client.party.requestSwitch(found.id, found.name)).outcome;
      },
      pronoVote: (id, score) => client.sport.vote(id, score),
      watchSport: (channelId) => client.sport.watch(channelId),
      idea: (genre, text) => client.party.submitIdea(genre, text),
    }),
    [client, modules.party],
  );
}
