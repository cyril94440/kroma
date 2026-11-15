import type { LiveNotice } from '@kromatv/core';
import { Box, Spinner, styles, Text, useStyles } from '@kromatv/ui/kit';
import { VideoView, type VideoPlayer } from 'expo-video';
import { useT } from '#mobile/lib/i18n';

const sDefinition = styles({ video: { flex: 1 } });

export function LiveVideo({
  player,
  label,
  notice,
  waiting,
}: Readonly<{ player: VideoPlayer; label: string; notice: LiveNotice | null; waiting: boolean }>) {
  const s = useStyles(sDefinition);
  const t = useT();
  return (
    <Box aspect={16 / 9} bg="black">
      <VideoView
        player={player}
        style={s.video}
        contentFit="contain"
        nativeControls={false}
        accessibilityLabel={label}
      />
      {notice || waiting ? (
        <Box absolute top={0} right={0} bottom={0} left={0} center gap={12} pointerEvents="none">
          {notice && !notice.spinner ? null : <Spinner size={36} />}
          {notice ? (
            <Text variant="label" color="white" textAlign="center">
              {t(notice.title, { name: notice.channelName })}
            </Text>
          ) : null}
        </Box>
      ) : null}
    </Box>
  );
}
