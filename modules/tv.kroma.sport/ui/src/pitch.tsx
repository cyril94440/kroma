import { Box, Text } from '@kromatv/ui/kit';
import type { OnPitch } from './match-calc';
import type { MatchPayload } from './schemas';
import { readable } from './team-color';

const GROUND = '#2e261c';
const KEEPER_TOP = 94;
const ATTACK_TOP = 57.5;

function top(side: 'dom' | 'ext', player: OnPitch): number {
  const rows = player.lignes ?? 5;
  const row = player.ligne ?? Math.min(rows - 1, Math.max(0, Math.ceil(player.place / 3)));
  const depth = KEEPER_TOP - (row * (KEEPER_TOP - ATTACK_TOP)) / Math.max(1, rows - 1);
  return side === 'dom' ? depth : 100 - depth;
}

interface PitchProps {
  teams: MatchPayload['equipes'];
  onPitch: { dom: OnPitch[]; ext: OnPitch[] };
}

export function Pitch({ teams, onPitch }: Readonly<PitchProps>) {
  return (
    <Box
      w="100%"
      aspect={0.72}
      bg="success/9"
      radius={12}
      style={{ position: 'relative' }}
      data-testid="sport-pitch"
    >
      {(['dom', 'ext'] as const).flatMap((side) =>
        onPitch[side].map((player) => {
          const x = player.x ?? 50;
          return (
            <Box
              key={player.id}
              center
              style={{
                position: 'absolute',
                left: `${3 + (side === 'ext' ? 100 - x : x) * 0.94}%`,
                top: `${top(side, player)}%`,
                transform: [{ translateX: -14 }, { translateY: -14 }],
              }}
            >
              <Box
                w={28}
                h={28}
                radius={14}
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
