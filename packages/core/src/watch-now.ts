import { hasPermission, type User } from '@kromatv/client/accounts';

/** Whether this viewer may skip the queue: someone whose request would have been
 * fetched without anyone's approval. The server decides too; this keeps the
 * button off a screen the server would answer 404. */
export function mayWatchNow(user: Pick<User, 'permissions'> | null | undefined): boolean {
  if (!user || !hasPermission(user, 'requests.create')) return false;
  return hasPermission(user, 'requests.auto') || hasPermission(user, 'requests.manage');
}

/** Whether to offer "Watch now" on a title: the server says it can fetch at
 * once, and the viewer may use it. */
export function offersWatchNow(
  user: Pick<User, 'permissions'> | null | undefined,
  serverOffers: boolean,
): boolean {
  return serverOffers === true && mayWatchNow(user);
}
