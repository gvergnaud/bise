// His context.quote, the one rule for main and the capsule (architect m_53938): fn space may read
// his selection, the spotlight's ❝ chip. main's helper config (main/helper/config.ts) and the
// capsule (capsule/quote.ts) both read it here. Pure.

/** prefs.context.quote: off only when false; unset counts as on, as settings › privacy shows it. */
export function quoteOn(prefs: unknown): boolean {
  if (typeof prefs !== 'object' || prefs === null) return true;
  const c = (prefs as { context?: unknown }).context;
  if (typeof c !== 'object' || c === null) return true;
  return (c as { quote?: unknown }).quote !== false;
}
