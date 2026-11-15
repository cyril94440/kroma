import type { FeedItem } from '@kromatv/core';
import type { Translate } from '@kromatv/i18n';

export function feedLine(item: FeedItem, t: Translate): string {
  switch (item.kind) {
    case 'but':
      return [
        t(item.ownGoal ? 'live.feed.ownGoal' : 'live.feed.but'),
        item.player,
        item.assist ? `(${item.assist})` : null,
      ]
        .filter(Boolean)
        .join(' ');
    case 'remplacement':
      return `${t('live.feed.remplacement')} ${item.cameOn ?? '?'} / ${item.wentOff ?? '?'}`;
    case 'jaune':
      return `${t('live.feed.jaune')} ${item.player ?? ''}`.trim();
    default:
      return `${t('live.feed.rouge')} ${item.player ?? ''}`.trim();
  }
}
