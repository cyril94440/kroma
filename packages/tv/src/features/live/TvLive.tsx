import { liveBadges } from '@kromatv/core';
import { useIdeas, useLiveActions, useLiveSync } from '@kromatv/core/react';
import { Player, TV_FLAGS, useT } from '@kromatv/ui';
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
import { IdeasDrawer } from '#tv/features/live/IdeasDrawer';
import { LiveActionsBar, type Panel } from '#tv/features/live/LiveActionsBar';
import { MatchDrawer } from '#tv/features/live/match/MatchDrawer';
import { PollDrawer } from '#tv/features/live/PollDrawer';
import { ReactionsDrawer } from '#tv/features/live/ReactionsDrawer';
import { SportDrawer } from '#tv/features/live/sport/SportDrawer';
import { useLiveAnswers } from '#tv/features/live/useLiveAnswers';
import { useLiveHandlers } from '#tv/features/live/useLiveHandlers';
import { useLiveScreen } from '#tv/features/live/useLiveScreen';
import { useSyncToasts } from '#tv/features/live/useSyncToasts';
import { useVerdictToasts } from '#tv/features/live/useVerdictToasts';
import { ViewersCard } from '#tv/features/live/ViewersCard';
import { VoteDrawer } from '#tv/features/live/VoteDrawer';

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
  const actions = useLiveActions(client, modules);
  const live = useLivePlayback(backend, playlistUrl);
  const controller = useLiveController(live);
  const [panel, setPanel] = useState<Panel | null>(null);
  const { engineRef } = live;
  const frameTimeMs = useCallback(() => engineRef.current?.frameTimeMs() ?? null, [engineRef]);
  const { status, party, sport, now } = room;
  const me = party.state?.me.userId ?? '';
  const vote = party.state?.vote ?? null;
  const poll = party.state?.poll ?? null;
  const sportState = sport.state;
  const match = sportState?.match ?? null;
  const closePanel = useCallback(() => setPanel(null), []);
  const answers = useLiveAnswers(actions, vote, poll, me);
  const { openVote, openPoll } = answers;
  const handlers = useLiveHandlers(actions, closePanel);
  const sync = useLiveSync({
    client,
    source: engineRef,
    party,
    playing: live.playing,
    channelId: status?.channel?.id ?? null,
  });
  const { ideas } = useIdeas(client, party.ideasSeq, modules.party && panel === 'ideas');

  useEffect(() => {
    if (modulesReady && !modules.live) nav.home();
  }, [modulesReady, modules.live, nav]);

  useVerdictToasts(party.lastClosed);
  useSyncToasts(sync.notice);

  const flags = useMemo(
    () => ({ ...TV_FLAGS, pointer: mousePointer, tracks: false, live: true }),
    [mousePointer],
  );

  const matchData = useMemo(
    () => ({
      match,
      prono: sportState?.prono ?? null,
      clips: sportState?.clips ?? [],
      me,
      postSeenAt: sport.postSeen?.at ?? null,
    }),
    [match, sportState, me, sport.postSeen],
  );
  const agent = party.state?.config.agentName ?? '';
  const activity = party.state?.activity
    ? t('live.activity', { name: agent, label: party.state.activity })
    : null;
  const badge = status ? liveBadges(status)[0] : undefined;
  const title = status?.channel?.name ?? t('nav.live');
  const next = status?.next ? `${t('live.next')} ${status.next.title}` : null;
  const subtitle = [status?.now?.title, next].filter(Boolean).join('  ·  ');
  const group = party.state?.sync ?? null;

  return (
    <>
      <Player.Root
        controller={controller}
        flags={flags}
        title={title}
        active={panel === null && !openVote && !openPoll}
        onClose={nav.back}
      >
        <Player.Title>{title}</Player.Title>
        {subtitle ? <Player.Subtitle>{subtitle}</Player.Subtitle> : null}
        {badge ? <Player.Warning>{t(`live.badge.${badge}`)}</Player.Warning> : null}
        <Player.Actions>
          <LiveActionsBar modules={modules} sync={sync} onOpen={setPanel} />
        </Player.Actions>
        <Player.Transport tileAt={noStoryboard} />
        <Player.Media>
          <backend.Surface live={live} title={title} />
        </Player.Media>
        <LiveStateCard status={status} failed={room.failed} />
        <ViewersCard status={status} syncOn={sync.on} group={group} />
        {modules.party && party.state ? (
          <ChatOverlay messages={party.state.messages} floating={party.floating} now={now} />
        ) : null}
      </Player.Root>
      <ChannelDrawer
        open={panel === 'channels'}
        currentId={status?.channel?.id ?? null}
        onClose={closePanel}
        onPick={handlers.zap}
      />
      <ChatDrawer
        open={panel === 'chat'}
        messages={party.state?.messages ?? []}
        activity={activity}
        onClose={closePanel}
        onReply={handlers.reply}
        onWatch={handlers.zapByName}
      />
      <ReactionsDrawer open={panel === 'reactions'} onClose={closePanel} onReact={handlers.react} />
      <MatchDrawer
        open={panel === 'match'}
        data={matchData}
        frameTimeMs={frameTimeMs}
        now={now + sport.clockOffsetMs}
        onClose={closePanel}
        onVote={handlers.voteProno}
        onPlayClip={handlers.playClip}
      />
      <SportDrawer
        open={panel === 'sport'}
        view={sportState?.view ?? null}
        client={client}
        onClose={closePanel}
        onWatch={handlers.watchSport}
      />
      <IdeasDrawer open={panel === 'ideas'} ideas={ideas} onClose={closePanel} />
      <VoteDrawer
        vote={openVote}
        userId={me}
        now={now}
        onAnswer={answers.answerVote}
        onClose={answers.putAwayVote}
      />
      <PollDrawer
        poll={openPoll}
        userId={me}
        now={now}
        onAnswer={answers.answerPoll}
        onClose={answers.putAwayPoll}
      />
    </>
  );
}
