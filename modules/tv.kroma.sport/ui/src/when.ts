const HOUR_MS = 3_600_000;

export function startsAt(ms: number, now: number, locale?: string): string {
  const day = (t: number) => new Date(t).setHours(0, 0, 0, 0);
  const time = new Date(ms).toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' });
  const days = Math.round((day(ms) - day(now)) / (24 * HOUR_MS));
  if (days <= 0) return time;
  if (days === 1) return `+1 ${time}`;
  return `${new Date(ms).toLocaleDateString(locale, { weekday: 'short' })} ${time}`;
}
