import { describe, expect, it } from 'vitest';
import { availableSections } from './LiveSections';

describe('availableSections', () => {
  it('offers only the channels with the live module alone', () => {
    expect(availableSections({ party: false, sport: false, hasMatch: false })).toEqual([
      'channels',
    ]);
  });

  it('adds chat and the ideas box with the party', () => {
    expect(availableSections({ party: true, sport: false, hasMatch: false })).toEqual([
      'chat',
      'channels',
      'ideas',
    ]);
  });

  it('adds the match only while there is one, and the sport list with the sport module', () => {
    expect(availableSections({ party: false, sport: true, hasMatch: false })).toEqual([
      'channels',
      'sport',
    ]);
    expect(availableSections({ party: true, sport: true, hasMatch: true })).toEqual([
      'chat',
      'channels',
      'match',
      'sport',
      'ideas',
    ]);
  });
});
