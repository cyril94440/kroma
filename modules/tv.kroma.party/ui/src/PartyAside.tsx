import { ModuleLoading } from '@kromatv/module-sdk';
import { Box, Button, Row } from '@kromatv/ui/kit';
import { ChatPanel } from './chat-panel';
import { PollPanel } from './poll-panel';
import { PresencePanel } from './presence-panel';
import { REACTIONS } from './reactions';
import { SwitchPanel } from './switch-panel';
import { SyncPanel } from './sync-panel';
import { AsideProps, type SyncPlayer } from './sync-player';
import { useParty } from './use-party';
import { useSyncSession } from './use-sync-session';
import { VotePanel } from './vote-panel';
import { VoicePanel } from './voice-panel';

function Panel({ props, player }: Readonly<{ props: AsideProps; player: SyncPlayer }>) {
  const { api, model, now } = useParty();
  const session = useSyncSession(player, props.playing, props.channel?.id ?? null, api, model);
  const state = model.state;
  if (!state) return <ModuleLoading panels={1} />;
  return (
    <Box data-testid="party-aside">
      <VotePanel
        api={api}
        vote={state.vote}
        closed={model.lastClosed}
        me={state.me.userId}
        now={now}
      />
      <Box mt={16}>
        <SyncPanel session={session} group={state.sync} />
      </Box>
      <Box mt={16}>
        <VoicePanel watching />
      </Box>
      <Box mt={16}>
        <ChatPanel api={api} state={state} floating={model.floating} />
      </Box>
      <Row wrap gap={6} mt={8}>
        {REACTIONS.map((emoji) => (
          <Button
            key={emoji}
            label={`${emoji}`}
            variant="glass"
            size="sm"
            onPress={() => void api.floating(emoji)}
          />
        ))}
      </Row>
      <Box mt={16}>
        <SwitchPanel api={api} admin={state.me.admin} />
      </Box>
      <Box mt={16}>
        <PollPanel
          api={api}
          poll={state.poll}
          last={state.lastPoll}
          me={state.me.userId}
          admin={state.me.admin}
          now={now}
        />
      </Box>
      <Box mt={16}>
        <PresencePanel api={api} state={state} />
      </Box>
    </Box>
  );
}

export default function PartyAside(props: Readonly<Record<string, unknown>>) {
  const parsed = AsideProps.safeParse(props);
  return parsed.success ? <Panel props={parsed.data} player={props.player as SyncPlayer} /> : null;
}
