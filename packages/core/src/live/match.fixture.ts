import type { Fact, MatchPayload, Player } from '@kromatv/client/sport';

export const T0 = 1_800_000_000_000;

const HOME: [string, number, string, string][] = [
  ['d1', 1, 'Trapp', 'G'],
  ['d2', 2, 'Traore', 'D'],
  ['d3', 3, 'Otavio', 'D'],
  ['d4', 4, 'Lees-Melou', 'M'],
  ['d5', 5, 'Chergui', 'D'],
  ['d6', 6, 'Coppola', 'D'],
  ['d7', 7, 'Kebbal', 'M'],
  ['d8', 8, 'Lopez', 'M'],
  ['d9', 9, 'Sinayoko', 'A'],
  ['d10', 10, 'Pagis', 'M'],
  ['d11', 11, 'Simon', 'A'],
];
const AWAY: [string, number, string, string][] = [
  ['m1', 1, 'Hradecky', 'G'],
  ['m2', 2, 'Vanderson', 'D'],
  ['m3', 3, 'Nazinho', 'D'],
  ['m4', 4, 'Zakaria', 'M'],
  ['m5', 5, 'Sane', 'D'],
  ['m6', 6, 'Dier', 'D'],
  ['m7', 7, 'Diop', 'M'],
  ['m8', 8, 'Camara', 'M'],
  ['m9', 9, 'Brunner', 'A'],
  ['m10', 10, 'Golovin', 'M'],
  ['m11', 11, 'Abline', 'A'],
];

function lineup(side: Player['equipe'], rows: [string, number, string, string][]): Player[] {
  return rows.map(([id, num, nom, poste], i) => ({
    id,
    equipe: side,
    num,
    nom,
    poste,
    place: i + 1,
    titulaire: true,
  }));
}

function players(): Player[] {
  return [
    ...lineup('dom', HOME),
    ...lineup('ext', AWAY),
    { id: 'd18', equipe: 'dom', num: 18, nom: 'Sissoko', poste: 'A', place: 0, titulaire: false },
  ];
}

export function fact(
  id: string,
  type: Fact['type'],
  equipe: Fact['equipe'],
  s: number,
  extra: Partial<Fact> = {},
): Fact {
  return {
    id,
    type,
    equipe,
    s,
    periode: 1,
    minute: `${Math.floor(s / 60) + 1}'`,
    joueur: null,
    passeur: null,
    entre: null,
    sort: null,
    x: 90,
    y: 50,
    t_image: T0 + s * 1000 + 3000,
    t_vu: T0 + 7_200_000,
    ...extra,
  };
}

export function baseMatch(extra: Partial<MatchPayload> = {}): MatchPayload {
  return {
    id: 'espn-test',
    competition: 'Ligue 1',
    chaine: 'FR: LIGUE1+ 5',
    statut: 'in',
    equipes: {
      dom: { nom: 'Paris FC', abbr: 'PFC', couleur: '#000066', formation: '4-2-3-1' },
      ext: { nom: 'AS Monaco', abbr: 'MON', couleur: '#d71431', formation: '4-2-3-1' },
    },
    joueurs: players(),
    periodes: [
      { periode: 1, s0: 0, t_image: T0 },
      { periode: 2, s0: 2700, t_image: null },
    ],
    evenements: [
      fact('e1', 'tir_cadre', 'dom', 600, { joueur: 'd9' }),
      fact('e2', 'but', 'dom', 2250, { joueur: 'd9', passeur: 'd8' }),
      fact('e3', 'corner', 'ext', 2300),
      fact('e4', 'jaune', 'ext', 2400, { joueur: 'm2' }),
      fact('e5', 'but', 'ext', 3897, { joueur: 'm9' }),
    ],
    possession: [{ t_image: T0 + 1_000_000, t_vu: T0 + 7_200_000, dom: 44.4, ext: 55.6 }],
    delai_s: 60,
    maj: T0 + 3_000_000,
    ...extra,
  };
}
