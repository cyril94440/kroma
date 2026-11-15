import { Box } from '#ui/components/atoms/box';
import { Button } from '#ui/components/atoms/button';
import { scaler } from '#ui/components/organisms/player/lib/metrics';
import { useT } from '#ui/services/i18n';

export interface LiveBarProps {
  /** The picture is behind the live edge: the bar then offers the way back. */
  behind: boolean;
  /** The chrome's scale (see ../lib/metrics). 1 on a television stage. */
  scale?: number;
  /** OK on the bar. At the live edge it does what the play control does. */
  onGoLive: () => void;
  onFocus?: () => void;
  onBlur?: () => void;
}

/**
 * What a live stream shows where a film has its seek bar: that the picture is
 * live, or the one control that takes it back there. It is a stop on the
 * navigator like the seek bar it replaces, so Up from the control row lands on
 * it.
 */
export function LiveBar({ behind, scale = 1, onGoLive, onFocus, onBlur }: Readonly<LiveBarProps>) {
  const t = useT();
  const px = scaler(scale);
  return (
    <Box row align="center" pb={px(12)}>
      <Button
        variant={behind ? 'primary' : 'scrim'}
        icon={behind ? 'player-track-next' : 'broadcast'}
        label={behind ? t('player.backToLive') : t('player.live')}
        onPress={onGoLive}
        onFocus={onFocus}
        onBlur={onBlur}
      />
    </Box>
  );
}
