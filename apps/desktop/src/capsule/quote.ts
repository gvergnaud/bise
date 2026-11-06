// The spotlight's ❝ chip, the capsule's side (Gabriel's v4, architect m_53938): his selection
// comes in the helper's context read at fn space (the only read that carries it); the capsule
// keeps it in React state alone, shows it as a chip he can drop, and sends it first with his
// message. His switch is shared/quote.ts's quoteOn (main's helper config reads the same). Pure.
import type { HelperEv } from "@/proto";
import type { SpotQuote } from "@/spotlight";

/** fn space's context read → the ❝ chip, or none (`take` false: his switch off or no spotlight;
 *  an excluded app, no selection, only blanks). */
export function quoteOf(ev: Extract<HelperEv, { ev: "context" }>, take: boolean): SpotQuote | null {
  const text = ev.context.selection;
  if (!take || ev.excluded === true || text == null || text.trim() === "") return null;
  return { text: text.trim(), app: ev.context.app ?? "" };
}
