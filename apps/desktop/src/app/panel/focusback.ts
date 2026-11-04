// src/app/panel · where his focus goes back when ⌘W closes the tab in front (cmd-w-artifact,
// designer m_53158): where it was before that tab opened (the composer, a list's row), never the
// body. openWhat notes the focus of the moment for the tab it opens; closeActive gives it back
// when the focus is lost with the tab (on the body, or inside the panel: the artifact's frame, the
// tab's own fields); a focus elsewhere (he typed in the composer) stays. The noted element gone: the
// composer's field. Owner: nm-panel.

/** The panel's box (Host.tsx): a focus inside it goes with its tab. */
const PANEL = '[data-testid="app.panel"]';
/** The composer's field, when the noted element is gone. */
const COMPOSER_FIELD = '[data-testid="composer.field"]';

const noted = new Map<string, HTMLElement>();

/** The focus he had outside the panel, if any (none: the body, or inside the panel). */
function outsideFocus(doc: Document): HTMLElement | undefined {
  const el = doc.activeElement;
  if (!(el instanceof HTMLElement) || el === doc.body || el.closest(PANEL) !== null) return undefined;
  return el;
}

/** A tab `key` opens: the focus of the moment is where ⌘W gives it back. */
export function noteFocus(key: string, doc: Document = document): void {
  const el = outsideFocus(doc);
  if (el === undefined) noted.delete(key);
  else noted.set(key, el);
}

/** The tab `key` is closing: a giver of the focus back, called once it closed; none when his focus is elsewhere (it stays). */
export function focusBackOf(key: string, doc: Document = document): (() => void) | undefined {
  const el = noted.get(key);
  noted.delete(key);
  if (outsideFocus(doc) !== undefined) return undefined;
  return () => {
    const to = el?.isConnected === true ? el : doc.querySelector<HTMLElement>(COMPOSER_FIELD);
    to?.focus({ preventScroll: true });
  };
}
