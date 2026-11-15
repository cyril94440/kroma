import { REACTIONS } from '@kromatv/core';
import { useT } from '@kromatv/ui';
import { Button, Drawer, FocusRegion } from '@kromatv/ui/kit';

/** The reactions everyone sees float over the picture. */
export function ReactionsDrawer({
  open,
  onClose,
  onReact,
}: Readonly<{ open: boolean; onClose: () => void; onReact: (emoji: string) => void }>) {
  const t = useT();
  return (
    <Drawer.Root open={open} onClose={onClose} title={t('live.reactions')} width="md" floating>
      <Drawer.Panel>
        <FocusRegion>
          {REACTIONS.map((emoji, at) => (
            <Button
              key={emoji}
              variant="scrim"
              label={emoji}
              autoFocus={at === 0}
              onPress={() => onReact(emoji)}
            />
          ))}
        </FocusRegion>
      </Drawer.Panel>
    </Drawer.Root>
  );
}
