import { describe, expect, it } from 'vitest';
import { renderMessage } from './render-message';
import type { ChatMessage } from './schemas';

const base: ChatMessage = {
  id: '1',
  userId: null,
  name: '',
  kind: 'system',
  text: 'English text',
  key: null,
  params: {},
  action: null,
  card: null,
  replyTo: null,
  at: 1,
  reactions: {},
};

const catalog: Record<string, string> = {
  'party.vote.passed': '{channel} : {yes} pour',
  'party.sync.fineDone': 'Fin : {results}',
  'party.sync.fineResult': '{name} {ms} ms',
  'party.sync.fineAbandoned': '{name} abandon',
};

const t = (key: string, vars?: Record<string, string | number>) =>
  key in catalog
    ? (catalog[key] ?? key).replace(/\{(\w+)\}/g, (_, k: string) => String(vars?.[k] ?? ''))
    : key;

describe('renderMessage', () => {
  it("shows a person's text as written", () => {
    expect(renderMessage({ ...base, kind: 'user', text: 'hello' }, t)).toBe('hello');
  });

  it("resolves a system key in the reader's language", () => {
    const m = { ...base, key: 'party.vote.passed', params: { channel: 'TF1', yes: '2' } };
    expect(renderMessage(m, t)).toBe('TF1 : 2 pour');
  });

  it('falls back to the stored English text for a key the catalog lacks', () => {
    expect(renderMessage({ ...base, key: 'party.future.key' }, t)).toBe('English text');
  });

  it('builds the fine sync summary from its items, including an abandoned one', () => {
    const m = {
      ...base,
      key: 'party.sync.fineDone',
      params: {
        items: [
          { name: 'Mamie', ms: 40, seeks: 1 },
          { name: 'Papi', abandoned: true },
        ],
      },
    };
    expect(renderMessage(m, t)).toBe('Fin : Mamie 40 ms · Papi abandon');
  });
});
