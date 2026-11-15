import { ModuleFailed, ModuleLoading, useT } from '@kromatv/module-sdk';
import { Box, PageHeader, Row } from '@kromatv/ui/kit';
import { ChatPanel } from './chat-panel';
import { IdeasPanel } from './ideas-panel';
import { PollPanel } from './poll-panel';
import { PresencePanel } from './presence-panel';
import { SwitchPanel } from './switch-panel';
import { useParty } from './use-party';
import { VotePanel } from './vote-panel';
import { VoicePanel } from './voice-panel';

export default function PartyPage() {
  const t = useT();
  const { api, model, failed, now } = useParty();
  const state = model.state;
  if (!state) return failed ? <ModuleFailed /> : <ModuleLoading panels={2} />;

  return (
    <>
      <PageHeader.Root>
        <PageHeader.Title>{t('party.title')}</PageHeader.Title>
        <PageHeader.Subtitle>{t('party.subtitle')}</PageHeader.Subtitle>
      </PageHeader.Root>
      <Row wrap gap={24} mt={24} align="flex-start">
        <Box flex minW={320} style={{ flexBasis: 480 }}>
          <ChatPanel api={api} state={state} floating={model.floating} />
        </Box>
        <Box flex minW={280} style={{ flexBasis: 320 }}>
          <VotePanel
            api={api}
            vote={state.vote}
            closed={model.lastClosed}
            me={state.me.userId}
            now={now}
          />
          <Box mt={20}>
            <VoicePanel watching={false} />
          </Box>
          <Box mt={20}>
            <SwitchPanel api={api} admin={state.me.admin} />
          </Box>
          <Box mt={20}>
            <PollPanel
              api={api}
              poll={state.poll}
              last={state.lastPoll}
              me={state.me.userId}
              admin={state.me.admin}
              now={now}
            />
          </Box>
          <Box mt={20}>
            <PresencePanel api={api} state={state} />
          </Box>
          <Box mt={20}>
            <IdeasPanel api={api} admin={state.me.admin} version={model.ideasSeq} />
          </Box>
        </Box>
      </Row>
    </>
  );
}
