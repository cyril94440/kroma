import type { ChannelView } from '@kromatv/client/live';
import type { Clip, SportEvent } from '@kromatv/client/sport';
import { errorCode, pronoErrorKey } from '@kromatv/core';
import type { LiveActions, ZapOutcome } from '@kromatv/core/react';
import { useT } from '@kromatv/ui';
import { toast } from '@kromatv/ui/kit';
import { useCallback, useMemo } from 'react';
import { useClient, useNav } from '#tv/app/router';

const ZAP_MESSAGE = {
  voteStarted: 'live.zapStarted',
  switchedDirectly: 'live.zapDirect',
  refused: 'live.zapRefused',
} as const satisfies Record<ZapOutcome, string>;

interface LiveHandlers {
  zap: (channel: ChannelView) => void;
  zapByName: (name: string) => void;
  reply: (text: string) => void;
  react: (emoji: string) => void;
  voteProno: (id: string, score: string) => void;
  watchSport: (event: SportEvent) => void;
  playClip: (clip: Clip) => void;
}

/** What a press on the live screen does and how it is told back: each action
 *  speaks its outcome in a toast, so a failure is never silent. */
export function useLiveHandlers(actions: LiveActions, closePanel: () => void): LiveHandlers {
  const t = useT();
  const nav = useNav();
  const client = useClient();
  const zapOutcome = useCallback(
    (name: string) => (outcome: ZapOutcome) =>
      toast({ message: t(ZAP_MESSAGE[outcome], { name }) }),
    [t],
  );
  const zapFailed = useCallback(() => toast({ message: t('live.zapRefused') }), [t]);

  return useMemo(
    () => ({
      zap: (channel) => {
        closePanel();
        actions.zap(channel).then(zapOutcome(channel.name), zapFailed);
      },
      zapByName: (name) => {
        actions.zapByName(name).then(zapOutcome(name), zapFailed);
      },
      reply: (text) => {
        actions.chat(text).then(
          () => toast({ message: t('live.sent') }),
          () => toast({ message: t('live.sendFailed') }),
        );
      },
      react: (emoji) => void actions.react(emoji).catch(() => undefined),
      voteProno: (id, score) => {
        actions.pronoVote(id, score).then(
          () => toast({ message: t('live.prono.saved') }),
          (error: unknown) => toast({ message: t(pronoErrorKey(errorCode(error))) }),
        );
      },
      watchSport: (event) => {
        const ref = event.chaine?.ref;
        if (!ref) {
          toast({ message: t('live.sport.noChannel') });
          return;
        }
        closePanel();
        actions.watchSport(ref).then(
          () => toast({ message: t('live.sport.watching', { channel: event.chaine?.nom ?? '' }) }),
          () => toast({ message: t('live.sport.refused') }),
        );
      },
      playClip: (clip) => {
        closePanel();
        nav.go('clip', {
          url: client.sport.clipUrl(clip),
          title: `${clip.minute} ${clip.buteur}`.trim(),
          subtitle: `${clip.dom} ${clip.score} ${clip.ext}`,
        });
      },
    }),
    [actions, client, closePanel, nav, t, zapFailed, zapOutcome],
  );
}
