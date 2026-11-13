// node --experimental-strip-types --test dev/plugins/ambient-qa/as-he-does/artpage.test.ts
// The driver's pick of an open artifact's page on a fake DOM: 11.11 lane B's Sites tab (app.panel.doc,
// no artifact.page nor app.panel.view) and a hidden page before the drawn one.
import test from 'node:test';
import assert from 'node:assert/strict';
import { ART_PAGE, artCols, visibleIn } from './artpage.ts';

type Box = { left: number; top: number; width: number; height: number };
class El {
  readonly testid: string;
  readonly box: Box;
  readonly parent: El | null;
  constructor(testid: string, box: Box, parent: El | null = null) {
    this.testid = testid;
    this.box = box;
    this.parent = parent;
  }
  getBoundingClientRect() {
    const b = this.box;
    return { ...b, x: b.left, y: b.top, right: b.left + b.width, bottom: b.top + b.height };
  }
  closest(sel: string): El | null {
    for (let e: El | null = this; e; e = e.parent) if (matches(e, sel)) return e;
    return null;
  }
}
// '[data-testid="x"], [data-testid="y"]' only, as the driver writes them
const matches = (e: El, sel: string) => sel.split(',').some((s) => s.trim() === `[data-testid="${e.testid}"]`);
const fakeDoc = (els: El[]) => ({
  querySelectorAll: (sel: string) => els.filter((e) => matches(e, sel)),
  querySelector: (sel: string) => els.find((e) => matches(e, sel)) ?? null,
});
const ZERO = { left: 0, top: 0, width: 0, height: 0 };
// the window of rows2-6-artifact-from-side: rail and sidebar, the thread, the panel on the right
const shell = () => {
  const panel = new El('app.panel', { left: 525, top: 40, width: 500, height: 540 });
  return {
    panel,
    els: [new El('app.sidebar', { left: 45, top: 40, width: 160, height: 540 }), new El('app.main', { left: 205, top: 40, width: 320, height: 540 }), panel],
  };
};
const cols = (els: El[]) => {
  (globalThis as { document?: unknown }).document = fakeDoc(els);
  try {
    return artCols(ART_PAGE);
  } finally {
    delete (globalThis as { document?: unknown }).document;
  }
};

test("lane B's Sites tab: an html artifact its app opens (app.panel.doc) is the open page, in the panel right of the thread", () => {
  const { panel, els } = shell();
  const doc = new El('app.panel.doc', { left: 525, top: 40, width: 500, height: 540 }, panel);
  assert.equal(visibleIn(fakeDoc([...els, doc]) as unknown as ParentNode, ART_PAGE), doc);
  assert.deepEqual(cols([...els, doc]), { rail: true, rail_w: 160, thread: true, page: true, in_panel: true, right_of_thread: true });
});

test('a hidden artifact.page before the drawn page is skipped', () => {
  const { panel, els } = shell();
  const hidden = new El('artifact.page', ZERO);
  const view = new El('app.panel.view', { left: 525, top: 40, width: 500, height: 540 }, panel);
  assert.equal(visibleIn(fakeDoc([hidden, ...els, view]) as unknown as ParentNode, ART_PAGE), view);
  assert.equal(cols([hidden, ...els, view]).in_panel, true);
});

test('no page drawn: shut', () => {
  const { els } = shell();
  const hidden = new El('app.panel.doc', ZERO);
  assert.equal(visibleIn(fakeDoc([...els, hidden]) as unknown as ParentNode, ART_PAGE), null);
  assert.deepEqual(cols([...els, hidden]), { rail: true, rail_w: 160, thread: true, page: false, in_panel: false, right_of_thread: false });
});
