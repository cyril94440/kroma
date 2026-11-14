export const SOURCE_FAILURES = [
  'error.sourceDisabled',
  'error.sourceGone',
  'error.sourceRefused',
  'error.sourceUnreachable',
] as const;

export type SourceFailure = (typeof SOURCE_FAILURES)[number];

const SOURCE_STATUSES = new Set([502, 503]);

export function isSourceFailure(code: unknown): code is SourceFailure {
  return SOURCE_FAILURES.some((known) => known === code);
}

/**
 * Why the server could not read a title held by a module: the `code` of its
 * 502/503 answer, or null for any other answer. The code is a key of the
 * `error` catalog, so the viewer reads it in their own language.
 */
export async function readSourceFailure(answer: Response): Promise<SourceFailure | null> {
  if (!SOURCE_STATUSES.has(answer.status)) return null;
  try {
    const body: unknown = await answer.json();
    const code =
      typeof body === 'object' && body !== null ? (body as { code?: unknown }).code : null;
    return isSourceFailure(code) ? code : null;
  } catch {
    return null;
  }
}

/** Asks the server for the first byte of a title and reports why it cannot be read, if it cannot. */
export async function probeSourceFailure(streamUrl: string): Promise<SourceFailure | null> {
  try {
    return await readSourceFailure(await fetch(streamUrl, { headers: { Range: 'bytes=0-0' } }));
  } catch {
    return null;
  }
}
