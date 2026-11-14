import type { useT } from '@kromatv/module-sdk';
import type { ChatMessage } from './schemas';

type Translate = ReturnType<typeof useT>;
type Vars = NonNullable<Parameters<Translate>[1]>;

interface FineItem {
  name: string;
  ms?: number;
  seeks?: number;
  abandoned?: boolean;
}

function fineItems(params: ChatMessage['params']): FineItem[] {
  const items = params.items;
  return Array.isArray(items) ? (items as FineItem[]) : [];
}

/** The line a message shows: the server's own text for a person or the
 *  assistant, and for a system line its key resolved in the reader's language,
 *  falling back to the English text the server stored when the catalog lacks it. */
export function renderMessage(message: ChatMessage, t: Translate): string {
  if (!message.key) return message.text;
  const vars: Vars = {};
  for (const [k, v] of Object.entries(message.params)) {
    if (typeof v === 'string' || typeof v === 'number') vars[k] = v;
  }
  if (message.key === 'party.sync.fineDone') {
    vars.results = fineItems(message.params)
      .map((item) =>
        item.abandoned
          ? t('party.sync.fineAbandoned', { name: item.name })
          : t('party.sync.fineResult', {
              name: item.name,
              ms: item.ms ?? 0,
              seeks: item.seeks ?? 0,
            }),
      )
      .join(' · ');
  }
  const out = t(message.key, vars);
  return out === message.key ? message.text : out;
}
