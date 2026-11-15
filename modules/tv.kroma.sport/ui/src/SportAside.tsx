import { ModuleLoading, useT } from '@kromatv/module-sdk';
import { Box, Text } from '@kromatv/ui/kit';
import { CalibrateForm } from './calibrate-form';
import { ClipsPanel } from './clips-panel';
import { AsideProps, useFrameTime } from './frame-time';
import { clockText } from './match-panel';
import { MatchSummary } from './match-summary';
import { PronoPanel } from './prono-panel';
import { useMatchView } from './use-match-view';
import { useSport } from './use-sport';

const FEED_LIMIT = 6;

function Panel({ props }: Readonly<{ props: AsideProps }>) {
  const t = useT();
  const { api, model, serverNow, postSeenAt } = useSport();
  const pdt = useFrameTime(props.player);
  const state = model.state;
  const match = state?.match ?? null;
  const view = useMatchView(match, pdt, serverNow, postSeenAt);
  if (!state) return <ModuleLoading panels={1} />;
  if (!match && !state.prono) return null;
  return (
    <Box data-testid="sport-aside">
      {match && view ? (
        <>
          <Text variant="title" data-testid="sport-aside-score">
            {`${match.equipes.dom.abbr || match.equipes.dom.nom} ${view.score.dom} – ${view.score.ext} ${match.equipes.ext.abbr || match.equipes.ext.nom}`}
          </Text>
          <Text
            variant="meta"
            color="textDim"
          >{`${match.competition} · ${clockText(view, t)}`}</Text>
          <MatchSummary match={match} view={view} limit={FEED_LIMIT} />
          <Box mt={12}>
            <CalibrateForm api={api} getPositionMs={() => props.player.getPositionMs()} />
          </Box>
          <ClipsPanel clips={state.clips} pdt={pdt} now={serverNow} matchId={match.id} />
        </>
      ) : null}
      {state.prono ? (
        <Box mt={16}>
          <PronoPanel
            prono={state.prono}
            me={state.me.userId}
            api={api}
            season={state.prono.saison}
          />
        </Box>
      ) : null}
    </Box>
  );
}

export default function SportAside(props: Readonly<Record<string, unknown>>) {
  const parsed = AsideProps.safeParse(props);
  return parsed.success ? <Panel props={parsed.data} /> : null;
}
