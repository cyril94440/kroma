export function play(element: HTMLVideoElement): Promise<void> {
  return element.play();
}

export function pause(element: HTMLVideoElement): void {
  element.pause();
}

export function isPaused(element: HTMLVideoElement): boolean {
  return element.paused;
}

export function seek(element: HTMLVideoElement, position: number): void {
  element.currentTime = position;
}

export function attachNative(element: HTMLVideoElement, url: string): () => void {
  element.src = url;
  return () => element.removeAttribute('src');
}

export function supportsNativeHls(element: HTMLVideoElement): boolean {
  return element.canPlayType('application/vnd.apple.mpegurl') !== '';
}

export function isStalled(element: HTMLVideoElement): boolean {
  return element.readyState < 3 && !element.paused;
}

export function position(element: HTMLVideoElement): number {
  return element.currentTime;
}

interface WithStartDate {
  getStartDate?: () => Date;
}

export function programmeTime(element: HTMLVideoElement, playingDate: Date | null): number | null {
  if (playingDate) return playingDate.getTime();
  const start = (element as WithStartDate).getStartDate?.().getTime();
  return start === undefined || Number.isNaN(start) ? null : start + element.currentTime * 1000;
}
