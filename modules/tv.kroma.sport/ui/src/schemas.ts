import { z } from 'zod';

const Side = z.enum(['dom', 'ext']);

export const Line = z.object({ nom: z.string(), score: z.string().catch('') });

export const ChannelLink = z.object({
  nom: z.string(),
  ref: z.string().nullable().catch(null),
  source: z.enum(['image', 'guide', 'web']).catch('guide'),
  certitude: z.enum(['vu', 'guide', 'a_confirmer']).catch('guide'),
  etrangere: z.boolean().catch(false),
});

export const SportEvent = z.object({
  id: z.string(),
  sport: z.enum(['foot', 'tennis', 'f1', 'golf']).catch('foot'),
  compet: z.string().catch(''),
  statut: z.enum(['pre', 'in', 'post']).catch('pre'),
  debut: z.number(),
  temps: z.string().catch(''),
  lignes: z.array(Line).catch([]),
  chaine: ChannelLink.nullable().catch(null),
  a_lecran: z.boolean().catch(false),
  masquer_score: z.boolean().catch(false),
  ligue1: z.boolean().catch(false),
  pop: z.number().catch(0),
});
export type SportEvent = z.infer<typeof SportEvent>;

export const MultiplexChannel = z.object({
  num: z.number().nullable().catch(null),
  nom: z.string().nullable().catch(null),
  ref: z.string(),
  etat: z.string().catch(''),
  event: z.string().nullable().catch(null),
  certitude: z.string().nullable().catch(null),
  equipes: z.array(z.string()).catch([]),
  score: z.string().catch(''),
  temps: z.string().catch(''),
  a_lecran: z.boolean().catch(false),
});

export const ScanStatus = z.object({
  etat: z.string().catch('inactif'),
  fait: z.number().catch(0),
  total: z.number().catch(0),
  captures: z.number().catch(0),
  fin_ts: z.number().nullable().catch(null),
  raison: z.string().nullable().catch(null),
});

export const SportView = z.object({
  maj: z.number().catch(0),
  direct: z.number().catch(0),
  prochain: z.number().nullable().catch(null),
  evenements: z.array(SportEvent).catch([]),
  ligue1: z
    .object({
      direct: z.number().catch(0),
      prochain: z.number().nullable().catch(null),
      canaux: z.array(MultiplexChannel).catch([]),
    })
    .catch({ direct: 0, prochain: null, canaux: [] }),
  scan: ScanStatus.catch({
    etat: 'inactif',
    fait: 0,
    total: 0,
    captures: 0,
    fin_ts: null,
    raison: null,
  }),
});
export type SportView = z.infer<typeof SportView>;

export const Player = z.object({
  id: z.string(),
  equipe: Side,
  num: z.number().catch(0),
  nom: z.string().catch(''),
  poste: z.string().catch(''),
  place: z.number().catch(0),
  titulaire: z.boolean().catch(false),
  ligne: z.number().optional(),
  lignes: z.number().optional(),
  x: z.number().optional(),
});
export type Player = z.infer<typeof Player>;

export const Fact = z.object({
  id: z.string(),
  type: z.enum([
    'tir',
    'tir_cadre',
    'tir_contre',
    'but',
    'corner',
    'jaune',
    'rouge',
    'remplacement',
  ]),
  equipe: Side,
  s: z.number().catch(0),
  periode: z.number().catch(1),
  minute: z.string().catch(''),
  joueur: z.string().nullable().catch(null),
  passeur: z.string().nullable().catch(null),
  entre: z.string().nullable().optional().catch(null),
  sort: z.string().nullable().optional().catch(null),
  x: z.number().nullable().catch(null),
  y: z.number().nullable().catch(null),
  t_image: z.number().nullable().catch(null),
  t_vu: z.number().catch(0),
});
export type Fact = z.infer<typeof Fact>;

const TeamInfo = z.object({
  nom: z.string().catch(''),
  abbr: z.string().catch(''),
  couleur: z.string().catch('#6b6256'),
  formation: z.string().catch(''),
});

export const MatchPayload = z.object({
  id: z.string(),
  competition: z.string().catch(''),
  chaine: z.string().catch(''),
  statut: z.enum(['pre', 'in', 'post']).catch('pre'),
  equipes: z.object({ dom: TeamInfo, ext: TeamInfo }),
  joueurs: z.array(Player).catch([]),
  periodes: z
    .array(
      z.object({ periode: z.number(), s0: z.number(), t_image: z.number().nullable().catch(null) }),
    )
    .catch([]),
  evenements: z
    .array(Fact.nullable().catch(null))
    .transform((all) => all.filter((f) => f !== null)),
  possession: z
    .array(z.object({ t_image: z.number(), t_vu: z.number(), dom: z.number(), ext: z.number() }))
    .catch([]),
  delai_s: z.number().catch(60),
  maj: z.number().catch(0),
});
export type MatchPayload = z.infer<typeof MatchPayload>;

export const PronoPayload = z.object({
  id: z.string(),
  competition: z.string().catch(''),
  home: z.string(),
  away: z.string(),
  home_abbr: z.string().catch(''),
  away_abbr: z.string().catch(''),
  home_color: z.string().catch('#6b6256'),
  away_color: z.string().catch('#6b6256'),
  statut: z.enum(['ouvert', 'en_cours', 'final']),
  chaine: z.string().catch(''),
  score: z.string().catch('0-0'),
  minute: z.string().catch(''),
  auto: z.boolean().catch(true),
  ferme_dans_s: z.number().catch(0),
  votes: z.record(z.string(), z.string()).catch({}),
  noms: z.record(z.string(), z.string()).catch({}),
  requis: z.array(z.string()).catch([]),
  distribution: z.array(z.object({ score: z.string(), n: z.number() })).catch([]),
  classement_match: z
    .array(
      z.object({
        user_id: z.string(),
        pseudo: z.string(),
        prono: z.string(),
        distance: z.number(),
        texte: z.string(),
      }),
    )
    .catch([]),
  resultats: z
    .array(
      z.object({
        user_id: z.string(),
        pseudo: z.string(),
        prono: z.string(),
        points: z.number(),
        exact: z.boolean(),
      }),
    )
    .catch([]),
  saison: z
    .array(
      z.object({ user_id: z.string(), pseudo: z.string(), points: z.number(), gagnes: z.number() }),
    )
    .catch([]),
  annule: z.boolean().optional(),
});
export type PronoPayload = z.infer<typeof PronoPayload>;

export const Clip = z.object({
  id: z.string(),
  match_id: z.string().catch(''),
  dom: z.string().catch(''),
  ext: z.string().catch(''),
  competition: z.string().catch(''),
  minute: z.string().catch(''),
  equipe: z.string().catch(''),
  buteur: z.string().catch(''),
  score: z.string().catch(''),
  debut_ms: z.number(),
  fin_ms: z.number(),
  duree_s: z.number().catch(0),
  ts: z.number().catch(0),
  file: z.object({ module: z.string(), path: z.string() }),
});
export type Clip = z.infer<typeof Clip>;

export const SportState = z.object({
  seq: z.number(),
  now: z.number(),
  me: z.object({ userId: z.string(), name: z.string(), admin: z.boolean() }),
  view: SportView,
  match: MatchPayload.nullable().catch(null),
  prono: PronoPayload.nullable().catch(null),
  clips: z.array(Clip).catch([]),
  delay: z
    .object({ seconds: z.number(), source: z.string() })
    .catch({ seconds: 60, source: 'defaut' }),
});
export type SportState = z.infer<typeof SportState>;

export const SportEventMessage = z.discriminatedUnion('type', [
  z.object({ type: z.literal('sport.view'), seq: z.number(), view: SportView }),
  z.object({
    type: z.literal('sport.match'),
    seq: z.number(),
    match: MatchPayload.nullable().catch(null),
  }),
  z.object({
    type: z.literal('sport.prono'),
    seq: z.number(),
    prono: PronoPayload.nullable().catch(null),
  }),
  z.object({ type: z.literal('sport.clips'), seq: z.number(), clips: z.array(Clip).catch([]) }),
]);
export type SportEventMessage = z.infer<typeof SportEventMessage>;

export const Ok = z.object({ ok: z.boolean().catch(true) });
export const Opened = z.object({ ok: z.boolean().catch(true), accepted: z.boolean().catch(false) });
export const Switched = z.object({
  outcome: z.string(),
  voteId: z.string().nullable().catch(null),
  reason: z.string().nullable().catch(null),
});
export const Leaderboard = z.object({
  season: z
    .array(
      z.object({ user_id: z.string(), pseudo: z.string(), points: z.number(), gagnes: z.number() }),
    )
    .catch([]),
});
export const Calibrated = z.object({
  delayS: z.number().catch(60),
  delaySource: z.string().catch('defaut'),
});

const Standing = z.object({
  rang: z.number(),
  club: z.string(),
  pts: z.number(),
  diff: z.number(),
  match: z.string().catch(''),
  outcome: z.string().nullable().catch(null),
});
export const StandingsReply = z.object({
  pending: z.boolean().catch(false),
  standings: z
    .object({ lignes: z.array(Standing).catch([]), matchs_comptes: z.number().catch(0) })
    .nullable()
    .catch(null),
});
export type StandingsReply = z.infer<typeof StandingsReply>;
