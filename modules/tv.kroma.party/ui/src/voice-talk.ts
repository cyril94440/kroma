import type { watchSpeech } from './voice-meter';
import { SESSION_TIMING, Timers } from './voice-timing';

export interface TalkDeps {
  watch: typeof watchSpeech;
  report: (speaking: boolean) => void;
}

/** Reports when the local microphone starts and stops carrying speech, and
 *  repeats "still speaking" every couple of seconds so a lost report heals
 *  itself: the server lets a speaking flag lapse a few seconds after the last
 *  one. A muted microphone never reports speech. */
export class Talk {
  private stopMeter: (() => void) | null = null;
  private speaking = false;
  private readonly timers = new Timers();

  constructor(private readonly deps: TalkDeps) {}

  get active(): boolean {
    return this.speaking;
  }

  start(stream: MediaStream, isMuted: () => boolean): void {
    this.stop();
    this.stopMeter = this.deps.watch(stream, (on) => {
      if (!(on && isMuted())) this.change(on);
    });
  }

  silence(): void {
    this.change(false);
  }

  stop(): void {
    this.stopMeter?.();
    this.stopMeter = null;
    this.timers.clearAll();
    this.speaking = false;
  }

  private change(on: boolean): void {
    if (on === this.speaking) return;
    this.speaking = on;
    this.deps.report(on);
    if (on) this.armRefresh();
    else this.timers.clear('refresh');
  }

  private armRefresh(): void {
    this.timers.arm('refresh', SESSION_TIMING.speakingRefreshMs, () => {
      if (!this.speaking) return;
      this.deps.report(true);
      this.armRefresh();
    });
  }
}
