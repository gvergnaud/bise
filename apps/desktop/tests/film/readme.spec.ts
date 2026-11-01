// The README film's take (readme-film): the real app (out/), off screen, in a throwaway home, on the
// scripted core with world.ts's made-up shop. Real clicks and keys; the hub's answers sent on cue.
// Not a check: it records a tape (tape.ts) the film page composes. Run by record.mts:
//   FILM_OUT=<dir> playwright test -c tests/film/film.config.ts readme
import { FakeCore, FakeHelper, expect, launchDesktop, test } from '../harness';
import { Tape } from './tape';
import { ASK, LAND_SHA, P, agents, perfLive, perfLive2, projects, world, type Row } from './world';

const OUT = process.env['FILM_OUT'] ?? '';
const W = 1360;
const H = 850;
const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

test('readme film take', async () => {
  test.setTimeout(240_000);
  if (OUT === '') throw new Error('FILM_OUT: the take folder');
  const now = Date.now();
  const core = new FakeCore(world(now) as never);
  const s = await launchDesktop({ core, helper: new FakeHelper(), route: `project/${P}`, size: `${W}x${H}` });
  try {
    const { page, cdp } = s;
    // a stable sky and no eggs (bims room.ts: the clock drives them)
    await page.evaluate(() => {
      const w = window as unknown as { __biseLocal?: unknown; __biseEggs?: unknown };
      w.__biseLocal = { hour: 11 };
      w.__biseEggs = false;
    });
    const tape = new Tape(cdp, page, OUT);
    await tape.scale(s.app, W, H, 2);
    await expect(page.getByTestId('thread.list')).toBeVisible({ timeout: 20_000 });
    // the tile on the whole office (its '⤢ fit'), not on main's bim: perf and search show as they come
    await page.getByTestId('app.outer.world').hover();
    const tileFit = page.getByTestId('app.outer.world').getByTestId('app.right.bims.fit');
    if ((await tileFit.count()) > 0) await tileFit.click();
    await page.mouse.move(W / 2, H / 2);
    await sleep(1500);
    await tape.start();
    await sleep(1200);
    await tape.mark('whole');

    // 1. his ask: a click in the composer, his line typed with real keys
    const editor = page.getByTestId('composer.field');
    await tape.mark('cursor', editor);
    await sleep(700);
    await editor.click();
    await tape.mark('click', editor);
    await sleep(300);
    await tape.mark('type', editor);
    await page.keyboard.type(ASK, { delay: 42 });
    await sleep(400);
    await core.result('send', undefined, { msg_id: 2, reason: 'made' });
    await tape.mark('enter', editor);
    await page.keyboard.press('Enter');
    await core.waitCmd((c) => c.cmd === 'send');
    const t = Date.now();
    await core.send({ ev: 'entry', project: P, agent: 'main', entry: { pos: 3, at_ms: t, kind: 'you', text: ASK, delivery: 'read', msg_id: 2 } } as never);
    await tape.mark('landed', page.getByTestId('thread.you.3'));
    await core.send(agents([{ name: 'main', status: 'working', doing: 'think' }], t) as never);
    await sleep(900);
    await core.send({ ev: 'entry', project: P, agent: 'main', entry: { pos: 4, at_ms: Date.now(), kind: 'agent', text: 'two tasks, in parallel: perf takes the page speed, search the size filters.' } } as never);
    await tape.mark('answer');
    await sleep(600);
    await core.send({ ev: 'entry', project: P, agent: 'main', entry: { pos: 5, at_ms: Date.now(), kind: 'notice', text: '✚ main → new agent @perf : product page under a second', notice: { level: 'info', text: '✚ main → new agent @perf : product page under a second' } } } as never);
    await sleep(350);
    await core.send({ ev: 'entry', project: P, agent: 'main', entry: { pos: 6, at_ms: Date.now(), kind: 'notice', text: '✚ main → new agent @search : search with size filters', notice: { level: 'info', text: '✚ main → new agent @search : search with size filters' } } } as never);
    await tape.mark('chips');
    await sleep(500);
    // the agents come: at the door first (phase starting), in the bar at once
    const rows = (perf: Partial<Row>, search: Partial<Row>): Row[] => [
      { name: 'main', status: 'idle' },
      { name: 'perf', status: 'working', title: 'product page under a second', purpose: 'product page under a second', ...perf },
      { name: 'search', status: 'working', title: 'search with size filters', purpose: 'search with size filters', ...search },
    ];
    await core.send(projects({ agents: 3, working: 2, waits: 0 }) as never);
    await core.send(agents(rows({ phase: 'starting' }, { phase: 'starting' }), Date.now()) as never);
    await tape.mark('agents', page.getByTestId('app.outer.agents').or(page.getByTestId('app.outer.world')).first());
    await sleep(2200);
    await tape.mark('whole-2');

    // 1b. perf's own thread (its row in the bar): what it does, live; then back to main's
    await core.send(agents(rows({ title: 'profiling the product page', doing: 'edit' }, { title: 'adding the size filter', doing: 'think' }), Date.now()) as never);
    const perfRow = page.getByTestId(`app.outer.agents.${P}.perf`).locator('[data-row]').first();
    await tape.mark('perf-row', perfRow);
    await sleep(900);
    await perfRow.click();
    await expect(page.getByTestId('thread.list')).toBeVisible();
    await tape.mark('perf-thread', page.getByTestId('thread.list'));
    await sleep(1600);
    for (const e of perfLive(Date.now())) await core.send({ ev: 'entry', project: P, agent: 'perf', entry: e } as never);
    await tape.mark('perf-edit');
    await sleep(2200);
    for (const e of perfLive2(Date.now())) await core.send({ ev: 'entry', project: P, agent: 'perf', entry: e } as never);
    await tape.mark('perf-measure');
    await sleep(2600);
    const mainRow = page.getByTestId(`app.outer.agents.${P}.main`).locator('[data-row]').first();
    await tape.mark('main-row', mainRow);
    await sleep(700);
    await mainRow.click();
    await sleep(900);
    await tape.mark('main-back');

    // 2. the office, full size (the tile's ⤢, then the panel full width, the whole office): they walk to their desks
    await page.getByTestId('app.outer.world').hover();
    await sleep(300);
    await page.getByTestId('app.outer.world.panel').click();
    await expect(page.getByTestId('app.panel.office')).toBeVisible({ timeout: 10_000 });
    await page.keyboard.press('Shift+Alt+Meta+B');
    await sleep(500);
    const fit = page.getByTestId('app.panel.office').getByTestId('app.right.bims.fit');
    if ((await fit.count()) > 0) await fit.click();
    await page.mouse.move(W - 4, 40);
    await sleep(900);
    await tape.mark('office', page.getByTestId('app.panel.office'));
    await core.send(agents(rows({ title: 'profiling the product page', doing: 'edit' }, { title: 'adding the size filter', doing: 'think' }), Date.now()) as never);
    await sleep(7000);
    const bims = await page.evaluate(() => (window as unknown as { __biseBims?: { trace: { name: string }[] } }).__biseBims?.trace.map((x) => x.name) ?? []);
    await tape.mark('office-end', page.getByTestId('app.panel.office'), [...new Set(bims)].join(','));
    await page.keyboard.press('Shift+Alt+Meta+B');
    await sleep(400);
    await page.keyboard.press('Meta+W');
    await sleep(400);
    if (await page.getByTestId('app.panel.office').isVisible()) await page.keyboard.press('Alt+Meta+B');
    await sleep(800);
    await tape.mark('back');

    // 3. search's question: the inbox above the composer, ⌃1 opens it, 1 answers
    const tq = Date.now();
    const card = { id: 21, project: P, kind: 'question', rank: 1, agent: 'search', question: 'which sizes go in the filter?', options: [{ n: 1, label: 'EU only, the shop ships to the EU' }, { n: 2, label: 'EU and US' }], urgent: false, since_ms: tq, text: 'which sizes go in the filter?\n1. EU only, the shop ships to the EU\n2. EU and US' };
    await core.send(projects({ agents: 3, working: 1, waits: 1 }) as never);
    await core.send(agents(rows({ title: 'profiling the product page', doing: 'build' }, { status: 'waiting', title: 'which sizes go in the filter?', waits: 1 }), tq) as never);
    await core.send({ ev: 'cards', project: P, cards: [card] } as never);
    await sleep(400);
    await tape.mark('question', page.getByTestId('inbox.block'));
    await sleep(2200);
    await tape.mark('key-open', page.getByTestId('inbox.block'), '⌃1');
    await page.keyboard.press('Control+1');
    await sleep(1600);
    await tape.mark('question-open', page.getByTestId('inbox.block'));
    await sleep(1200);
    await tape.mark('key-pick', page.getByTestId('inbox.block'), '1');
    await page.keyboard.press('1');
    await sleep(250);
    await core.send(projects({ agents: 3, working: 2, waits: 0 }) as never);
    await core.send({ ev: 'cards', project: P, cards: [] } as never);
    await core.send(agents(rows({ title: 'profiling the product page', doing: 'build' }, { title: 'adding the size filter', doing: 'edit' }), Date.now()) as never);
    await core.send({ ev: 'entry', project: P, agent: 'main', entry: { pos: 7, at_ms: Date.now(), kind: 'answered', text: 'EU only, the shop ships to the EU', answered: { agent: 'search', question: 'which sizes go in the filter?', answer: 'EU only, the shop ships to the EU', why: 'he answered' } } } as never);
    await tape.mark('answered', page.getByTestId('thread.list'));
    await sleep(2200);

    // 4. perf lands on main; a click on its sha opens the change
    await core.send({ ev: 'entry', project: P, agent: 'main', entry: { pos: 8, at_ms: Date.now(), kind: 'landed', text: '3 files +41 −7', landed: { agent: 'perf', target: 'main', from: 'sb/perf', sha: LAND_SHA, files: 3, add: 41, del: 7 } } } as never);
    await core.send(agents(rows({ status: 'done', title: 'the product page loads in 0.8 s' }, { title: 'adding the size filter', doing: 'edit' }), Date.now()) as never);
    await tape.mark('land', page.getByTestId('thread.landed.8'));
    await sleep(700);
    await core.send({ ev: 'entry', project: P, agent: 'main', entry: { pos: 9, at_ms: Date.now(), kind: 'agent', text: 'perf is on main: the product page loads in 0.8 s, it was 2.9 s.' } } as never);
    await tape.mark('land-answer', page.getByTestId('thread.list'));
    await sleep(2200);
    const sha = page.getByTestId('thread.landed.8.sha');
    await tape.mark('land-click', sha);
    await sha.click({ timeout: 5000 }).catch(() => undefined);
    await sleep(2500);
    await tape.mark('diff', page.locator('main').first());
    await sleep(2500);

    await tape.stop({ width: W, height: H, scale: 2, sha: LAND_SHA });
  } finally {
    await s.close();
    core.dispose();
  }
});
