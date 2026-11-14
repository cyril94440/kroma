import type { Translate } from '@kromatv/i18n';
import type { StreamFailure } from '#web/features/playback/stream-failure';

export function failureCopy(t: Translate, failure: StreamFailure): { error: string; hint: string } {
  if (failure === 'denied') {
    return { error: t('player.streamDenied'), hint: t('player.streamDeniedHint') };
  }
  if (failure === 'missing') {
    return { error: t('player.streamMissing'), hint: t('player.streamMissingHint') };
  }
  if (failure !== 'broken') {
    return {
      error: t(failure.messageKey, failure.messageVars),
      hint: t('player.sourceFailedHint'),
    };
  }
  return { error: t('player.cantPlay'), hint: t('player.cantPlayHint') };
}
