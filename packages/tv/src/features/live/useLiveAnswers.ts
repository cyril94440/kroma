import type { PollView, VoteView } from '@kromatv/client/party';
import { pollToAnswer, voteToAnswer } from '@kromatv/core';
import type { LiveActions } from '@kromatv/core/react';
import { useT } from '@kromatv/ui';
import { toast } from '@kromatv/ui/kit';
import { useCallback, useState } from 'react';

/** The vote and the poll this viewer still has to answer, with what answering
 *  or putting one away does. A vote outranks a poll while both are open. */
export function useLiveAnswers(
  actions: LiveActions,
  vote: VoteView | null,
  poll: PollView | null,
  me: string,
) {
  const t = useT();
  const [dismissedVote, setDismissedVote] = useState<number | null>(null);
  const [dismissedPoll, setDismissedPoll] = useState<number | null>(null);

  const answerVote = useCallback(
    (approve: boolean) => {
      if (vote) setDismissedVote(vote.id);
      actions.vote(approve).then(
        () => toast({ message: t('live.voted') }),
        () => undefined,
      );
    },
    [actions, vote, t],
  );
  const answerPoll = useCallback(
    (value: string) => {
      if (poll) setDismissedPoll(poll.id);
      actions.pollVote(value).then(
        () => toast({ message: t('live.pollAnswered') }),
        () => undefined,
      );
    },
    [actions, poll, t],
  );

  const openVote = voteToAnswer(vote, me, dismissedVote);
  const openPoll = openVote ? null : pollToAnswer(poll, me, dismissedPoll);
  return {
    openVote,
    openPoll,
    answerVote,
    answerPoll,
    putAwayVote: () => vote && setDismissedVote(vote.id),
    putAwayPoll: () => poll && setDismissedPoll(poll.id),
  };
}
