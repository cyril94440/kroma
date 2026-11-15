import type { PollView, VoteView } from '@kromatv/client/party';
import { REACTIONS, voteOf, voteWaiting } from '@kromatv/core';
import { Box, Button, Text } from '@kromatv/ui/kit';
import { useT } from '#mobile/lib/i18n';

/** The zapping vote as a banner under the picture: who asked for what, the
 *  tally, and For or Against. The requester does not vote on their own request. */
export function VoteBanner({
  vote,
  userId,
  now,
  onAnswer,
}: Readonly<{
  vote: VoteView;
  userId: string;
  now: number;
  onAnswer: (approve: boolean) => void;
}>) {
  const t = useT();
  const mine = voteOf(vote, userId);
  const own = vote.by?.userId === userId;
  const seconds = Math.max(0, Math.ceil((vote.endsAt - now) / 1000));
  return (
    <Box gap={10} p={16} radius="lg" bg="surface2">
      <Text variant="label">
        {t('live.voteBy', { name: vote.by?.name ?? '', channel: vote.label })}
      </Text>
      <Text variant="meta" color="textMuted">
        {`${t('live.voteEnds', { seconds })}  ·  ${t('live.voteTally', { yes: vote.tally.yes, no: vote.tally.no })}${voteWaiting(vote) > 0 ? ` (${voteWaiting(vote)})` : ''}`}
      </Text>
      {own ? null : (
        <Box row gap={10}>
          <Button
            icon="thumb-up"
            label={t('live.voteFor')}
            variant={mine === true ? 'primary' : 'scrim'}
            onPress={() => onAnswer(true)}
          />
          <Button
            icon="thumb-down"
            label={t('live.voteAgainst')}
            variant={mine === false ? 'primary' : 'scrim'}
            onPress={() => onAnswer(false)}
          />
        </Box>
      )}
    </Box>
  );
}

/** A poll with one button per answer and how many chose each. */
export function PollCard({
  poll,
  userId,
  now,
  onAnswer,
}: Readonly<{ poll: PollView; userId: string; now: number; onAnswer: (value: string) => void }>) {
  const t = useT();
  const mine = poll.votes[userId];
  const seconds = Math.max(0, Math.ceil((poll.endsAt - now) / 1000));
  return (
    <Box gap={10} p={16} radius="lg" bg="surface2">
      <Text variant="label">{poll.question}</Text>
      <Text variant="meta" color="textMuted">
        {t('live.pollEnds', { seconds })}
      </Text>
      <Box row gap={10} wrap>
        {poll.options.map((option) => (
          <Button
            key={option}
            label={`${option}  ${poll.byAnswer[option]?.length ?? 0}`}
            variant={mine === option ? 'primary' : 'scrim'}
            onPress={() => onAnswer(option)}
          />
        ))}
      </Box>
    </Box>
  );
}

export function ReactionsRow({ onReact }: Readonly<{ onReact: (emoji: string) => void }>) {
  return (
    <Box row gap={8}>
      {REACTIONS.map((emoji) => (
        <Button key={emoji} label={emoji} variant="scrim" onPress={() => onReact(emoji)} />
      ))}
    </Box>
  );
}
