import type { MatchPayload } from '@kromatv/client/sport';
import { type MatchView, pitchSlot, readable } from '@kromatv/core';
import { useT } from '@kromatv/ui';
import { Box, Text } from '@kromatv/ui/kit';

const GROUND = '#2e261c';
const DOT = 36;

export function PitchTab({
  teams,
  onPitch,
}: Readonly<{ teams: MatchPayload['equipes']; onPitch: MatchView['onPitch'] }>) {
  const t = useT();
  if (onPitch.dom.length + onPitch.ext.length === 0) {
    return (
      <Text variant="bodyTv" color="textDim">
        {t('live.lineupsNone')}
      </Text>
    );
  }
  return (
    <Box w="100%" aspect={0.8} bg="success/9" radius="lg" style={{ position: 'relative' }}>
      {(['dom', 'ext'] as const).flatMap((side) =>
        onPitch[side].map((player) => {
          const slot = pitchSlot(side, player);
          return (
            <Box
              key={player.id}
              center
              style={{
                position: 'absolute',
                left: `${slot.leftPercent}%`,
                top: `${slot.topPercent}%`,
                transform: [{ translateX: -DOT / 2 }, { translateY: -DOT / 2 }],
              }}
            >
              <Box
                w={DOT}
                h={DOT}
                radius={DOT / 2}
                center
                style={{ backgroundColor: readable(teams[side].couleur, GROUND) }}
              >
                <Text variant="labelTv" color="accentInk">
                  {player.num}
                </Text>
              </Box>
              <Text variant="labelTv">{player.nom}</Text>
            </Box>
          );
        }),
      )}
    </Box>
  );
}
