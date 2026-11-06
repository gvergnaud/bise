// The spotlight's ❝ chip, the capsule's side (Gabriel's v4, architect m_53938): his selection
// comes in the helper's context read at fn space (the only read that carries it); the capsule
// keeps it in React state alone, shows it as a chip he can drop, and sends it first with his
// message. Pure.
import type { HelperEv } from "@/proto";
import type { SpotQuote } from "@/spotlight";

/** His switch, prefs.context.quote: off only when false (unset: on, as settings › privacy shows it). */
export function quotePref(prefs: unknown): boolean {
  if (typeof prefs !== "object" || prefs === null) return true;
  const c = (prefs as { context?: unknown }).context;
  if (typeof c !== "object" || c === null) return true;
  return (c as { quote?: unknown }).quote !== false;
}

/** fn space's context read → the ❝ chip, or none (`take` false: his switch off or no spotlight;
 *  an excluded app, no selection, only blanks). */
export function quoteOf(ev: Extract<HelperEv, { ev: "context" }>, take: boolean): SpotQuote | null {
  const text = ev.context.selection;
  if (!take || ev.excluded === true || text == null || text.trim() === "") return null;
  return { text: text.trim(), app: ev.context.app ?? "" };
}
