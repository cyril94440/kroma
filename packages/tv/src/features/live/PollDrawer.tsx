import type { PollView } from '@kromatv/client/party';
import { useT } from '@kromatv/ui';
import { Drawer, FocusColumn, ListRow, styles, Text, useStyles } from '@kromatv/ui/kit';

const sDefinition = styles({ column: { gap: 14 } });

/** A poll with its answers as rows. A free-text poll has no row to press, so
 *  it is answered from a phone and this drawer only shows the question. */
export function PollDrawer({
  poll,
  userId,
  now,
  onAnswer,
  onClose,
}: Readonly<{
  poll: PollView | null;
  userId: string;
  now: number;
  onAnswer: (value: string) => void;
  onClose: () => void;
}>) {
  const s = useStyles(sDefinition);
  const t = useT();
  const answer = poll ? poll.votes[userId] : undefined;
  const seconds = poll ? Math.max(0, Math.ceil((poll.endsAt - now) / 1000)) : 0;
  return (
    <Drawer.Root open={poll !== null} onClose={onClose} title={t('live.poll')} width="md" floating>
      <Drawer.Panel>
        {poll ? (
          <FocusColumn style={s.column}>
            <Text variant="headingTv">{poll.question}</Text>
            <Text variant="labelTv" color="textMuted">
              {t('live.pollEnds', { seconds })}
            </Text>
            {poll.options.map((option, at) => (
              <ListRow.Root
                key={option}
                size="tv"
                ground="surface"
                chevron={false}
                autoFocus={at === 0}
                selected={answer === option}
                onPress={() => onAnswer(option)}
              >
                <ListRow.Label>{option}</ListRow.Label>
                <ListRow.Trailing>
                  <Text variant="labelTv" color="accentText">
                    {poll.byAnswer[option]?.length ?? 0}
                  </Text>
                </ListRow.Trailing>
              </ListRow.Root>
            ))}
          </FocusColumn>
        ) : null}
      </Drawer.Panel>
    </Drawer.Root>
  );
}
