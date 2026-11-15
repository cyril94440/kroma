import type { Clip, SportEvent } from '@kromatv/client/sport';
import { errorCode, pronoErrorKey } from '@kromatv/core';
import type { LiveActions, ZapOutcome } from '@kromatv/core/react';
import { toast } from '@kromatv/ui/kit';
import { useRouter } from 'expo-router';
import { useMemo } from 'react';
import { useT } from '#mobile/lib/i18n';
import { clipHref } from '#mobile/lib/clip';
import { useClient } from '#mobile/lib/session';

const ZAP_MESSAGE = {
  voteStarted: 'live.zapStarted',
  switchedDirectly: 'live.zapDirect',
  refused: 'live.zapRefused',
} as const satisfies Record<ZapOutcome, string>;

interface LiveHandlers {
  zapByName: (name: string) => void;
  reply: (text: string) => void;
  react: (emoji: string) => void;
  voteProno: (id: string, score: string) => void;
  watchSport: (event: SportEvent) => void;
  playClip: (clip: Clip) => void;
}

/** What a tap on the live screen does and how it is told back: each action
 *  speaks its outcome in a toast, so a failure is never silent. */
export function useLiveHandlers(actions: LiveActions): LiveHandlers {
  const t = useT();
  const router = useRouter();
  const client = useClient();
  return useMemo(() => {
    const say = (message: string) => toast({ message });
    return {
      zapByName: (name) => {
        actions.zapByName(name).then(
          (outcome) => say(t(ZAP_MESSAGE[outcome], { name })),
          () => say(t('live.zapRefused')),
        );
      },
      reply: (text) => {
        actions.chat(text).then(
          () => undefined,
          () => say(t('live.sendFailed')),
        );
      },
      react: (emoji) => void actions.react(emoji).catch(() => undefined),
      voteProno: (id, score) => {
        actions.pronoVote(id, score).then(
          () => say(t('live.prono.saved')),
          (error: unknown) => say(t(pronoErrorKey(errorCode(error)))),
        );
      },
      watchSport: (event) => {
        const ref = event.chaine?.ref;
        if (!ref) {
          say(t('live.sport.noChannel'));
          return;
        }
        actions.watchSport(ref).then(
          () => say(t('live.sport.watching', { channel: event.chaine?.nom ?? '' })),
          () => say(t('live.sport.refused')),
        );
      },
      playClip: (clip) =>
        router.push(
          clipHref({
            uri: client.sport.clipUrl(clip),
            title: `${clip.minute} ${clip.buteur}`.trim(),
            subtitle: `${clip.dom} ${clip.score} ${clip.ext}`,
          }) as never,
        ),
    };
  }, [actions, client, router, t]);
}
