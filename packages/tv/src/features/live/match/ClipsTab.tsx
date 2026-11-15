import type { Clip } from '@kromatv/client/sport';
import { useT } from '@kromatv/ui';
import { FocusColumn, ListRow, styles, Text, useStyles } from '@kromatv/ui/kit';

const sDefinition = styles({ column: { gap: 12 } });

export function ClipsTab({
  clips,
  onPlay,
}: Readonly<{ clips: readonly Clip[]; onPlay: (clip: Clip) => void }>) {
  const s = useStyles(sDefinition);
  const t = useT();
  if (clips.length === 0) {
    return (
      <Text variant="bodyTv" color="textDim">
        {t('live.clips.none')}
      </Text>
    );
  }
  return (
    <FocusColumn style={s.column}>
      {clips.map((clip, at) => (
        <ListRow.Root
          key={clip.id}
          icon="player-play"
          size="tv"
          ground="surface"
          chevron={false}
          autoFocus={at === 0}
          onPress={() => onPlay(clip)}
        >
          <ListRow.Label>{`${clip.minute} ${clip.buteur}`.trim()}</ListRow.Label>
          <ListRow.Hint>{`${clip.dom} ${clip.score} ${clip.ext}`}</ListRow.Hint>
        </ListRow.Root>
      ))}
    </FocusColumn>
  );
}
