// The right side's coins (or its outer card) keep their own band: nothing the screen draws runs
// under them, in every layout where they show (his m_51994: they drew over /debug's charts, and
// over the thread's history once he closed an artifact's tab). Measured: the band's box against the
// content's (the thread's column, the /debug page's box) and the panel's column, window px.
import { describe, expect, it } from 'vitest';
import { closeFrontTab, goTo, tabArtifact, windowAt } from './inputs';
import { feature, type SideView } from './run';

const THREAD = goTo({ screen: 'thread', project: 'p', agent: 'main' });
const DEBUG = goTo({ screen: 'debug', project: 'p', agent: 'main' });
const DEBUG_COMP = goTo({ screen: 'debug', project: 'p', agent: 'main', tab: 'completion' });

/** The band is clear of the content, and coins between the thread and the panel stand on a panel he sees. */
function clear(v: SideView): void {
  if (v.band === null) return;
  expect(v.content.r, `the content ends left of the ${v.coins} band`).toBeLessThanOrEqual(v.band.l);
  if (v.coins === 'between') {
    expect(v.panelShows, 'coins between the thread and a panel with nothing in it').toBe(true);
    expect(v.band.r).toBe(v.panel?.l);
  }
}

describe("the right side's band against what is beside it", () => {
  it('/debug ends left of the coins and of the outer card, at every width', () => {
    for (const w of [900, 1100, 1300, 1440, 1920, 2560]) {
      for (const route of [DEBUG, DEBUG_COMP]) {
        const v = feature().does(windowAt(w), route).view.side();
        expect(v.band, `a band at ${String(w)}`).not.toBeNull();
        clear(v);
      }
    }
  });

  it('/debug beside an open artifact ends left of the coins between', () => {
    const v = feature().does(windowAt(1920), DEBUG, tabArtifact('p', 'a1')).view.side();
    expect(v.coins).toBe('between');
    clear(v);
  });

  it("the thread's column stays clear whatever the panel holds", () => {
    for (const w of [1100, 1440, 1920]) {
      clear(feature().does(windowAt(w), THREAD).view.side());
      clear(feature().does(windowAt(w), THREAD, tabArtifact('p', 'a1')).view.side());
    }
  });

  it("closing the artifact's tab folds the panel: the coins go back to the window's edge, off the history", () => {
    for (const how of ['x', 'esc'] as const) {
      for (const forget of [false, true]) {
        const w = feature().does(windowAt(1920), THREAD, tabArtifact('p', 'a1'));
        expect(w.view.side().coins).toBe('between');
        // esc with nothing known of what was before it (a relaunch, the route's own open)
        const v = w.does(closeFrontTab(how, forget)).view.side();
        expect(v.panel, `${how}${forget ? ' (before forgotten)' : ''}: the panel folds`).toBeNull();
        expect(v.coins).toBe('card');
        clear(v);
      }
    }
  });

  it('closing one of two tabs keeps the panel and its coins between', () => {
    const v = feature().does(windowAt(1920), THREAD, tabArtifact('p', 'a1'), tabArtifact('p', 'a2'), closeFrontTab('esc', true)).view.side();
    expect(v.coins).toBe('between');
    clear(v);
  });
});
