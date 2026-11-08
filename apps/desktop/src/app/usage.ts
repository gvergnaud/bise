// The window's side of the app's usage counts (page usage-metrics-plan, step 3; main/usage keeps
// the time): his input as a signal with no content, at most once every INPUT_EVERY_MS, and his
// gestures by their closed name (bise_proto's AppGesture). Nothing else leaves the window: never
// a key, a word, a path or a name. The window keeps no state of its own about it. Owner: telemetry.
import type { AppGesture } from '@/proto';

/** At most one input signal this often (main's engaged minute is 60 s: 5 s is close enough). */
export const INPUT_EVERY_MS = 5_000;

const usage = (): { input: () => void; gesture: (g: AppGesture) => void } | undefined => (window as Partial<Window>).bise?.usage;

let wired = false;

/** Listens to his keys, clicks and scrolls, once per window: a signal to main, never the event. */
export function wireUsageInput(): void {
  if (wired) return;
  wired = true;
  let last = -Infinity;
  const on = (): void => {
    const now = Date.now();
    if (now - last < INPUT_EVERY_MS) return;
    last = now;
    usage()?.input();
  };
  for (const type of ['keydown', 'pointerdown', 'wheel'] as const) window.addEventListener(type, on, { capture: true, passive: true });
}

/** One of his gestures, by its name. */
export function countGesture(g: AppGesture): void {
  usage()?.gesture(g);
}

/** The gesture of an artifact opened, by its kind (the hub's: switchboard artifacts.rs). Pure. */
export function artifactGesture(kind: string): AppGesture {
  switch (kind) {
    case 'page':
      return 'artifact_page';
    case 'doc':
      return 'artifact_doc';
    case 'sheet':
      return 'artifact_sheet';
    case 'slides':
      return 'artifact_slides';
    case 'image':
      return 'artifact_image';
    case 'link':
      return 'artifact_link';
    case 'code':
      return 'artifact_code';
    default:
      return 'artifact_other';
  }
}
