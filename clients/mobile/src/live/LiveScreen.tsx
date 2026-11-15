import type { ChannelView } from '@kromatv/client/live';
import { liveBadges, liveNotice, playlistKey, pollToAnswer } from '@kromatv/core';
import { useIdeas, useLiveActions, useLiveSync } from '@kromatv/core/react';
import { Box, Button, Spinner, styles, Text, Toaster, toast, useStyles } from '@kromatv/ui/kit';
import { Redirect } from 'expo-router';
import { VideoView } from 'expo-video';
import { useEffect, useMemo, useState } from 'react';
import { ScrollView } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { useT } from '#mobile/lib/i18n';
import { useGutters } from '#mobile/lib/layout';
import { PollCard, ReactionsRow, VoteBanner } from '#mobile/live/LivePanels';
import { availableSections, LiveSections, type Section } from '#mobile/live/LiveSections';
import { LiveVideo } from '#mobile/live/LiveVideo';
import { LiveSyncPanel } from '#mobile/live/LiveSync';
import { useLiveEngine } from '#mobile/live/use-live-engine';
import { useLiveHandlers } from '#mobile/live/use-live-handlers';
import { useLiveView } from '#mobile/live/use-live-view';
import { useLiveScreen } from '#mobile/live/use-live-screen';

function LiveHeader({
  title,
  detail,
  behind,
  viewers,
  onGoLive,
}: Readonly<{
  title: string;
  detail: string;
  behind: boolean;
  viewers: number;
  onGoLive: () => void;
}>) {
  const t = useT();
  return (
    <Box row justify="space-between" align="center">
      <Box flex>
        <Text variant="title">{title}</Text>
        <Text variant="meta" color="textMuted">
          {detail}
        </Text>
      </Box>
      {behind ? (
        <Button
          icon="player-track-next"
          label={t('player.backToLive')}
          variant="primary"
          onPress={onGoLive}
        />
      ) : (
        <Text variant="meta" color="textMuted">
          {t('live.viewers', { count: viewers })}
        </Text>
      )}
    </Box>
  );
}

export default function LiveScreen() {
  const s = useStyles(sDefinition);
  const t = useT();
  const insets = useSafeAreaInsets();
  const gutters = useGutters();
  const { client, room, modules, ready } = useLiveScreen();
  const actions = useLiveActions(client, modules);
  const handlers = useLiveHandlers(actions);
  const { status, party, sport, now } = room;
  const playlist = playlistKey(status);
  const url = useMemo(
    () => (playlist ? client.live.playlistUrl(playlist) : null),
    [client, playlist],
  );
  const engine = useLiveEngine(url);
  const [section, setSection] = useState<Section>('chat');
  const { me, matchData, hasMatch, activity, sportView } = useLiveView(room);
  const vote = party.state?.vote ?? null;
  const poll = pollToAnswer(party.state?.poll ?? null, me, null);
  const notice = liveNotice(status, room.failed || engine.failed);
  const badge = status ? liveBadges(status)[0] : undefined;
  const sections = availableSections({ party: modules.party, sport: modules.sport, hasMatch });
  const shown = sections.includes(section) ? section : 'channels';
  const sync = useLiveSync({
    client,
    source: engine.sync,
    party,
    playing: engine.playing,
    channelId: status?.channel?.id ?? null,
  });
  const { ideas } = useIdeas(client, party.ideasSeq, modules.party && shown === 'ideas');
  const syncNotice = sync.notice;
  useEffect(() => {
    if (syncNotice) toast({ message: t(syncNotice.key, syncNotice.vars) });
  }, [syncNotice, t]);

  if (ready && !modules.live) return <Redirect href="/" />;

  const zap = (channel: ChannelView) => void actions.zap(channel).catch(() => undefined);

  return (
    <Box fill bg="bg" style={{ paddingTop: insets.top }}>
      <LiveVideo
        player={engine.player}
        label={status?.channel?.name ?? t('nav.live')}
        notice={notice}
        waiting={engine.waiting}
      />
      <ScrollView
        contentContainerStyle={[s.body, gutters.style, { paddingBottom: insets.bottom + 96 }]}
      >
        <LiveHeader
          title={status?.channel?.name ?? t('nav.live')}
          detail={[status?.now?.title, badge ? t(`live.badge.${badge}`) : null]
            .filter(Boolean)
            .join('  ·  ')}
          behind={engine.behind}
          viewers={status?.viewers.length ?? 0}
          onGoLive={engine.goLive}
        />
        {vote ? (
          <VoteBanner
            vote={vote}
            userId={me}
            now={now}
            onAnswer={(approve) => void actions.vote(approve).catch(() => undefined)}
          />
        ) : null}
        {poll ? (
          <PollCard
            poll={poll}
            userId={me}
            now={now}
            onAnswer={(value) => void actions.pollVote(value).catch(() => undefined)}
          />
        ) : null}
        {modules.party ? <ReactionsRow onReact={handlers.react} /> : null}
        {modules.party ? <LiveSyncPanel sync={sync} group={party.state?.sync ?? null} /> : null}
        <LiveSections
          sections={sections}
          shown={shown}
          onShow={setSection}
          client={client}
          data={{
            messages: party.state?.messages ?? [],
            activity,
            currentId: status?.channel?.id ?? null,
            match: matchData,
            sportView,
            ideas,
          }}
          actions={{
            onSend: handlers.reply,
            onWatchCard: handlers.zapByName,
            onZap: zap,
            onVote: handlers.voteProno,
            onPlayClip: handlers.playClip,
            onWatchSport: handlers.watchSport,
            onIdea: actions.idea,
          }}
          frameTimeMs={engine.frameTimeMs}
          now={now + sport.clockOffsetMs}
        />
      </ScrollView>
      <Toaster position="bottom-center" inset={{ x: 16, y: insets.bottom + 24 }} />
    </Box>
  );
}

const sDefinition = styles({
  video: { flex: 1 },
  body: { gap: 16, paddingTop: 16 },
});
