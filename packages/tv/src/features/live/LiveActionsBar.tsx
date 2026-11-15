import type { LiveModules } from '@kromatv/core';
import type { LiveSync } from '@kromatv/core/react';
import { useT } from '@kromatv/ui';
import { Button } from '@kromatv/ui/kit';

export type Panel = 'channels' | 'chat' | 'reactions' | 'match' | 'sport' | 'ideas';

/** The row of buttons over the picture, one per thing the live screen can open;
 *  a module that is absent leaves its buttons out. */
export function LiveActionsBar({
  modules,
  hasMatch,
  sync,
  onOpen,
}: Readonly<{
  modules: LiveModules;
  hasMatch: boolean;
  sync: LiveSync;
  onOpen: (panel: Panel) => void;
}>) {
  const t = useT();
  return (
    <>
      <Button icon="list" label={t('live.channels')} variant="scrim" onPress={() => onOpen('channels')} />
      {modules.party ? (
        <Button icon="message" label={t('live.chat')} variant="scrim" onPress={() => onOpen('chat')} />
      ) : null}
      {modules.party ? (
        <Button
          icon="mood-smile"
          label={t('live.reactions')}
          variant="scrim"
          onPress={() => onOpen('reactions')}
        />
      ) : null}
      {modules.party ? (
        <Button
          icon="users-group"
          label={t(sync.on ? 'live.sync.leave' : 'live.sync.join')}
          variant={sync.on ? 'primary' : 'scrim'}
          pressed={sync.on}
          onPress={sync.on ? sync.leave : sync.join}
        />
      ) : null}
      {modules.sport && hasMatch ? (
        <Button icon="ball-football" label={t('live.match')} variant="scrim" onPress={() => onOpen('match')} />
      ) : null}
      {modules.sport ? (
        <Button icon="trophy" label={t('live.sport')} variant="scrim" onPress={() => onOpen('sport')} />
      ) : null}
      {modules.party ? (
        <Button icon="bulb" label={t('live.ideas')} variant="scrim" onPress={() => onOpen('ideas')} />
      ) : null}
    </>
  );
}
