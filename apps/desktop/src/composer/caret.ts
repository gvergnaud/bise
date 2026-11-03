// The caret asked for a composer that is about to show (ui-review m_52822: after 'ask perf about
// it' or '⋯ put it in a message' the chip was in perf's draft but the focus stayed on the clicked
// button): askCaret names its target ('p/agent'); that composer, once its editor is ready, takes
// the ask and puts the caret at its end, after the chip (Composer.tsx). One ask at a time, spent
// once taken or by the next ask. Owner: artifact-find.

let asked: string | null = null;

/** The composer of `target` takes the focus, caret at its end, when it next shows. */
export function askCaret(target: string): void {
  asked = target;
}

/** Whether the composer of `target` was asked for the caret (spent once true). */
export function takeCaret(target: string): boolean {
  if (asked !== target) return false;
  asked = null;
  return true;
}
