import { describe, expect, it } from 'vitest';
import { baseMatch } from './match.fixture';
import { SportState, type SportEventMessage } from '@kromatv/client/sport';
import {
  adoptSport as adopt,
  applySportEvent as applyEvent,
  EMPTY_SPORT as EMPTY,
} from './sport-state';

const state = (seq: number) =>
  SportState.parse({
    seq,
    now: 5000,
    me: { userId: 'u', name: 'U', admin: false },
    view: {},
    match: null,
    prono: null,
    clips: [],
  });

const matchEvent = (seq: number): SportEventMessage => ({
  type: 'sport.match',
  seq,
  match: baseMatch(),
});

describe('sport state', () => {
  it('adopts a snapshot with its sequence and the offset of the server clock', () => {
    const model = adopt(state(7), 1000);

    expect([model.seq, model.stale, model.clockOffsetMs]).toEqual([7, false, 4000]);
  });

  it('drops an event the snapshot already holds', () => {
    const model = adopt(state(7), 0);

    const next = applyEvent(model, matchEvent(7), 0);

    expect(next).toBe(model);
  });

  it('applies the next event in order', () => {
    const model = adopt(state(7), 0);

    const next = applyEvent(model, matchEvent(8), 0);

    expect([next.seq, next.stale, next.state?.match?.id]).toEqual([8, false, 'espn-test']);
  });

  it('marks the model stale when an event skips ahead so the caller refetches the state', () => {
    const model = adopt(state(7), 0);

    const next = applyEvent(model, matchEvent(11), 0);

    expect([next.seq, next.stale]).toEqual([11, true]);
  });

  it('converges to the snapshot after a gap', () => {
    const stale = applyEvent(adopt(state(7), 0), matchEvent(11), 0);

    const refetched = adopt(state(12), 0);

    expect([
      stale.stale,
      refetched.seq,
      refetched.stale,
      applyEvent(refetched, matchEvent(12), 0) === refetched,
    ]).toEqual([true, 12, false, true]);
  });

  it('ignores events until a snapshot has arrived', () => {
    expect(applyEvent(EMPTY, matchEvent(1), 0)).toBe(EMPTY);
  });

  it('remembers when the finished match was first seen and forgets it for another match', () => {
    const finished = (id: string, seq: number): SportEventMessage => ({
      type: 'sport.match',
      seq,
      match: baseMatch({ id, statut: 'post' }),
    });
    const first = applyEvent(adopt(state(7), 1000), finished('a', 8), 2000);

    const again = applyEvent(first, finished('a', 9), 9000);
    const other = applyEvent(again, finished('b', 10), 9000);

    expect([first.postSeen, again.postSeen?.at, other.postSeen]).toEqual([
      { id: 'a', at: 6000 },
      6000,
      { id: 'b', at: 13000 },
    ]);
  });
});
