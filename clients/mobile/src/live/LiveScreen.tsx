import type { ChannelView } from '@kromatv/client/live';
import { liveBadges, liveNotice, playlistKey, pollToAnswer } from '@kromatv/core';
import { useLiveActions } from '@kromatv/core/react';
import { Box, Button, Spinner, styles, Text, useStyles } from '@kromatv/ui/kit';
import { Redirect } from 'expo-router';
import { VideoView } from 'expo-video';
import { useMemo, useState } from 'react';
import { ScrollView } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { useT } from '#mobile/lib/i18n';
import { useGutters } from '#mobile/lib/layout';
import { PollCard, ReactionsRow, VoteBanner } from '#mobile/live/LivePanels';
import { availableSections, LiveSections, type Section } from '#mobile/live/LiveSections';
import { useLiveEngine } from '#mobile/live/use-live-engine';
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
  const { status, party, sport, now } = room;
  const playlist = playlistKey(status);
  const url = useMemo(
    () => (playlist ? client.live.playlistUrl(playlist) : null),
    [client, playlist],
  );
  const engine = useLiveEngine(url);
  const [section, setSection] = useState<Section>('chat');
  const me = party.state?.me.userId ?? '';
  const vote = party.state?.vote ?? null;
  const poll = pollToAnswer(party.state?.poll ?? null, me, null);
  const match = sport.state?.match ?? null;
  const notice = liveNotice(status, room.failed || engine.failed);
  const badge = status ? liveBadges(status)[0] : undefined;
  const sections = availableSections(modules.party, modules.sport && match !== null);
  const shown = sections.includes(section) ? section : 'channels';

  if (ready && !modules.live) return <Redirect href="/" />;

  const zap = (channel: ChannelView) => void actions.zap(channel).catch(() => undefined);

  return (
    <Box fill bg="bg" style={{ paddingTop: insets.top }}>
      <Box aspect={16 / 9} bg="black">
        <VideoView
          player={engine.player}
          style={s.video}
          contentFit="contain"
          nativeControls={false}
          accessibilityLabel={status?.channel?.name ?? t('nav.live')}
        />
        {notice || engine.waiting ? (
          <Box absolute top={0} right={0} bottom={0} left={0} center gap={12} pointerEvents="none">
            {notice && !notice.spinner ? null : <Spinner size={36} />}
            {notice ? (
              <Text variant="label" color="white" textAlign="center">
                {t(notice.title, { name: notice.channelName })}
              </Text>
            ) : null}
          </Box>
        ) : null}
      </Box>
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
        {modules.party ? (
          <ReactionsRow onReact={(emoji) => void actions.react(emoji).catch(() => undefined)} />
        ) : null}
        <LiveSections
          sections={sections}
          shown={shown}
          onShow={setSection}
          client={client}
          actions={actions}
          messages={party.state?.messages ?? []}
          currentId={status?.channel?.id ?? null}
          onZap={zap}
          match={match}
          frameTimeMs={engine.frameTimeMs}
          now={now + sport.clockOffsetMs}
        />
      </ScrollView>
    </Box>
  );
}

const sDefinition = styles({
  video: { flex: 1 },
  body: { gap: 16, paddingTop: 16 },
});
