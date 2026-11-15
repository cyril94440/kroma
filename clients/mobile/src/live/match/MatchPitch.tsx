import type { MatchPayload } from '@kromatv/client/sport';
import { type MatchView, pitchSlot, readable } from '@kromatv/core';
import { Box, Text } from '@kromatv/ui/kit';
import { useT } from '#mobile/lib/i18n';

const GROUND = '#2e261c';
const DOT = 28;

export function MatchPitch({
  teams,
  onPitch,
}: Readonly<{ teams: MatchPayload['equipes']; onPitch: MatchView['onPitch'] }>) {
  const t = useT();
  if (onPitch.dom.length + onPitch.ext.length === 0) {
    return (
      <Text variant="body" color="textDim">
        {t('live.lineupsNone')}
      </Text>
    );
  }
  return (
    <Box w="100%" aspect={0.72} bg="success/9" radius="lg" style={{ position: 'relative' }}>
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
                <Text variant="meta" color="accentInk">
                  {player.num}
                </Text>
              </Box>
              <Text variant="meta">{player.nom}</Text>
            </Box>
          );
        }),
      )}
    </Box>
  );
}
