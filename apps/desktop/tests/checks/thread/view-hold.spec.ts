// thread/view-hold · a view's card keeps its y when it goes to the panel and when it comes back
// (designer m_50494: the line he clicked stays where his pointer was; mcp-apps-demo m_50786: on a
// long thread the click took the thread up, older rows and 'back to the bottom'). On FakeCore, a
// thread of 200 entries, 1440 (the panel narrows the thread) and 1100 (the panel's sheet over it):
//   - mid: he scrolled up to a view's card in the middle of the thread;
//   - end: the card is the last entry's, the thread stuck to its end.
// Every drawn frame from his click to 1.5 s after, the card's top is read. Passes when the card is
// on every frame within 1 px of the y it had (end: never lower than that y and never above it, the
// list only clamped at its end), and a thread stuck to its end is still stuck to it with no 'back to
// the bottom', for the card's link to the panel then its link back. Before the fix: the end's click unstuck the
// thread (views/Card.tsx's fake wheel), and scrolled up the card moved 4 px (1100) to 26 px (1440),
// the list holding the row's .th-row, not the card. Shots before and after each click
// (view-hold-<where>-<width>-<panel|here>-<0|1>.png), for designer.
import type { Page } from '@playwright/test';
import { FakeCore, launchDesktop, test, expect } from '../../harness';
import { thread } from '../../../src/proto/words_thread.gen';
import { views } from '../../../src/proto/words_views.gen';

const ACME = 'acme-1a2b3c4d';
const T0 = Date.now() - 120_000;
const N = 200;
const WORDS = 'the cold run is slow because the index is rebuilt at every start of the hub, so we keep it warm between two runs of the gate and measure again';
const CALL = (pos: number): string => `${String(T0)}-${String(pos)}`;

type E = Record<string, unknown>;
const para = (n: number, i: number): string => Array.from({ length: n }, (_, k) => `${String(i)}.${String(k)} ${WORDS}.`).join(' ');
const you = (pos: number): E => ({ pos, at_ms: T0 + pos * 100, kind: 'you', text: `where are we on step ${String(pos)}?`, delivery: 'read', msg_id: 1000 + pos });
const agent = (pos: number): E => ({ pos, at_ms: T0 + pos * 100, kind: 'agent', text: para(2, pos) });
const viewed = (pos: number): E => ({
  pos,
  at_ms: T0 + pos * 100,
  kind: 'tools',
  text: thread.did_run({ n: 1 }),
  tools: {
    count: 1,
    summary: thread.did_run({ n: 1 }),
    items: [{ pos, id: pos, at_ms: T0 + pos * 100, name: 'run_typescript', text: 'composing the tune', kind: 'run', state: 'ok', views: [{ plugin: 'piano', server: 'piano', uri: 'ui://piano/compose', call: CALL(pos), state: 'live' }] }],
  },
});

/** 200 entries, his asks and long answers; the view's tool entry at `at`. */
function history(at: number): E[] {
  return Array.from({ length: N }, (_, i) => {
    const pos = i + 1;
    if (pos === at) return viewed(pos);
    return pos % 2 === 1 ? you(pos) : agent(pos);
  });
}

const RESOURCE = { origin: 'http://v-0123456789abcdef.localhost:47123', html: '<p>piano</p>', csp: {}, permissions: [], prefersBorder: false, toolInput: {}, toolResult: { structuredContent: {} }, tool: { name: 'compose' } };

type Seen = { ys: (number | null)[]; stuck: (string | null)[]; pill: boolean[] };

/** Reads the card's top, the list's data-stuck and its 'back to the bottom' on every frame for `ms`. */
async function watch(page: Page, id: string, ms: number): Promise<Seen> {
  return page.evaluate(
    async ({ id, ms }) => {
      const seen: Seen = { ys: [], stuck: [], pill: [] };
      const until = performance.now() + ms;
      await new Promise<void>((done) => {
        const tick = (): void => {
          seen.ys.push(document.querySelector(`[data-testid="${id}"]`)?.getBoundingClientRect().top ?? null);
          seen.stuck.push(document.querySelector('[data-testid="thread.list"]')?.getAttribute('data-stuck') ?? null);
          seen.pill.push(document.querySelector('[data-testid="thread.to-end"]') !== null);
          if (performance.now() < until) requestAnimationFrame(tick);
          else done();
        };
        requestAnimationFrame(tick);
      });
      return seen;
    },
    { id, ms },
  );
}

const atEnd = (page: Page): Promise<boolean> =>
  page.evaluate(() => {
    const sc = document.querySelector<HTMLElement>('[data-testid="thread.list"] [data-virtuoso-scroller]');
    return sc !== null && sc.scrollTop + sc.clientHeight >= sc.scrollHeight - 2;
  });

for (const width of [1440, 1100] as const)
  for (const where of ['mid', 'end'] as const) {
    test(`thread/view-hold: the card keeps its y, to the panel and back (${where}, ${String(width)})`, async () => {
      test.setTimeout(90_000);
      const core = new FakeCore();
      const s = await launchDesktop({ core, route: `agent/${ACME}/perf` });
      try {
        const { page } = s;
        await s.app.evaluate(({ BrowserWindow }, w) => {
          BrowserWindow.getAllWindows().find((b) => b.webContents.getURL().includes('surface=window'))?.setContentSize(w, 900);
        }, width);
        await expect.poll(() => page.evaluate(() => window.innerWidth), { timeout: 5000 }).toBe(width);
        await expect(page.getByTestId('thread.list')).toBeVisible({ timeout: 15_000 });
        // the panel closed before his click, as his was (the start world opens no view's tab)
        await expect(page.locator('[data-testid^="app.panel.tab."][data-testid$=".close"]')).toHaveCount(0);
        for (let i = 0; i < 6; i++) await core.result('view_resource', undefined, RESOURCE);
        const at = where === 'mid' ? N / 2 : N;
        await core.send({ ev: 'thread', project: ACME, agent: 'perf', entries: history(at), before: null, more: false } as never);
        const id = `views.card.${CALL(at)}`;
        const card = page.getByTestId(id);
        if (where === 'mid') {
          // he scrolls up to it: the card in the middle of the view
          await expect(page.getByTestId(`thread.you.${String(N - 1)}`)).toBeVisible({ timeout: 10_000 });
          const box = await page.getByTestId('thread.list').boundingBox();
          if (box === null) throw new Error('no list');
          await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
          // wheel up until the card is drawn (Virtuoso draws the rows near the view only)
          await page.mouse.wheel(0, -100);
          for (let i = 0; i < 200 && (await card.count()) === 0; i++) {
            await page.evaluate(() => {
              const sc = document.querySelector<HTMLElement>('[data-testid="thread.list"] [data-virtuoso-scroller]');
              if (sc !== null) sc.scrollTop -= 300;
            });
            await page.waitForTimeout(50);
          }
          await expect(card).toBeVisible({ timeout: 10_000 });
          await card.evaluate((el) => {
            el.scrollIntoView({ block: 'center' });
          });
          await page.mouse.wheel(0, -1);
        }
        await expect(card).toHaveAttribute('data-state', 'live', { timeout: 10_000 });
        await page.waitForTimeout(800);
        const stuck = where === 'end';
        await expect(page.getByTestId('thread.list')).toHaveAttribute('data-stuck', String(stuck));
        if (stuck) await expect.poll(() => atEnd(page), { timeout: 3000, message: 'the end: the list at its end before the click' }).toBe(true);

        for (const [step, button, state] of [
          [views.open_panel, 'views.fullscreen', 'panel'],
          [views.back_inline, 'views.inline', 'live'],
        ] as const) {
          const y0 = (await card.boundingBox())?.y;
          if (y0 === undefined) throw new Error(`${step}: no card`);
          const shot = `view-hold-${where}-${String(width)}-${state === 'live' ? 'here' : 'panel'}`;
          await page.screenshot({ path: test.info().outputPath(`${shot}-0.png`) });
          const seen = watch(page, id, 1500);
          const press = card.getByTestId(button);
          // at 1100 the panel's sheet lies over the line: his press reaches it as the same events
          if (width === 1100 && state === 'live') {
            await press.dispatchEvent('pointerdown');
            await press.dispatchEvent('click');
          } else await press.click();
          const got = await seen;
          await page.screenshot({ path: test.info().outputPath(`${shot}-1.png`) });
          await expect(card).toHaveAttribute('data-state', state);
          const missing = got.ys.filter((y) => y === null).length;
          const ys = got.ys.filter((y): y is number => y !== null);
          const worst = Math.max(...ys.map((y) => Math.abs(y - y0)));
          console.log(`${where} ${String(width)} ${step}: y0 ${String(y0)}, worst drift ${worst.toFixed(1)} px over ${String(ys.length)} frames, ${String(missing)} without the card, last ${String(ys.at(-1))}, stuck ${String(got.stuck.at(-1))}, pill ${String(got.pill.some(Boolean))}`);
          test.info().annotations.push({ type: 'drift', description: JSON.stringify({ where, width, step, y0, worst, missing, last: ys.at(-1) }) });
          expect(missing, `${step}: the card on every frame`).toBe(0);
          if (!stuck) expect(worst, `${step}: its y held on every frame (px)`).toBeLessThan(1);
          else if (state === 'panel') {
            // the card shrinks at the list's end: the list only clamps there, the card goes down by
            // what the end gave back, never up
            expect(Math.min(...ys.map((y) => y - y0)), `${step}: never above its y (px)`).toBeGreaterThanOrEqual(-1);
          } else {
            // the line grows into the card at the list's end: the list follows its end so the card
            // shows (a fold opened at the end, designer m_43996), its top never above the list's
            const top = await page.getByTestId('thread.list').evaluate((el) => el.getBoundingClientRect().top);
            expect(Math.min(...ys) - top, `${step}: the card's top in the list on every frame (px)`).toBeGreaterThanOrEqual(-1);
            expect(Math.max(...ys.map((y) => y - y0)), `${step}: never below its y (px)`).toBeLessThanOrEqual(1);
          }
          await expect(page.getByTestId('thread.list'), `${step}: stuck to its end as it was`).toHaveAttribute('data-stuck', String(stuck));
          expect(got.pill.some(Boolean) && stuck, `${step}: no 'back to the bottom' at the end`).toBe(false);
          if (stuck) expect(await atEnd(page), `${step}: still at its end`).toBe(true);
          await page.waitForTimeout(500);
        }
      } finally {
        await s.close();
      }
    });
  }
