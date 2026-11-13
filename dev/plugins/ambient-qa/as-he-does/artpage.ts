// An open artifact's page, as the driver finds it in the window. Three shapes draw one: bise's own
// frame (artifact.page, today's page), a plugin view's tab (app.panel.view), and an artifact its
// app opens (app/panel/Document.tsx's app.panel.doc: since Sites part 2 an html artifact opens in
// plugins/sites' view, its frame over the doc's views.tab.anchor). 11.11's lane B (80b32224e) read
// only the first two and the first match in DOM order: the Sites tab has neither, so every artifact
// row read the page as shut (drawn False) while the shots showed proof.html drawn. The driver takes
// the first VISIBLE match (a hidden or empty one before it is skipped). Pure, no Playwright.

export const ART_PAGE = '[data-testid="artifact.page"], [data-testid="app.panel.view"], [data-testid="app.panel.doc"]';

/** The first match of `sel` in `root` that has a size (drawn), else null. */
export function visibleIn(root: ParentNode, sel: string): Element | null {
  for (const e of Array.from(root.querySelectorAll(sel))) {
    const r = e.getBoundingClientRect();
    if (r.width > 0 && r.height > 0) return e;
  }
  return null;
}

/**
 * The 'artifact from the right side' row's columns, run in the page by page.evaluate(artCols,
 * ART_PAGE): self-contained (an evaluate sees no outer name), so it repeats visibleIn's loop.
 * rail: the left bar shown, left of the thread; page: a drawn artifact page; in_panel: inside the
 * panel; right_of_thread: right of the thread.
 */
export function artCols(sel: string) {
  const shown = (s: string) => {
    for (const e of Array.from(document.querySelectorAll(s))) {
      const r = e.getBoundingClientRect();
      if (r.width > 0 && r.height > 0) return e;
    }
    return null;
  };
  // the left bar: shown, and left of the thread (1c5659b8 run: the dragged sidebar is ~240 px, a
  // fixed 160 px rail check read it as missing; native-mac: the places rail sits left of it, ~56 px)
  const side = document.querySelector('[data-testid="app.sidebar"]')?.getBoundingClientRect();
  const main = document.querySelector('[data-testid="app.main"]')?.getBoundingClientRect();
  const rail = !!side && side.width > 0 && side.left < 80 && (!main || side.right <= main.left + 2);
  // native-mac: the artifact opens in the panel's tab, right of the thread (no 3-column view)
  const pg = shown(sel);
  const pr = pg?.getBoundingClientRect();
  return {
    rail, rail_w: side ? Math.round(side.width) : null, thread: !!shown('[data-testid="app.main"]'), page: !!pg,
    in_panel: !!pg?.closest('[data-testid="app.panel"]'), right_of_thread: !!pr && !!main && pr.left >= main.right - 2,
  };
}
