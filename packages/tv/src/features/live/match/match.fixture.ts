import { MatchPayload } from '@kromatv/client/sport';

export const KICKOFF = 1_800_000_000_000;
export const GOAL_AT = KICKOFF + 603_000;

export function liveMatch(): MatchPayload {
  const player = (id: string, num: number, nom: string, equipe: 'dom' | 'ext', poste: string) => ({
    id,
    equipe,
    num,
    nom,
    poste,
    place: num,
    titulaire: true,
  });
  return MatchPayload.parse({
    id: 'm1',
    competition: 'Ligue 1',
    chaine: 'Ligue 1+ 1',
    statut: 'in',
    equipes: {
      dom: { nom: 'Rennes', abbr: 'REN', couleur: '#c00000', formation: '4-3-3' },
      ext: { nom: 'Marseille', abbr: 'OM', couleur: '#0099ff', formation: '4-3-3' },
    },
    joueurs: [
      player('d1', 1, 'Mandanda', 'dom', 'G'),
      player('d9', 9, 'Kebbal', 'dom', 'A'),
      player('e1', 1, 'Rulli', 'ext', 'G'),
    ],
    periodes: [{ periode: 1, s0: 0, t_image: KICKOFF }],
    evenements: [
      {
        id: 'g1',
        type: 'but',
        equipe: 'dom',
        s: 600,
        periode: 1,
        minute: "10'",
        joueur: 'd9',
        t_image: GOAL_AT,
        t_vu: GOAL_AT + 20_000,
      },
    ],
    delai_s: 60,
  });
}
