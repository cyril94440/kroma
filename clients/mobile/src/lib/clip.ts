import { z } from 'zod';
import { routeParam } from '#mobile/lib/nav';

interface ClipRoute {
  uri: string;
  title: string;
  subtitle: string;
}

const ClipParams = z.object({ uri: z.string().min(1), title: z.string(), subtitle: z.string() });

export function clipHref(clip: ClipRoute): string {
  const { uri, title, subtitle } = clip;
  return `/clip?${new URLSearchParams({ uri, title, subtitle }).toString()}`;
}

/** The clip a player route names, or null when its path cannot name one. */
export function clipRoute(params: {
  uri?: string | string[];
  title?: string | string[];
  subtitle?: string | string[];
}): ClipRoute | null {
  const parsed = ClipParams.safeParse({
    uri: routeParam(params.uri) ?? '',
    title: routeParam(params.title) ?? '',
    subtitle: routeParam(params.subtitle) ?? '',
  });
  return parsed.success ? parsed.data : null;
}
