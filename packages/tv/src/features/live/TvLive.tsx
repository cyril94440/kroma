import type { ChannelView } from '@kromatv/client/live';
import { liveBadges } from '@kromatv/core';
import { Player, TV_FLAGS, useT } from '@kromatv/ui';
import { Box, Button, Text, toast } from '@kromatv/ui/kit';
import { type ReactNode, useCallback, useEffect, useMemo, useState } from 'react';
import { useEnv } from '#tv/app/providers/env';
import { useClient, useNav } from '#tv/app/router';
import { ChannelDrawer } from '#tv/features/live/ChannelDrawer';
import { ChatDrawer } from '#tv/features/live/ChatDrawer';
import { ChatOverlay } from '#tv/features/live/ChatOverlay';
import type { LiveBackend, LiveEngine } from '#tv/features/live/engine/liveEngine';
import { useLiveController } from '#tv/features/live/engine/useLiveController';
import { useLivePlayback } from '#tv/features/live/engine/useLivePlayback';
import { LiveStateCard } from '#tv/features/live/LiveStateCard';
import { MatchDrawer } from '#tv/features/live/MatchDrawer';
import { PollDrawer } from '#tv/features/live/PollDrawer';
import { ReactionsDrawer } from '#tv/features/live/ReactionsDrawer';
import { useLiveScreen } from '#tv/features/live/useLiveScreen';
import { useVerdictToasts } from '#tv/features/live/useVerdictToasts';
import { VoteDrawer } from '#tv/features/live/VoteDrawer';

type Panel = 'channels' | 'chat' | 'reactions' | 'match';

const noStoryboard = () => null;

/** The live-TV screen: the stream in the shared player chrome, with the
 *  channel list, the chat, the reactions and the match panel one press away,
 *  and the zapping vote and polls answered with the remote. A module that is
 *  absent leaves its panel out; the picture never depends on any of them. */
export function TvLive<E extends LiveEngine>({
  backend,
}: Readonly<{ backend: LiveBackend<E> }>): ReactNode {
  const nav = useNav();
  const t = useT();
  const client = useClient();
  const { mousePointer } = useEnv();
  const { room, playlistUrl, modules, modulesReady } = useLiveScreen();
  const live = useLivePlayback(backend, playlistUrl);
  const controller = useLiveController(live);
  const [panel, setPanel] = useState<Panel | null>(null);
  const [dismissedVote, setDismissedVote] = useState<number | null>(null);
  const [dismissedPoll, setDismissedPoll] = useState<number | null>(null);
  const [frameTime, setFrameTime] = useState<number | null>(null);
  const { engineRef } = live;
  const { status, party, sport, now } = room;
  const me = party.state?.me.userId ?? '';
  const vote = party.state?.vote ?? null;
  const poll = party.state?.poll ?? null;
  const match = sport.state?.match ?? null;

  useEffect(() => {
    if (modulesReady && !modules.live) nav.home();
  }, [modulesReady, modules.live, nav]);

  useEffect(() => {
    setFrameTime(engineRef.current?.frameTimeMs() ?? null);
  }, [engineRef, now]);

  useVerdictToasts(party.lastClosed);

  const flags = useMemo(
    () => ({ ...TV_FLAGS, pointer: mousePointer, tracks: false, live: true }),
    [mousePointer],
  );

  const zap = useCallback(
    (channel: ChannelView) => {
      setPanel(null);
      const run = modules.party
        ? client.party.requestSwitch(channel.id, channel.name).then((out) => out.outcome)
        : client.live.switchTo(channel.id).then(() => 'switchedDirectly' as const);
      run.then(
        (outcome) =>
          toast({
            message:
              outcome === 'voteStarted'
                ? t('live.zapStarted', { name: channel.name })
                : outcome === 'switchedDirectly'
                  ? t('live.zapDirect', { name: channel.name })
                  : t('live.zapRefused'),
          }),
        () => toast({ message: t('live.zapRefused') }),
      );
    },
    [client, modules.party, t],
  );

  const answerVote = useCallback(
    (approve: boolean) => {
      if (vote) setDismissedVote(vote.id);
      client.party.vote(approve).then(
        () => toast({ message: t('live.voted') }),
        () => undefined,
      );
    },
    [client, vote, t],
  );

  const answerPoll = useCallback(
    (value: string) => {
      if (poll) setDismissedPoll(poll.id);
      client.party.pollVote(value).then(
        () => toast({ message: t('live.pollAnswered') }),
        () => undefined,
      );
    },
    [client, poll, t],
  );

  const voteOpen = vote !== null && vote.id !== dismissedVote && vote.by?.userId !== me && me !== '' && !(me in vote.votes);
  const pollOpen = poll !== null && poll.id !== dismissedPoll && !voteOpen && me !== '' && !(me in poll.votes);
  const closePanel = useCallback(() => setPanel(null), []);
  const badge = status ? liveBadges(status)[0] : undefined;
  const title = status?.channel?.name ?? t('nav.live');
  const next = status?.next ? `${t('live.next')} ${status.next.title}` : null;
  const subtitle = [status?.now?.title, next].filter(Boolean).join('  ·  ');

  return (
    <>
      <Player.Root controller={controller} flags={flags} title={title} onClose={nav.back}>
        <Player.Title>{title}</Player.Title>
        {subtitle ? <Player.Subtitle>{subtitle}</Player.Subtitle> : null}
        {badge ? <Player.Warning>{t(`live.${badge}`)}</Player.Warning> : null}
        <Player.Actions>
          <Button icon="list" label={t('live.channels')} variant="scrim" onPress={() => setPanel('channels')} />
          {modules.party ? (
            <Button icon="message" label={t('live.chat')} variant="scrim" onPress={() => setPanel('chat')} />
          ) : null}
          {modules.party ? (
            <Button icon="mood-smile" label={t('live.reactions')} variant="scrim" onPress={() => setPanel('reactions')} />
          ) : null}
          {modules.sport && match ? (
            <Button icon="ball-football" label={t('live.match')} variant="scrim" onPress={() => setPanel('match')} />
          ) : null}
        </Player.Actions>
        <Player.Transport tileAt={noStoryboard} />
        <Player.Media>
          <backend.Surface live={live} title={title} />
        </Player.Media>
        <LiveStateCard status={status} failed={room.failed} />
        {status && status.viewers.length > 0 ? (
          <Box absolute right={64} top={150} z={12} pointerEvents="none">
            <Text variant="labelTv" color="textMuted">
              {t('live.viewers', { count: status.viewers.length })}
            </Text>
          </Box>
        ) : null}
        {modules.party && party.state ? (
          <ChatOverlay messages={party.state.messages} floating={party.floating} now={now} />
        ) : null}
      </Player.Root>
      <ChannelDrawer
        open={panel === 'channels'}
        currentId={status?.channel?.id ?? null}
        onClose={closePanel}
        onPick={zap}
      />
      <ChatDrawer
        open={panel === 'chat'}
        messages={party.state?.messages ?? []}
        onClose={closePanel}
        onReply={(text) => void client.party.chat(text).catch(() => undefined)}
      />
      <ReactionsDrawer
        open={panel === 'reactions'}
        onClose={closePanel}
        onReact={(emoji) => void client.party.floating(emoji).catch(() => undefined)}
      />
      <MatchDrawer
        open={panel === 'match'}
        match={match}
        frameTimeMs={frameTime}
        now={now + sport.clockOffsetMs}
        onClose={closePanel}
      />
      <VoteDrawer
        vote={voteOpen ? vote : null}
        userId={me}
        now={now}
        onAnswer={answerVote}
        onClose={() => vote && setDismissedVote(vote.id)}
      />
      <PollDrawer
        poll={pollOpen ? poll : null}
        userId={me}
        now={now}
        onAnswer={answerPoll}
        onClose={() => poll && setDismissedPoll(poll.id)}
      />
    </>
  );
}
