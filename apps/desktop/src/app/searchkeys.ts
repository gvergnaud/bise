// A list page's search keys (his m_52658, designer m_52674), the same on Artifacts, Plugins,
// Scheduled, the inbox and Running: '/' focuses the page's search (ds ListSearch) wherever the focus
// is on the page, unless a text field has it ('/' is a letter there, the composer's slash list);
// ⌘F (the menu's Find) focuses it too. In a thread ⌘F stays the find bar (ThreadScreen.tsx). What
// the field's own keys do (esc, ⏎, ↓) stays each screen's; Running's open output keeps ⌘F for its find. Owner: nm-pages-a.
import { useEffect, type RefObject } from 'react';
import { useMenuAction } from './menu';

export interface SlashKey {
  readonly key: string;
  readonly metaKey: boolean;
  readonly ctrlKey: boolean;
  readonly altKey: boolean;
  readonly defaultPrevented: boolean;
  readonly target: EventTarget | null;
}

/** A text field has the focus: '/' is a letter there. Pure. */
export const isField = (t: EventTarget | null): boolean =>
  t instanceof HTMLElement && (t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || t.isContentEditable);

/** '/' alone, not taken yet, outside a text field: the page's search takes it. Pure. */
export function slashFocuses(e: SlashKey): boolean {
  return e.key === '/' && !e.metaKey && !e.ctrlKey && !e.altKey && !e.defaultPrevented && !isField(e.target);
}

/** While the page is shown: '/' and ⌘F focus its search (`field`, a ds ListSearch's input).
 *  `ownFind`: ⌘F is the page's own first while it says so (Running's open output: its find). */
export function usePageSearchKeys(field: RefObject<HTMLInputElement | null>, ownFind?: () => boolean): void {
  useEffect(() => {
    const slash = (e: KeyboardEvent) => {
      const el = field.current;
      if (el === null || !slashFocuses(e)) return;
      e.preventDefault();
      el.focus();
    };
    window.addEventListener('keydown', slash);
    return () => { window.removeEventListener('keydown', slash); };
  }, [field]);
  useMenuAction('find', () => {
    if (ownFind?.() === true) return;
    const el = field.current;
    if (el === null) return;
    el.focus();
    el.select();
  });
}
