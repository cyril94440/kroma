import type { VoteView } from '@kromatv/client/party';
import { voteOf, voteWaiting } from '@kromatv/core';
import { useT } from '@kromatv/ui';
import { Drawer, FocusColumn, ListRow, styles, Text, useStyles } from '@kromatv/ui/kit';

const sDefinition = styles({ column: { gap: 14 } });

/** A zapping vote, answered with the remote: For or Against. The requester
 *  never votes on their own request, so for them the drawer is never opened. */
export function VoteDrawer({
  vote,
  userId,
  now,
  onAnswer,
  onClose,
}: Readonly<{
  vote: VoteView | null;
  userId: string;
  now: number;
  onAnswer: (approve: boolean) => void;
  onClose: () => void;
}>) {
  const s = useStyles(sDefinition);
  const t = useT();
  const mine = vote ? voteOf(vote, userId) : null;
  const seconds = vote ? Math.max(0, Math.ceil((vote.endsAt - now) / 1000)) : 0;
  return (
    <Drawer.Root
      open={vote !== null}
      onClose={onClose}
      title={t('live.voteTitle')}
      width="md"
      floating
    >
      <Drawer.Panel>
        {vote ? (
          <FocusColumn style={s.column}>
            <Text variant="headingTv">
              {t('live.voteBy', { name: vote.by?.name ?? '', channel: vote.label })}
            </Text>
            <Text variant="labelTv" color="textMuted">
              {t('live.voteEnds', { seconds })}
            </Text>
            <Text variant="labelTv" color="textMuted">
              {t('live.voteTally', { yes: vote.tally.yes, no: vote.tally.no })}
              {voteWaiting(vote) > 0 ? ` (${voteWaiting(vote)})` : ''}
            </Text>
            <ListRow.Root
              icon="thumb-up"
              size="tv"
              ground="surface"
              chevron={false}
              autoFocus
              selected={mine === true}
              onPress={() => onAnswer(true)}
            >
              <ListRow.Label>{t('live.voteFor')}</ListRow.Label>
            </ListRow.Root>
            <ListRow.Root
              icon="thumb-down"
              size="tv"
              ground="surface"
              chevron={false}
              selected={mine === false}
              onPress={() => onAnswer(false)}
            >
              <ListRow.Label>{t('live.voteAgainst')}</ListRow.Label>
            </ListRow.Root>
          </FocusColumn>
        ) : null}
      </Drawer.Panel>
    </Drawer.Root>
  );
}
