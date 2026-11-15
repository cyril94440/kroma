import type { Clip } from '@kromatv/client/sport';
import { Box, Button, Text } from '@kromatv/ui/kit';
import { useT } from '#mobile/lib/i18n';

export function MatchClips({
  clips,
  onPlay,
}: Readonly<{ clips: readonly Clip[]; onPlay: (clip: Clip) => void }>) {
  const t = useT();
  if (clips.length === 0) {
    return (
      <Text variant="body" color="textDim">
        {t('live.clips.none')}
      </Text>
    );
  }
  return (
    <Box gap={8}>
      {clips.map((clip) => (
        <Box key={clip.id} row justify="space-between" align="center" gap={10}>
          <Box flex>
            <Text variant="body">{`${clip.minute} ${clip.buteur}`.trim()}</Text>
            <Text variant="meta" color="textDim">{`${clip.dom} ${clip.score} ${clip.ext}`}</Text>
          </Box>
          <Button
            icon="player-play"
            label={t('live.clips.play')}
            size="sm"
            variant="glass"
            onPress={() => onPlay(clip)}
          />
        </Box>
      ))}
    </Box>
  );
}
