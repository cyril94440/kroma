import type { ModuleApi } from '@kromatv/module-sdk';
import {
  AssistantStatus,
  AuditPage,
  Done,
  Lessons,
  ModelList,
  ModelTest,
  Notes,
  Rules,
  Suggestions,
} from './schemas';

export function auditPath(genre: string, before?: number): string {
  const params = new URLSearchParams({ limit: '100' });
  if (genre) params.set('genre', genre);
  if (before !== undefined) params.set('before', String(before));
  return `/audit?${params.toString()}`;
}

export function createHermesApi(api: ModuleApi) {
  return {
    status: () => api.get('/status', AssistantStatus),
    pause: (paused: boolean) => api.post('/pause', { paused }, Done),
    audit: (genre: string, before?: number) => api.get(auditPath(genre, before), AuditPage),
    rules: () => api.get('/rules', Rules),
    saveRules: (text: string) => api.put('/rules', { text }, Done),
    resetRules: () => api.post('/rules/reset', {}, Done),
    lessons: () => api.get('/lessons', Lessons),
    removeLesson: (id: number) => api.delete(`/lessons/${id}`, Done),
    suggestions: () => api.get('/suggestions', Suggestions),
    removeSuggestion: (id: number) => api.delete(`/suggestions/${id}`, Done),
    notes: () => api.get('/notes', Notes),
    forget: (userId: string) => api.delete(`/notes/${encodeURIComponent(userId)}`, Done),
    testModel: () => api.post('/model/test', {}, ModelTest),
    listModels: () => api.get('/model/list', ModelList),
  };
}

export type HermesApi = ReturnType<typeof createHermesApi>;
