import { useT, useModuleFileUrl } from '@kromatv/module-sdk';
import { Box, Button, Row, Text } from '@kromatv/ui/kit';
import { useState } from 'react';
import { clipUrl, clipVisible } from './clip-url';
import type { Clip } from './schemas';

interface ClipsPanelProps {
  clips: Clip[];
  pdt: number | null;
  now: number;
  matchId?: string | null;
}

export function ClipsPanel({ clips, pdt, now, matchId = null }: Readonly<ClipsPanelProps>) {
  const t = useT();
  const fileUrl = useModuleFileUrl();
  const [playing, setPlaying] = useState<Clip | null>(null);
  const visible = clips.filter(
    (c) => (matchId === null || c.match_id === matchId) && clipVisible(c, pdt, now),
  );
  if (visible.length === 0)
    return (
      <Text variant="body" color="textDim">
        {t('sport.clips.none')}
      </Text>
    );
  return (
    <Box data-testid="sport-clips">
      {playing ? (
        <Box mb={12}>
          <video
            src={clipUrl(fileUrl, playing.file)}
            controls
            autoPlay
            playsInline
            style={{ width: '100%', borderRadius: 12, background: '#000' }}
          >
            <track kind="captions" />
          </video>
          <Button
            label={t('sport.clips.close')}
            size="sm"
            variant="glass"
            onPress={() => setPlaying(null)}
          />
        </Box>
      ) : null}
      {visible.map((clip) => (
        <Row key={clip.id} between mb={6}>
          <Box>
            <Text variant="body">{`${clip.minute} ${clip.buteur}`.trim()}</Text>
            <Text variant="meta" color="textDim">{`${clip.dom} ${clip.score} ${clip.ext}`}</Text>
          </Box>
          <Button
            label={t('sport.clips.play')}
            size="sm"
            variant="glass"
            onPress={() => setPlaying(clip)}
          />
        </Row>
      ))}
    </Box>
  );
}
