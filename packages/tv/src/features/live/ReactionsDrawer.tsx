import { REACTIONS } from '@kromatv/core';
import { useT } from '@kromatv/ui';
import { Drawer, FocusRegion, styles, useStyles } from '@kromatv/ui/kit';
import { ReactionButton } from '#tv/features/live/ReactionButton';

const sDefinition = styles({ row: { flexDirection: 'row', flexWrap: 'wrap', gap: 20 } });

/** The reactions everyone sees float over the picture. */
export function ReactionsDrawer({
  open,
  onClose,
  onReact,
}: Readonly<{ open: boolean; onClose: () => void; onReact: (emoji: string) => void }>) {
  const s = useStyles(sDefinition);
  const t = useT();
  return (
    <Drawer.Root open={open} onClose={onClose} title={t('live.reactions')} width="md" floating>
      <Drawer.Panel>
        <FocusRegion style={s.row}>
          {REACTIONS.map(({ emoji, label }, at) => (
            <ReactionButton
              key={emoji}
              emoji={emoji}
              label={label}
              autoFocus={at === 0}
              onPress={() => onReact(emoji)}
            />
          ))}
        </FocusRegion>
      </Drawer.Panel>
    </Drawer.Root>
  );
}
