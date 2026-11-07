// app/keep-all · his ask (ambient-lead m_11756, m_11994): nothing he typed or opened is lost across
// a quit + relaunch on the same userData (the update swap; a reload or a core restart reads the same
// keep files: app/keep.test.ts, store/keepOpen.test.ts), on EVERY surface amb-win's keep wires (d666c9a3), not only perf's thread (app/keep).
// qa-type's pass, on the scripted core (fixture world: acme's main, perf, old-fix; its cards) and
// the scripted helper, real keys and clicks only. One test per surface group, each through one
// relaunch on one userData, under 30 s (Gabriel's pick on page e2e-desktop); after each, the relaunch opens with no start route (the kept one).
// Passes when, after the relaunch:
//   - threads: two targets' drafts (acme main, perf), ⌘F's needle, a tools row opened and the
//     terminal panel shown are back; his next key lands after his words (caret at the end);
//   - agents: Tune open on perf with a half-typed rename and model id, then the new-agent sheet's
//     brief and name;
//   - ⌘I: a half-typed answer on the first card;
//   - spotlight: his half-typed words (fn space again after a relaunch);
//   - a send drops the composer's words for good (gone after the next relaunch);
//   - a key field is never kept: a fake key typed in settings is in no userData/keep-*.json, and
//     no keep file holds anything shaped like a key.
// Known not kept yet (d666c9a3): the agents list's focused row, other lists' scroll, the ⌘K query.
import { readdirSync, readFileSync, rmSync } from 'node:fs';
import path from 'node:path';
import type { Page } from '@playwright/test';
import { FakeCore, FakeHelper, launchDesktop, testUserData, test, expect, type Session } from '../../harness';
import { capsulePage } from '../native/world';

const ACME = 'acme-1a2b3c4d';
const FAKE_KEY = 'sk-qa-NOT-A-REAL-KEY-7f3a9c1e5b';
/** shapes a secret would have in a keep file */
const SECRETISH = /sk-[A-Za-z0-9-]{12,}|api[_-]?key|bearer\s+[A-Za-z0-9]/i;

type Cycle = 'after a relaunch';

/** The keep files of a userData, whole. */
function keepFiles(userData: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const f of readdirSync(userData)) if (/^keep-.*\.json$/.test(f)) out[f] = readFileSync(path.join(userData, f), 'utf8');
  return out;
}

/** His next key lands after the words: typed into the focused field if it kept the focus, else after a click at its end. */
async function caretAtEnd(page: Page, field: ReturnType<Page['locator']>, words: string, what: string): Promise<void> {
  const focused = await field.evaluate((f) => f.contains(document.activeElement)).catch(() => false);
  test.info().annotations.push({ type: 'focus', description: `${what}: focused on its own ${String(focused)}` });
  if (!focused) {
    // the field may still move (a sheet opening after the relaunch, slow under load): its end is
    // read once its box holds still for 100 ms, so the click lands at the end of his words
    // (keep-all's load flake: the model id's caret not after them at load >= 37)
    let b = await field.boundingBox();
    await expect.poll(async () => {
      const prev = b;
      await page.waitForTimeout(100);
      b = await field.boundingBox();
      return prev !== null && b !== null && prev.x === b.x && prev.y === b.y && prev.width === b.width;
    }, { message: `${what}: the field holds still`, timeout: 5000 }).toBe(true);
    if (b) await page.mouse.click(b.x + b.width - 4, b.y + b.height - 6);
    else await field.click();
  }
  await page.keyboard.type('§');
  const read = async (): Promise<string> => field.evaluate((f) => (f instanceof HTMLInputElement || f instanceof HTMLTextAreaElement ? f.value : (f as HTMLElement).innerText).replace(/\n$/, ''));
  expect.soft(await read(), `${what}: his next key goes after his words`).toBe(`${words}§`);
  await page.keyboard.press('Backspace');
  await expect.poll(read).toBe(words);
}

/**
 * The next native menu (Menu.popup) is never drawn: its item `id` is picked as his click would,
 * then the menu closes. Once: the popup after it is macOS's again.
 */
async function pickInNextMenu(s: Session, id: string): Promise<void> {
  await s.app.evaluate(({ Menu }, pick) => {
    const proto = Menu.prototype as unknown as { popup: (o?: { callback?: () => void }) => void };
    const own = proto.popup;
    proto.popup = function (this: { getMenuItemById: (i: string) => { click: () => void } | null }, o?: { callback?: () => void }) {
      proto.popup = own;
      const item = this.getMenuItemById(pick);
      if (item === null) throw new Error(`no '${pick}' in the menu`);
      item.click();
      o?.callback?.();
    };
  }, id);
}

/**
 * One cycle on one userData: quit + relaunch (no start route), the one that loses the renderer's
 * memory and the core both; a reload and a core restart read the same keep files (app/keep.test.ts).
 * `check` reads everything back; returns the last session, still open.
 */
async function cycles(o: { route: string; helper?: boolean; setup: (s: Session, h?: FakeHelper) => Promise<void>; check: (s: Session, c: Cycle, h?: FakeHelper) => Promise<void> }): Promise<{ s: Session; userData: string; dispose: () => Promise<void> }> {
  const userData = testUserData();
  const core = new FakeCore();
  const helper = o.helper ? new FakeHelper() : undefined;
  const s = await launchDesktop({ core, ...(helper ? { helper } : {}), route: o.route, userData });
  try {
    await o.setup(s, helper);
    await s.page.waitForTimeout(700); // past the keep's 300 ms write
    const gone = new Promise<void>((r) => s.app.process().once('exit', () => { r(); }));
    await s.app.evaluate(({ app }) => { app.quit(); }).catch(() => undefined);
    await Promise.race([gone, new Promise<void>((r) => setTimeout(r, 10_000))]);
  } finally {
    await s.close();
    core.dispose();
    helper?.dispose();
  }
  const core2 = new FakeCore();
  const helper2 = o.helper ? new FakeHelper() : undefined;
  const s2 = await launchDesktop({ core: core2, ...(helper2 ? { helper: helper2 } : {}), userData });
  await o.check(s2, 'after a relaunch', helper2);
  const files = keepFiles(userData);
  test.info().annotations.push({ type: 'keep files', description: Object.keys(files).join(' ') });
  for (const [f, body] of Object.entries(files)) expect.soft(body, `${f} holds nothing shaped like a key`).not.toMatch(SECRETISH);
  return {
    s: s2,
    userData,
    dispose: async () => {
      await s2.close();
      core2.dispose();
      helper2?.dispose();
      rmSync(path.dirname(userData), { recursive: true, force: true });
    },
  };
}

test("app/keep-all: threads: two targets' drafts, ⌘F, an open row, the terminal panel; a send drops its words", async () => {
  test.setTimeout(180_000);
  const MAIN = 'main: à finir après le café';
  const PERF = 'perf: la moitié d’une question 👍';
  const field = (p: Page) => p.locator('[data-testid="composer.field"]').first();
  // ⌃` shows the terminal where it lives (terminal/panel.ts place): a shell tab in the right panel
  // by default since bise fait mac, the bottom panel when he moved it there
  const term = (p: Page) => p.locator(`[data-testid="terminal.panel"], [data-testid="terminal.tab"][data-project="${ACME}"]:not([hidden])`).first();
  const { s, userData, dispose } = await cycles({
    route: `agent/${ACME}/perf`,
    setup: async (s) => {
      const { page } = s;
      await expect(field(page), "perf's composer").toBeVisible({ timeout: 10_000 });
      await field(page).click();
      await page.keyboard.type(PERF);
      // acme's row in the sidebar opens its main's thread (one row per project since a80965b1d;
      // app/Sidebar.tsx ProjectItem)
      await page.getByTestId(`app.side.project.${ACME}`).click();
      await expect(page.getByTestId('thread.screen')).toHaveAttribute('data-agent', 'main', { timeout: 5000 });
      await field(page).click();
      await page.keyboard.type(MAIN);
      // back to perf by the bar's ‹ (a click on perf's outer row opens its ask in place when it asks)
      await page.getByTestId('app.bar.back').click();
      await expect(page.getByTestId('thread.screen')).toHaveAttribute('data-agent', 'perf', { timeout: 5000 });
      // the thread comes back by ‹ and settles (its kept scroll, the list's first draw): a click
      // in that redraw can be lost, so he clicks again while the row is still closed
      const tools = page.getByTestId('thread.tools.17');
      await expect(async () => {
        if ((await tools.getAttribute('aria-expanded')) !== 'true') await tools.click();
        await expect(tools).toHaveAttribute('aria-expanded', 'true', { timeout: 1000 });
      }).toPass({ timeout: 10_000 });
      await page.keyboard.press('Meta+f');
      await expect(page.getByTestId('thread.find.field'), '⌘F opens').toBeFocused({ timeout: 3000 });
      await page.keyboard.type('cold');
      await page.keyboard.press('Control+Backquote');
      await expect(term(page), '⌃` shows the terminal').toBeVisible({ timeout: 5000 });
    },
    check: async (s, c) => {
      const { page } = s;
      await expect.soft(page.getByTestId('thread.screen'), `${c}: perf's thread`).toHaveAttribute('data-agent', 'perf', { timeout: 10_000 });
      await expect.soft(field(page), `${c}: perf's words`).toContainText(PERF, { timeout: 5000 });
      await expect.soft(page.getByTestId('thread.find.field'), `${c}: ⌘F's needle`).toHaveValue('cold');
      await expect.soft(page.getByTestId('thread.tools.17'), `${c}: the row he opened`).toHaveAttribute('aria-expanded', 'true');
      await expect.soft(term(page), `${c}: the terminal`).toBeVisible();
      await caretAtEnd(page, field(page), PERF, `${c}, perf's composer`);
      await page.screenshot({ path: test.info().outputPath(`threads-${c.replace(/\W+/g, '-')}.png`) });
      // the other target's draft: acme main's
      await page.getByTestId(`app.side.project.${ACME}`).click();
      await expect.soft(page.getByTestId('thread.screen'), `${c}: acme main's thread`).toHaveAttribute('data-agent', 'main', { timeout: 5000 });
      await expect.soft(field(page), `${c}: acme main's words`).toContainText(MAIN, { timeout: 5000 });
      await page.getByTestId('app.bar.back').click();
      await expect(page.getByTestId('thread.screen')).toHaveAttribute('data-agent', 'perf', { timeout: 5000 });
    },
  });
  try {
    const { page } = s;
    await field(page).click();
    await page.keyboard.press('Enter');
    await expect(field(page), 'a send empties the field').not.toContainText(PERF, { timeout: 5000 });
    await page.waitForTimeout(500);
    // the composer's store sits in the keep as 'ls:bise.composer.v1.<target>' = {draft, history, queue}:
    // the sent words leave the draft (history keeps them for ↑, as the TUI does)
    const drafts: string[] = [];
    for (const body of Object.values(keepFiles(userData))) {
      const rec = JSON.parse(body) as { fields?: Record<string, { text?: string }> };
      for (const [id, f] of Object.entries(rec.fields ?? {})) {
        if (!id.startsWith('ls:bise.composer.v1.') || !id.endsWith('/perf')) continue;
        const draft = (JSON.parse(f.text ?? '{}') as { draft?: string | null }).draft;
        drafts.push(draft ?? '');
      }
    }
    test.info().annotations.push({ type: 'perf draft after the send', description: JSON.stringify(drafts) });
    expect(drafts.join(' '), "perf's kept draft no longer holds the sent words").not.toContain('la moitié');
  } finally {
    await dispose();
  }
});

test('app/keep-all: agents: Tune with a half-typed rename and model id, then the new-agent sheet', async () => {
  test.setTimeout(180_000);
  const NAME = 'perf-hot';
  const MODEL = 'mistral/devstral-med';
  const { dispose } = await cycles({
    route: `agents/${ACME}`,
    setup: async (s) => {
      const { page } = s;
      // the row's '…' opens macOS's own menu (bise fait mac, main/menu-popup.ts Menu.popup): drawn
      // on his screen (the harness rule: a window of the app on screen, 6,34 243x294) and out of
      // Playwright's reach. His pick, Rename…, is made in the menu itself, never drawn.
      await pickInNextMenu(s, 'rename');
      await page.getByTestId('agents.more.perf').click();
      const rename = page.getByTestId('agents.rename.perf');
      await expect(rename, "perf's Tune opens with its name field").toBeVisible({ timeout: 5000 });
      await rename.click();
      await page.keyboard.press('Meta+a');
      await page.keyboard.type(NAME);
      await page.getByTestId('agents.model.perf').click();
      await page.keyboard.press('Meta+a');
      await page.keyboard.type(MODEL);
    },
    check: async ({ page }, c) => {
      await expect.soft(page.getByTestId('agents.screen'), `${c}: the agents screen`).toBeVisible({ timeout: 10_000 });
      await expect.soft(page.getByTestId('agents.rename.perf'), `${c}: Tune open, the rename half typed`).toHaveValue(NAME, { timeout: 5000 });
      await expect.soft(page.getByTestId('agents.model.perf'), `${c}: the model id half typed`).toHaveValue(MODEL);
      if (await page.getByTestId('agents.model.perf').isVisible()) await caretAtEnd(page, page.getByTestId('agents.model.perf'), MODEL, `${c}, the model id`);
      await page.screenshot({ path: test.info().outputPath(`tune-${c.replace(/\W+/g, '-')}.png`) });
    },
  });
  await dispose();

  const BRIEF = 'trouver pourquoi le p99 grimpe à 14 h';
  const NEWNAME = 'p99-hunt';
  const second = await cycles({
    route: `agents/${ACME}`,
    setup: async ({ page }) => {
      await page.getByTestId('agents.new').click();
      await expect(page.getByTestId('agents.new.brief'), 'the new-agent sheet').toBeVisible({ timeout: 5000 });
      await page.getByTestId('agents.new.brief').click();
      await page.keyboard.type(BRIEF);
      await page.getByTestId('agents.new.name').click();
      await page.keyboard.type(NEWNAME);
    },
    check: async ({ page }, c) => {
      await expect.soft(page.getByTestId('agents.newsheet'), `${c}: the sheet open`).toBeVisible({ timeout: 10_000 });
      await expect.soft(page.getByTestId('agents.new.brief'), `${c}: the brief`).toHaveValue(BRIEF, { timeout: 5000 });
      await expect.soft(page.getByTestId('agents.new.name'), `${c}: the name`).toHaveValue(NEWNAME);
      if (await page.getByTestId('agents.new.name').isVisible()) await caretAtEnd(page, page.getByTestId('agents.new.name'), NEWNAME, `${c}, the new name`);
      await page.screenshot({ path: test.info().outputPath(`new-${c.replace(/\W+/g, '-')}.png`) });
    },
  });
  await second.dispose();
});

/**
 * The inbox block open on the first card that takes words (one inbox, 2b): its first row opens it in
 * place, ↓ on the empty field walks past the approvals (options only) until the composer answers.
 */
async function toWords(page: Page): Promise<void> {
  const answering = page.locator('[data-testid="composer"] .lbl', { hasText: /^↳ answering / });
  if ((await page.locator('[data-testid^="inbox.block.open."]').count()) === 0) await page.getByTestId('inbox.block.row.1').click({ timeout: 10_000 });
  await expect(page.locator('[data-testid^="inbox.block.open."]'), 'the block opens in place').toBeVisible({ timeout: 5000 });
  for (let i = 0; i < 6 && !(await answering.isVisible()); i++) {
    await page.locator('[data-testid="composer.field"]').first().click();
    await page.keyboard.press('ArrowDown');
    await page.waitForTimeout(150);
  }
  await expect(answering, 'the composer answers a card').toBeVisible({ timeout: 3000 });
}

test('app/keep-all: ⌘I: a half-typed answer on a card', async () => {
  test.setTimeout(150_000);
  const WORDS = 'plutôt 15 min, mais';
  const field = (page: Page) => page.locator('[data-testid="composer.field"]').first();
  // the block's head ('3 waiting · 2 projects'): after a core restart the rows come back one by one,
  // so the walk waits for every card the setup saw before it starts
  const waiting = async (page: Page) => /(\d+) waiting/.exec((await page.getByTestId('inbox.block').textContent().catch(() => '')) ?? '')?.[1] ?? '';
  let want = '';
  const { dispose } = await cycles({
    route: `agent/${ACME}/main`,
    setup: async ({ page }) => {
      await toWords(page);
      want = await waiting(page);
      await field(page).click();
      await page.keyboard.type(WORDS);
    },
    check: async ({ page }, c) => {
      await expect.soft(page.getByTestId('inbox.block'), `${c}: the inbox block`).toBeVisible({ timeout: 10_000 });
      await expect.poll(() => waiting(page), { timeout: 15_000, message: `${c}: every card back in the block` }).toBe(want);
      await toWords(page);
      await expect.soft(field(page), `${c}: his answer, in the block's composer`).toHaveText(WORDS, { timeout: 5000 });
      if (await field(page).isVisible()) await caretAtEnd(page, field(page), WORDS, `${c}, the answer`);
      await page.screenshot({ path: test.info().outputPath(`inbox-${c.replace(/\W+/g, '-')}.png`) });
    },
  });
  await dispose();
});

test('app/keep-all: spotlight: his half-typed words', async () => {
  test.setTimeout(150_000);
  const WORDS = 'où en est le déploiement';
  const open = async (s: Session, h?: FakeHelper): Promise<Page> => {
    // fn space, from the scripted helper (the one input we script)
    const cap = await capsulePage(s);
    // fn space toggles: only when the spotlight isn't up already
    // its field: the app's composer (PmField testId 'spot.input' draws its editable as
    // 'spot.input.field', 33d67f61d), a contenteditable: his words are its text, not a value
    if (h && !(await cap.getByTestId('spot.input.field').isVisible())) await h.send({ ev: 'fn', act: 'key', key: 'space' });
    return cap;
  };
  const { dispose } = await cycles({
    route: `agent/${ACME}/main`,
    helper: true,
    setup: async (s, h) => {
      const cap = await open(s, h);
      const field = cap.getByTestId('spot.input.field');
      await expect(field, 'the spotlight').toBeVisible({ timeout: 5000 });
      await field.click();
      await cap.keyboard.type(WORDS);
      await cap.waitForTimeout(700);
      await cap.reload();
    },
    check: async (s, c, h) => {
      const cap = await open(s, h);
      await expect.soft(cap.getByTestId('spot.input.field'), `${c}: his words in the spotlight`).toHaveText(WORDS, { timeout: 5000 });
      await cap.screenshot({ path: test.info().outputPath(`spot-${c.replace(/\W+/g, '-')}.png`) });
    },
  });
  await dispose();
});

test('app/keep-all: a key typed in settings is never kept', async () => {
  test.setTimeout(90_000);
  const userData = testUserData();
  const core = new FakeCore();
  const s = await launchDesktop({ core, route: 'settings/models', userData });
  try {
    const { page } = s;
    const add = page.getByTestId('settings.key.add');
    // the key form sits in Setup's 'Accounts and keys' fold, closed unless a model waits for a key
    // (settings/setupPage.tsx; under load that ask comes late): he opens it
    const fold = page.getByTestId('settings.fold.accounts');
    await expect(add.or(page.getByTestId('settings.key.field')).or(fold).first(), 'the key form or its fold').toBeVisible({ timeout: 10_000 });
    if ((await add.or(page.getByTestId('settings.key.field')).count()) === 0 && (await fold.getAttribute('aria-expanded')) !== 'true') await fold.click();
    await expect(add.or(page.getByTestId('settings.key.field')).first(), 'the key form').toBeVisible({ timeout: 10_000 });
    if (await add.isVisible()) await add.click();
    const key = page.getByTestId('settings.key.field');
    await expect(key).toBeVisible({ timeout: 5000 });
    await key.click();
    await page.keyboard.type(FAKE_KEY);
    await page.waitForTimeout(800);
    await page.reload();
    await page.waitForTimeout(800);
    const files = keepFiles(userData);
    test.info().annotations.push({ type: 'keep files', description: Object.keys(files).join(' ') });
    expect(Object.values(files).join('\n'), 'the key is in no keep file').not.toContain(FAKE_KEY.slice(6));
    const shown = await page.getByTestId('settings.key.field').inputValue().catch(() => '');
    expect(shown, 'the key field comes back empty').not.toContain(FAKE_KEY.slice(6));
  } finally {
    await s.close();
    core.dispose();
    rmSync(path.dirname(userData), { recursive: true, force: true });
  }
});
