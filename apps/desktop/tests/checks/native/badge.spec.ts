// native/badge · bar W33/N10 (amb-mac 4ded46e1): with the window in the back he sees what waits, as
// the TUI's tab title did. Passes when the Dock's badge is the number of what waits (none at 0), bise's
// notifications ask included as the inbox lists it, and follows the core: a new card counts at once,
// answered ones drop out. The window's title no longer counts: since bise fait mac's frame
// (b06021770, native-mac §6) it is what he looks at (app/title.ts, its unit tests), so the count is
// the badge's alone. The window is never focused in a test run, so it is always 'in the back'.
// Test mode has no Dock icon: main prints the badge as BADGE_LINE on stdout. The Dock icon itself is
// the VM pass's.
import { FakeCore, test, expect } from '../../harness';
import type { CoreEv, HubEv } from '../../../src/proto';
import { core_ev, hub_ev } from '../../../src/proto/fixtures.gen';
import { onWorld, type World } from './world';

const BADGE = 'bise-desktop: badge';

type Cards = Extract<HubEv, { ev: 'cards' }>;

/** the fixture world with acme's cards cut to `n` (the projects line says the same) */
function world(n: number): { start: (CoreEv | HubEv)[]; cards: Cards } {
  const cards = hub_ev.find((e): e is Cards => e.ev === 'cards');
  if (!cards) throw new Error('no cards in the fixtures');
  const two: Cards = { ...cards, cards: cards.cards.slice(0, n) };
  const start: (CoreEv | HubEv)[] = [
    ...core_ev.map((e) =>
      e.ev === 'projects' ? { ...e, projects: e.projects.map((p) => (p.project === two.project ? { ...p, cards: two.cards, waits: n } : p)) } : e,
    ),
    ...hub_ev.map((e) => (e.ev === 'cards' ? two : e)),
  ];
  return { start, cards: two };
}

const badge = (w: World): string => {
  const all = w.out.of(BADGE);
  const last = all[all.length - 1];
  return last === undefined ? '(none said)' : (JSON.parse(last) as string);
};

test('native/badge: the Dock badge counts what waits, and follows the core', async () => {
  const { start, cards } = world(2);
  await onWorld(async (w) => {
    // the start world's badge line goes out before the check reads main's stdout, so the start
    // count is read from the first change: a third acme card. The fixture world has cards in other
    // projects too, and in a fresh test home bise's notifications ask waits beside them (one number,
    // designer m_25690): read the count once it holds, then each change moves it by what changed.
    // The ask lands a moment after the cards (+1): n is the count only once it holds still for a
    // while, else a late ask counts in the next card's step
    const first = cards.cards[0];
    if (!first) throw new Error('no card');
    const three = [...cards.cards, { ...first, id: 97, question: 'merge it?' }];
    await w.core.send({ ...cards, cards: three });
    let n = 0;
    let seen = '';
    let since = 0;
    await expect
      .poll(() => {
        const b = badge(w);
        n = /^\d+$/.test(b) ? Number(b) : 0;
        if (b !== seen) {
          seen = b;
          since = Date.now();
        }
        return n >= 3 && Date.now() - since >= 1000;
      }, { message: 'the badge counts the cards, and holds', timeout: 6000, intervals: [100] })
      .toBe(true);
    await w.core.send({ ...cards, cards: [...three, { ...first, id: 98, question: 'ship it?' }] });
    await expect.poll(() => badge(w), { message: 'a new card counts at once', timeout: 2000 }).toBe(String(n + 1));
    // acme's cards answered: its 4 leave the count, the other projects' stay
    await w.core.send({ ...cards, cards: [] });
    await expect.poll(() => badge(w), { message: "acme's answered ones drop out", timeout: 2000 }).toBe(n - 3 > 0 ? String(n - 3) : '');
  }, { core: new FakeCore(start) });
});
