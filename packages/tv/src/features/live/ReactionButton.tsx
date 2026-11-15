import { useT } from '@kromatv/ui';
import { Focusable, Text } from '@kromatv/ui/kit';
import type { MessageKey } from '@kromatv/i18n';

const SIZE = 96;

/** A reaction as everyone sees it floating over the picture: the glyph is the
 *  wire value and is drawn as content, the control is named for what it says. */
export function ReactionButton({
  emoji,
  label,
  autoFocus,
  onPress,
}: Readonly<{
  emoji: string;
  label: MessageKey;
  autoFocus?: boolean;
  onPress: () => void;
}>) {
  const t = useT();
  return (
    <Focusable
      label={t(label)}
      autoFocus={autoFocus}
      onPress={onPress}
      focusScale={1.08}
      style={{ width: SIZE, height: SIZE, borderRadius: SIZE / 2, alignItems: 'center', justifyContent: 'center' }}
    >
      <Text variant="headingTv">
        {emoji}
      </Text>
    </Focusable>
  );
}
