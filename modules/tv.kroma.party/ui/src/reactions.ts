export const REACTIONS = ['⚽', '\u{1F525}', '\u{1F602}', '\u{1F62E}', '❤️', '\u{1F44F}'];

export function reactionCount(
  reactions: Record<string, { userId: string }[]>,
  emoji: string,
): number {
  return reactions[emoji]?.length ?? 0;
}

export function reactedBy(
  reactions: Record<string, { userId: string }[]>,
  emoji: string,
  userId: string,
): boolean {
  return (reactions[emoji] ?? []).some((p) => p.userId === userId);
}
