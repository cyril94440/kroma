import type { MessageKey, TVars } from '@kromatv/i18n';
import { z } from 'zod';

export const SOURCE_FAILURES = [
  'error.sourceDisabled',
  'error.sourceGone',
  'error.sourcePreparing',
  'error.sourceRefused',
  'error.sourceUnreachable',
] as const;

export interface SourceFailure {
  messageKey: MessageKey;
  messageVars?: TVars;
}

const SOURCE_STATUSES = new Set([502, 503]);

const ERROR_CODE = /^error\.\w+$/;

const SourceErrorBody = z.object({
  code: z.string().regex(ERROR_CODE),
  error: z.string().trim().min(1).optional().catch(undefined),
});

function isKnownFailure(code: string): code is (typeof SOURCE_FAILURES)[number] {
  return SOURCE_FAILURES.some((known) => known === code);
}

/**
 * Why the server could not read a title held by a module: the `error.*` code of
 * its 502/503 answer, or null for any other answer. A code this client knows is
 * a key of the `error` catalog and reads in the viewer's language; one it does
 * not is shown as the server's own translated message, else as a generic line,
 * so a code added server-side surfaces without a client release.
 */
export async function readSourceFailure(answer: Response): Promise<SourceFailure | null> {
  if (!SOURCE_STATUSES.has(answer.status)) return null;
  try {
    const body = SourceErrorBody.safeParse(await answer.json());
    if (!body.success) return null;
    const { code, error } = body.data;
    if (isKnownFailure(code)) return { messageKey: code };
    if (error) return { messageKey: 'error.sourceMessage', messageVars: { message: error } };
    return { messageKey: 'error.sourceUnknown' };
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
