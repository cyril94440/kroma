import { useLocale, useT } from '@kromatv/ui';

export type ConfigLocales = Readonly<Record<string, Readonly<Record<string, string>>>>;

/** Words a config field's label, option or placeholder: the module's own
 *  catalog in the reader's language, then its English one, then the app's. A
 *  string no catalog knows renders as written, so a module that wrote its labels
 *  out keeps working. */
export function useConfigText(locales: ConfigLocales | undefined): (text: string) => string {
  const locale = useLocale();
  const app = useT();
  return (text) =>
    locales?.[locale]?.[text] ?? locales?.en?.[text] ?? app(text as Parameters<typeof app>[0]);
}
