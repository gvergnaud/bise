// A card in the spotlight (the capsule, fn+space) linked to an artifact (his m_53914: '↗
// bise-pour-vibe page' did nothing): its chip, its option that opens the page and its ⏎ bring bise's
// main window to the front, the artifact in its panel's tab at its block, the window staying on
// the screen he was on, even when the card is another project's; the card stays waiting, nothing
// is answered. Typed inputs in; what the capsule asked of the main window, the effects that window
// ran, its route and the commands sent out. Nothing else is read.
import { describe, expect, it } from 'vitest';
import { card, ev, goTo, inCapsule, projects, row } from './inputs';
import { feature } from './run';

const ACME = 'acme-1a2b3c4d';
const BISE = 'bise-5e6f7a8b';
const PAGE = 'http://127.0.0.1:47123/p/bise-pour-vibe';
const vibe = { id: 'bise-pour-vibe', kind: 'page', title: 'bise-pour-vibe', v: 3, url: PAGE, at: { kind: 'block', block: 'shortcuts' } } as const;
const asks = card({
  project: BISE,
  id: 77,
  agent: 'main',
  question: 'bise → vibe desktop: create the Notion page: shortcuts?',
  artifact: vibe,
  options: [
    { n: 1, label: 'open the page', act: { act: 'open', url: `${PAGE}#shortcuts`, artifact: 'bise-pour-vibe', at: 'shortcuts' } },
    { n: 2, label: 'create the Notion page: shortcuts' },
  ],
});
const listed = ev('artifacts', { project: BISE, items: [{ id: 'bise-pour-vibe', kind: 'page', title: 'bise-pour-vibe', agent: 'main', url: PAGE, version: 3, at_ms: 1_791_100_000_000, new: false }] });
const rows = projects(row({ project: ACME, name: 'acme' }), row({ project: BISE, name: 'bise', cards: [asks], waits: 1 }));
// the window is on another project's thread than the card's
const here = { screen: 'thread', project: ACME, agent: 'main' } as const;
const opened = { out: 'nav', to: { project: BISE, artifact: 'bise-pour-vibe', at: 'shortcuts' }, tab: true } as const;
const waiting = (w: ReturnType<typeof feature>): string[] => w.view.inbox().map((c) => `${c.project}-${String(c.id)}`);

describe("a capsule card's artifact", () => {
  it("its chip opens it in the main window's panel tab at its block, another project's too: the window stays, the card waits", () => {
    const w = feature().given(rows, listed).does(goTo(here), inCapsule(asks, 'chip'));
    expect(w.windowAsks()).toHaveLength(1);
    expect(w.effects()).toEqual([opened]);
    expect(w.view.route()).toEqual(here);
    expect(waiting(w)).toEqual([`${BISE}-77`]);
    expect(w.sent().filter((c) => c.cmd === 'answer' || c.cmd === 'close')).toEqual([]);
  });

  it("'1 open the page' opens it the same way and answers nothing; '2' answers", () => {
    const w = feature().given(rows, listed).does(goTo(here), inCapsule(asks, { pick: 1 }));
    expect(w.effects()).toEqual([opened]);
    expect(w.view.route()).toEqual(here);
    expect(w.sent().filter((c) => c.cmd === 'answer')).toEqual([]);

    w.does(inCapsule(asks, { pick: 2 }));
    expect(w.sent()).toContainEqual({ cmd: 'answer', project: BISE, card: 77, reply: '2' });
  });

  it("⏎ 'open in bise' opens this card in the window's inbox, not its first item", () => {
    const w = feature().given(rows, listed).does(goTo(here), inCapsule(asks, 'enter'));
    expect(w.windowAsks()).toEqual([`inbox?card=${BISE}/77`]);
    expect(w.effects()).toEqual([]);
  });

  it('a page the list no longer has opens its url in his browser, from the capsule', () => {
    const w = feature().given(rows).does(inCapsule(asks, { pick: 1 }));
    expect(w.windowAsks()).toEqual([]);
    expect(w.capsuleEffects()).toEqual([{ out: 'fx', fx: { fx: 'openExternal', args: [`${PAGE}#shortcuts`] } }]);
  });
});
