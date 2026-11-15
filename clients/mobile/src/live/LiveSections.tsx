import type { ChannelView } from '@kromatv/client/live';
import type { MatchPayload } from '@kromatv/client/sport';
import type { ChatMessage } from '@kromatv/client/party';
import type { LiveActions, LiveClient } from '@kromatv/core/react';
import { Box, Button } from '@kromatv/ui/kit';
import { useT } from '#mobile/lib/i18n';
import { LiveChannels } from '#mobile/live/LiveChannels';
import { LiveChat } from '#mobile/live/LiveChat';
import { LiveMatch } from '#mobile/live/LiveMatch';

export type Section = 'chat' | 'channels' | 'match';

export function availableSections(party: boolean, sportMatch: boolean): Section[] {
  return [
    ...(party ? (['chat'] as const) : []),
    'channels',
    ...(sportMatch ? (['match'] as const) : []),
  ];
}

/** The tabs under the picture and the one that is open: chat, the channel list
 *  and the match, each only where its module is on. */
export function LiveSections({
  sections,
  shown,
  onShow,
  client,
  actions,
  messages,
  currentId,
  onZap,
  match,
  frameTimeMs,
  now,
}: Readonly<{
  sections: readonly Section[];
  shown: Section;
  onShow: (section: Section) => void;
  client: Pick<LiveClient, 'live'>;
  actions: LiveActions;
  messages: readonly ChatMessage[];
  currentId: string | null;
  onZap: (channel: ChannelView) => void;
  match: MatchPayload | null;
  frameTimeMs: () => number | null;
  now: number;
}>) {
  const t = useT();
  return (
    <Box gap={16}>
      <Box row gap={8}>
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
          messages={messages}
          onSend={(text) => void actions.chat(text).catch(() => undefined)}
        />
      ) : null}
      {shown === 'channels' ? (
        <LiveChannels client={client} currentId={currentId} onPick={onZap} />
      ) : null}
      {shown === 'match' ? <LiveMatch match={match} frameTimeMs={frameTimeMs} now={now} /> : null}
    </Box>
  );
}
