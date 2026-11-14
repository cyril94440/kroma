import { describe, expect, it } from 'vitest';
import { auditPath } from './hermes-api';
import { AssistantStatus, AuditPage, ModelList, ModelTest, Notes } from './schemas';

describe('the status the server sends', () => {
  it('fills what a newer or older server leaves out', () => {
    const status = AssistantStatus.parse({ enabled: true, provider: 'gemini', somethingNew: 1 });
    expect(status.paused).toBe(false);
    expect(status.callsLastHour).toBe(0);
    expect(status.model).toBe('');
  });
});

describe('the audit page', () => {
  it('keeps each record with the data the code decided on', () => {
    const page = AuditPage.parse({
      rows: [{ id: 3, at: 10, genre: 'chat', event: 'gate', data: { verdict: 'Engage' } }],
      genres: ['chat'],
    });
    expect(page.rows[0]?.data).toEqual({ verdict: 'Engage' });
    expect(AuditPage.parse({}).rows).toEqual([]);
  });

  it('asks for a genre and the page before a record', () => {
    expect(auditPath('', undefined)).toBe('/audit?limit=100');
    expect(auditPath('llm', 42)).toBe('/audit?limit=100&genre=llm&before=42');
  });
});

describe('the model checks', () => {
  it('reads a success and a failure', () => {
    expect(ModelTest.parse({ ok: true, answer: 'ok', ms: 120 }).answer).toBe('ok');
    expect(ModelTest.parse({ ok: false, error: 'HTTP 401' }).error).toBe('HTTP 401');
    expect(ModelList.parse({ ok: true, models: ['a', 'b'] }).models).toHaveLength(2);
    expect(ModelList.parse({ ok: false }).models).toEqual([]);
  });

  it('reads the notes per person', () => {
    expect(
      Notes.parse({ notes: [{ userId: 'u1', name: 'Ana', note: 'Aime le rugby' }] }).notes[0]?.name,
    ).toBe('Ana');
  });
});
