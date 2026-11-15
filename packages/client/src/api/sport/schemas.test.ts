import { describe, expect, it } from 'vitest';
import { Clip, MatchPayload, SportEventMessage, SportState, SportView } from './schemas';

const VIEW = {
  maj: 1,
  direct: 1,
  prochain: null,
  evenements: [
    {
      id: 'e1',
      sport: 'foot',
      compet: 'Ligue 1',
      statut: 'in',
      debut: 1_800_000_000_000,
      temps: "34'",
      lignes: [
        { nom: 'Lens', score: '1' },
        { nom: 'Lille', score: '0' },
      ],
      chaine: { nom: 'LIGUE1+', ref: 'p:7', source: 'guide', certitude: 'vu', etrangere: false },
      a_lecran: true,
      masquer_score: false,
      ligue1: true,
      pop: 3,
    },
  ],
};

const MATCH = {
  id: 'm1',
  competition: 'Ligue 1',
  chaine: 'LIGUE1+',
  statut: 'in',
  equipes: {
    dom: { nom: 'Lens', abbr: 'RCL', couleur: '#f00', formation: '4-3-3' },
    ext: { nom: 'Lille', abbr: 'LIL', couleur: '#00f', formation: '4-4-2' },
  },
  joueurs: [],
  periodes: [{ periode: 1, s0: 0, t_image: null }],
  evenements: [{ id: 'f1', type: 'but', equipe: 'dom', s: 600 }, null],
  possession: [],
  delai_s: 60,
  maj: 2,
};

describe('wire shapes the sport module pins', () => {
  it('reads the view with its channel link', () => {
    const view = SportView.parse(VIEW);

    expect(view.evenements[0]?.chaine?.ref).toBe('p:7');
    expect(view.ligue1.canaux).toEqual([]);
  });

  it('drops a fact it cannot read and keeps the others', () => {
    const match = MatchPayload.parse(MATCH);

    expect(match.evenements).toHaveLength(1);
    expect(match.evenements[0]?.type).toBe('but');
  });

  it('reads the state a joining client fetches', () => {
    const state = SportState.parse({
      seq: 3,
      now: 5,
      me: { userId: 'u1', name: 'Ana', admin: false },
      view: VIEW,
      match: MATCH,
      prono: null,
      clips: [],
    });

    expect(state.delay).toEqual({ seconds: 60, source: 'defaut' });
    expect(state.match?.equipes.dom.nom).toBe('Lens');
  });

  it('reads a clip with the module that holds its file', () => {
    const clip = Clip.parse({
      id: 'c1',
      debut_ms: 1,
      fin_ms: 2,
      file: { module: 'tv.kroma.live', path: 'run/clips/a.mp4' },
    });

    expect(clip.file.module).toBe('tv.kroma.live');
  });
});

describe('the sport events', () => {
  it('reads every topic and refuses an unknown one', () => {
    expect(SportEventMessage.safeParse({ type: 'sport.view', seq: 1, view: VIEW }).success).toBe(
      true,
    );
    expect(SportEventMessage.safeParse({ type: 'sport.match', seq: 2, match: null }).success).toBe(
      true,
    );
    expect(SportEventMessage.safeParse({ type: 'sport.clips', seq: 3, clips: [] }).success).toBe(
      true,
    );
    expect(SportEventMessage.safeParse({ type: 'sport.nope', seq: 4 }).success).toBe(false);
  });
});
