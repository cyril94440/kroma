import type { ChannelView } from '@kromatv/client/live';
import type { ChatMessage, Idea, IdeaGenre } from '@kromatv/client/party';
import type { Clip, SportEvent, SportView } from '@kromatv/client/sport';
import type { LiveClient } from '@kromatv/core/react';
import { Box, Button } from '@kromatv/ui/kit';
import { useT } from '#mobile/lib/i18n';
import { LiveChannels } from '#mobile/live/LiveChannels';
import { LiveChat } from '#mobile/live/LiveChat';
import { LiveIdeas } from '#mobile/live/LiveIdeas';
import { LiveSport } from '#mobile/live/LiveSport';
import { LiveMatch, type MatchPanelData } from '#mobile/live/match/LiveMatch';

export type Section = 'chat' | 'channels' | 'match' | 'sport' | 'ideas';

export function availableSections(modules: {
  party: boolean;
  sport: boolean;
  hasMatch: boolean;
}): Section[] {
  return [
    ...(modules.party ? (['chat'] as const) : []),
    'channels',
    ...(modules.sport && modules.hasMatch ? (['match'] as const) : []),
    ...(modules.sport ? (['sport'] as const) : []),
    ...(modules.party ? (['ideas'] as const) : []),
  ];
}

export interface SectionData {
  messages: readonly ChatMessage[];
  activity: string | null;
  currentId: string | null;
  match: MatchPanelData;
  sportView: SportView | null;
  ideas: readonly Idea[];
}

export interface SectionActions {
  onSend: (text: string) => void;
  onWatchCard: (channel: string) => void;
  onZap: (channel: ChannelView) => void;
  onVote: (id: string, score: string) => void;
  onPlayClip: (clip: Clip) => void;
  onWatchSport: (event: SportEvent) => void;
  onIdea: (genre: IdeaGenre, text: string) => Promise<unknown>;
}

/** The tabs under the picture and the one that is open, each only where its
 *  module is on. */
export function LiveSections({
  sections,
  shown,
  onShow,
  client,
  data,
  actions,
  frameTimeMs,
  now,
}: Readonly<{
  sections: readonly Section[];
  shown: Section;
  onShow: (section: Section) => void;
  client: Pick<LiveClient, 'live' | 'sport'>;
  data: SectionData;
  actions: SectionActions;
  frameTimeMs: () => number | null;
  now: number;
}>) {
  const t = useT();
  return (
    <Box gap={16}>
      <Box row wrap gap={8}>
        {sections.map((entry) => (
          <Button
            key={entry}
            label={t(`live.${entry}`)}
            variant={entry === shown ? 'primary' : 'scrim'}
            onPress={() => onShow(entry)}
          />
        ))}
      </Box>
      {shown === 'chat' ? (
        <LiveChat
          messages={data.messages}
          activity={data.activity}
          onSend={actions.onSend}
          onWatch={actions.onWatchCard}
        />
      ) : null}
      {shown === 'channels' ? (
        <LiveChannels client={client} currentId={data.currentId} onPick={actions.onZap} />
      ) : null}
      {shown === 'match' ? (
        <LiveMatch
          data={data.match}
          frameTimeMs={frameTimeMs}
          now={now}
          onVote={actions.onVote}
          onPlayClip={actions.onPlayClip}
        />
      ) : null}
      {shown === 'sport' ? (
        <LiveSport view={data.sportView} client={client} onWatch={actions.onWatchSport} />
      ) : null}
      {shown === 'ideas' ? <LiveIdeas ideas={data.ideas} onSubmit={actions.onIdea} /> : null}
    </Box>
  );
}
