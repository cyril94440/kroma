const KEY = 'kroma.party.voice.device';
const ID_BYTES = 12;

function randomId(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(ID_BYTES));
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
}

/** An id for this tab, kept for its lifetime across reloads (session storage is
 *  per tab), so two tabs of one account are two participants and a reloaded tab
 *  replaces its own stale entry instead of lingering beside it. */
export function deviceId(): string {
  try {
    const kept = sessionStorage.getItem(KEY);
    if (kept) return kept;
    const created = randomId();
    sessionStorage.setItem(KEY, created);
    return created;
  } catch {
    return randomId();
  }
}

export function newSessionId(): string {
  return randomId().slice(0, 12);
}
