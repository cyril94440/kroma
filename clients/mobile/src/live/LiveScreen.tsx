import type { ChannelView } from '@kromatv/client/live';
import { liveBadges, liveStage, playlistKey } from '@kromatv/core';
import { Box, Button, Spinner, styles, Text, useStyles } from '@kromatv/ui/kit';
import { Redirect } from 'expo-router';
import { VideoView } from 'expo-video';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { ScrollView } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { useT } from '#mobile/lib/i18n';
import { useGutters } from '#mobile/lib/layout';
import { LiveChannels } from '#mobile/live/LiveChannels';
import { LiveChat } from '#mobile/live/LiveChat';
import { LiveMatch } from '#mobile/live/LiveMatch';
import { PollCard, ReactionsRow, VoteBanner } from '#mobile/live/LivePanels';
import { useLiveEngine } from '#mobile/live/use-live-engine';
import { useLiveScreen } from '#mobile/live/use-live-screen';

type Section = 'chat' | 'channels' | 'match';

export default function LiveScreen() {
  const s = useStyles(sDefinition);
  const t = useT();
  const insets = useSafeAreaInsets();
  const gutters = useGutters();
  const { client, room, modules, ready } = useLiveScreen();
  const { status, party, sport, now } = room;
  const playlist = playlistKey(status);
  const url = useMemo(
    () => (playlist ? client.live.playlistUrl(playlist) : null),
    [client, playlist],
  );
  const engine = useLiveEngine(url);
  const [section, setSection] = useState<Section>('chat');
  const [frameTime, setFrameTime] = useState<number | null>(null);
  const me = party.state?.me.userId ?? '';
  const vote = party.state?.vote ?? null;
  const poll = party.state?.poll ?? null;
  const match = sport.state?.match ?? null;
  const stage = room.failed || engine.failed ? 'error' : liveStage(status);
  const badge = status ? liveBadges(status)[0] : undefined;
  const { frameTimeMs } = engine;

  useEffect(() => {
    setFrameTime(frameTimeMs());
  }, [frameTimeMs, now]);

  const zap = useCallback(
    (channel: ChannelView) => {
      const run = modules.party
        ? client.party.requestSwitch(channel.id, channel.name)
        : client.live.switchTo(channel.id);
      run.catch(() => undefined);
    },
    [client, modules.party],
  );

  const sections: Section[] = [
    ...(modules.party ? (['chat'] as const) : []),
    'channels',
    ...(modules.sport && match ? (['match'] as const) : []),
  ];
  const shown = sections.includes(section) ? section : 'channels';

  if (ready && !modules.live) return <Redirect href="/" />;

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
        {stage === 'playing' && !engine.waiting ? null : (
          <Box absolute top={0} right={0} bottom={0} left={0} center gap={12} pointerEvents="none">
            {stage === 'error' || stage === 'offAir' ? null : <Spinner size={36} />}
            <Text variant="label" color="white" textAlign="center">
              {stage === 'switching'
                ? t('live.switching')
                : stage === 'offAir'
                  ? t('live.offAir', { name: status?.offAir?.name ?? status?.channel?.name ?? '' })
                  : stage === 'error'
                    ? t('live.error')
                    : t('live.standby')}
            </Text>
          </Box>
        )}
      </Box>
      <ScrollView
        contentContainerStyle={[s.body, gutters.style, { paddingBottom: insets.bottom + 96 }]}
      >
        <Box row justify="space-between" align="center">
          <Box flex>
            <Text variant="title">{status?.channel?.name ?? t('nav.live')}</Text>
            <Text variant="meta" color="textMuted">
              {[status?.now?.title, badge ? t(`live.badge.${badge}`) : null]
                .filter(Boolean)
                .join('  ·  ')}
            </Text>
          </Box>
          {engine.behind ? (
            <Button
              icon="player-track-next"
              label={t('player.backToLive')}
              variant="primary"
              onPress={engine.goLive}
            />
          ) : (
            <Text variant="meta" color="textMuted">
              {t('live.viewers', { count: status?.viewers.length ?? 0 })}
            </Text>
          )}
        </Box>
        {vote ? (
          <VoteBanner
            vote={vote}
            userId={me}
            now={now}
            onAnswer={(approve) => void client.party.vote(approve).catch(() => undefined)}
          />
        ) : null}
        {poll && !(me in poll.votes) ? (
          <PollCard
            poll={poll}
            userId={me}
            now={now}
            onAnswer={(value) => void client.party.pollVote(value).catch(() => undefined)}
          />
        ) : null}
        {modules.party ? (
          <ReactionsRow
            onReact={(emoji) => void client.party.floating(emoji).catch(() => undefined)}
          />
        ) : null}
        <Box row gap={8}>
          {sections.map((entry) => (
            <Button
              key={entry}
              label={t(`live.${entry === 'channels' ? 'channels' : entry}`)}
              variant={entry === shown ? 'primary' : 'scrim'}
              onPress={() => setSection(entry)}
            />
          ))}
        </Box>
        {shown === 'chat' ? (
          <LiveChat
            messages={party.state?.messages ?? []}
            onSend={(text) => void client.party.chat(text).catch(() => undefined)}
          />
        ) : null}
        {shown === 'channels' ? (
          <LiveChannels client={client} currentId={status?.channel?.id ?? null} onPick={zap} />
        ) : null}
        {shown === 'match' ? (
          <LiveMatch match={match} frameTimeMs={frameTime} now={now + sport.clockOffsetMs} />
        ) : null}
      </ScrollView>
    </Box>
  );
}

const sDefinition = styles({
  video: { flex: 1 },
  body: { gap: 16, paddingTop: 16 },
});
