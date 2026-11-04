// The as-he-does run's window reader (ambient-lead m_18718, m_18946): the promoted app on the
// real-shaped world, read over CDP on 127.0.0.1 only, page events only. Never a native panel:
// settings.add.choose (the folder picker) is never touched.
//   node --experimental-strip-types driver.ts <cdp port> <out dir> <first|second|draft-type|draft-check> [<draft text>]
// first: the open frame, the banner, settings › projects, the add sheet's scan and one add, a
// message to a project's main and its answer. second: the open frame and the banner.
// draft-type: harness' composer gets <draft text>, never sent (ambient-lead m_19525); draft-check,
// after the restart: the same composer holds it again. Each writes <out>/<mode>[-<tag>].json.
// Writes <out>/<mode>.json ({steps: [{name, ok, info}], shots}) and <out>/<mode>-N-*.png.
import { createRequire } from 'node:module';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import type { Browser, ConsoleMessage, Locator, Page, Request, Response } from '@playwright/test';
import { fxDir, named } from './fx.ts';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const DESKTOP = process.env.PW_FROM ?? path.join(HERE, '..', '..', '..', '..', 'apps', 'desktop');
const { chromium }: typeof import('@playwright/test') = createRequire(path.join(DESKTOP, 'package.json'))('@playwright/test');
const [port, out = '.', mode = '', draft] = process.argv.slice(2);
// the files its words carry: never under a path that names a world project (fx.ts)
const FX = fxDir(out, process.env);
fs.mkdirSync(FX, { recursive: true });
const tag = process.env.AS_HE_DOES_TAG ? `-${process.env.AS_HE_DOES_TAG}` : '';
const WATCHDOG_MS = Number(process.env.AS_HE_DOES_WATCHDOG_MS ?? 600000);
// AS_HE_DOES_QUICK_ROWS=answer: a quick pass of those rows alone (rowsE812's and rowsF2A7's rows
// whose name holds one of the words), to prove or clear a red on a fresh world (ambient-lead m_23521)
// AS_HE_DOES_ROWS_FILE (fast-loop, scripts/desktop_rerun.py): one row name a line, as reds.txt says
// them (digits as N, commas kept): a row runs when its name holds one, digits as N on both sides.
// AS_HE_DOES_GROUPS=rowsF2A7: those row groups alone, seed included (a step's `group` says which
// group it ran in).
const ONLY = [...(process.env.AS_HE_DOES_QUICK_ROWS ?? '').split(','),
  ...(process.env.AS_HE_DOES_ROWS_FILE ? fs.readFileSync(process.env.AS_HE_DOES_ROWS_FILE, 'utf8').split('\n') : [])]
  .map((w) => w.trim()).filter(Boolean);
const asN = (s: string) => s.replace(/\d+/g, 'N');
const wanted = (name: string) => ONLY.length === 0 || ONLY.some((w) => name.includes(w) || asN(name).includes(asN(w)));
const GROUPS = (process.env.AS_HE_DOES_GROUPS ?? '').split(',').map((w) => w.trim()).filter(Boolean);
let GROUP: string | null = null;
// the time a step spent polling (`until`) since the step before (fast-loop): each step's `wait_s`
// says how much of its at_s gap was a wait for the app or the hub, not the step's own work
let WAITED_MS = 0;
const ANSWER_MS = Number(process.env.AS_HE_DOES_ANSWER_MS ?? 240000);
const MESSAGE = 'hello from the as-he-does proof';

/** One row of the pass: its name, green or not, what it saw. */
type Step = { name: string; ok: unknown; info: unknown; at_s: number; wait_s: number; group?: string };
// the pass's JSON: its rows, its shots, and what a mode adds (pages, ...)
const res: { mode: string; steps: Step[]; shots: string[]; [k: string]: unknown } = { mode, steps: [], shots: [] };
const save = () => fs.writeFileSync(path.join(out, `${mode}${tag}.json`), JSON.stringify(res, null, 1));
// at_s: seconds from the driver's start, so a pass shows where its time went
const T0 = Date.now();
let lastStep: { name: string | null; at: number } = { name: null, at: T0 };
const step = (name: string, ok: unknown, info?: unknown) => {
  res.steps.push({ name, ok, info: info ?? null, at_s: Math.round((Date.now() - T0) / 1000), wait_s: Math.round(WAITED_MS / 1000), ...(GROUP ? { group: GROUP } : {}) });
  WAITED_MS = 0;
  lastStep = { name, at: Date.now() };
  save();
};
// two watchdogs (gate-nightly, fast-try p7). The stall one: no step for STALL_MS (600 s, today's
// whole-pass limit) is a hang, red. The pass one: WATCHDOG_MS, or for the quick pass 600 s per row
// group it runs (sized below, where the groups are listed). Before, one 600 s limit for the whole
// pass ended every quick pass red in its last rows (cand-*q 2026-10-14..17: 22 steps, then 'driver
// watchdog'), and the rows past it never ran.
const STALL_MS = Number(process.env.AS_HE_DOES_STALL_MS ?? 600000);
// pids a row holds with SIGSTOP (rowsF2A7): always SIGCONT'd, also when the pass ends early
const HELD = new Set<number>();
const unhold = () => { for (const p of HELD) { try { process.kill(p, 'SIGCONT'); } catch { /* gone */ } } HELD.clear(); };
const watchdog = (why: unknown) => { unhold(); step('driver watchdog', false, why); process.exit(2); };
let passTimer: NodeJS.Timeout | null = null;
const passLimit = (ms: number) => {
  if (passTimer) clearTimeout(passTimer);
  passTimer = setTimeout(() => watchdog({ limit_s: ms / 1000, last: lastStep.name }), ms);
  passTimer.unref();
};
passLimit(WATCHDOG_MS);
setInterval(() => {
  const idle = Date.now() - lastStep.at;
  if (idle > STALL_MS) watchdog({ stalled_s: Math.round(idle / 1000), after: lastStep.name });
}, 5000).unref();

// the app's CDP port answers some seconds after the open (c28260a9's first run: 3 drives refused at
// once, ECONNREFUSED): retried for up to 90 s, then a failed step, never an uncaught error
let connected: Browser | null = null;
for (const t0 = Date.now(); !connected;) {
  connected = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 10000 }).catch((e: unknown) => {
    if (Date.now() - t0 > 90000) { step('connect to the app (CDP)', false, String(e).slice(0, 300)); process.exit(1); }
    return null;
  });
  if (!connected) await new Promise((r) => setTimeout(r, 1000));
}
const browser: Browser = connected;
let found: Page | undefined;
for (let i = 0; i < 100 && !found; i++) {
  for (const c of browser.contexts()) for (const p of c.pages()) {
    if (await p.locator('[data-testid="app.root"]').count().catch(() => 0)) found = p;
  }
  if (!found) await new Promise((r) => setTimeout(r, 200));
}
res.pages = browser.contexts().flatMap((c) => c.pages().map((p) => p.url()));
if (!found) { step('the window page', false, res.pages); process.exit(1); }
const page: Page = found;

page.on('close', () => closed());
// a renderer gone is said apart from a window closed (try-gate-f2a7's first run: 'the page closed'
// 9 s after two cards came, no crash report): its time and what went
page.on('crash', () => step('the window renderer did not crash', false, { at: new Date().toISOString() }));
browser.on('disconnected', () => { if (!ending) step('the app kept its CDP connection', false, { at: new Date().toISOString() }); });
let n = 0;
const shot = async (name: string) => {
  const f = path.join(out, `${mode}${tag}-${++n}-${name}.png`);
  try { await page.screenshot({ path: f, timeout: 15000 }); }
  catch {
    try {
      const cdp = await page.context().newCDPSession(page);
      const r = await cdp.send('Page.captureScreenshot', { format: 'png' });
      fs.writeFileSync(f, Buffer.from(r.data, 'base64'));
    } catch (e) { step(`frame ${name}`, false, String(e).slice(0, 200)); return; }
  }
  res.shots.push(f); save();
};
const text = (sel: string) => page.locator(sel).first().innerText({ timeout: 3000 }).catch(() => null);
// the right bar's pager (fa548fa11, designer m_27020): 'show N more' under 50, 'show 50 more · N
// left' past it; '+ N more' is the inbox strip's only (stripRows.ts)
const BAR_MORE = /^show \d+ more\b/;
const click = async (name: string, loc: Locator) => {
  try { await loc.click({ timeout: 8000 }); return true; }
  catch (e) { step(name, false, String(e).slice(0, 240)); return false; }
};
// a project's sidebar row by its name (.nm, nm-frame): the row's whole text runs on into its pill
// and its ⌘ digit ('harness⌘11 new artifact'), so never getByText on the row
const projectRow = (name: string) => page.locator('[data-testid^="app.side.project."]')
  .filter({ has: page.locator('.nm', { hasText: new RegExp(`^${name}$`) }) }).first();
// open a project's main as he does: its sidebar row. On a library page (artifacts) the sidebar
// shows the library and '← Projects' (app.side.back) first: back to the projects, then the row
const openProject = async (name: string, label = `open ${name}`) => {
  const row = projectRow(name);
  if (!(await row.isVisible().catch(() => false))) {
    await page.locator('[data-testid="app.side.back"]').first().click({ timeout: 3000 })
      .catch(() => page.locator('[data-testid="app.rail.bise"]').first().click({ timeout: 3000 }).catch(() => {}));
    await row.waitFor({ timeout: 5000 }).catch(() => {});
  }
  return click(label, row);
};
// an open artifact's ✕: its panel tab's close since native-mac (app.panel.tab.artifact:<p>/<id>.close,
// 'Close tab · ⌘W'), the artifact head's own before
const ART_CLOSE = '[data-testid="artifact.close"], [data-testid^="app.panel.tab.artifact:"][data-testid$=".close"]';
// the project's shell: a panel tab since native-mac, the bottom panel before
const TERM_SEL = '[data-testid="terminal.panel"], [data-testid^="app.panel.body.term:"]';
// bise's row (⌘0), the same way: from a library page, '← Projects' first
const openBise = async () => {
  const row = page.locator('[data-testid="app.side.bise"]').first();
  if (!(await row.isVisible().catch(() => false))) {
    await page.locator('[data-testid="app.side.back"]').first().click({ timeout: 3000 }).catch(() => {});
    await row.waitFor({ timeout: 5000 }).catch(() => {});
  }
  return click('bise', row);
};
// the window can go away under a pass (7e691aef's rerun: the candidate quit at 07:48:09, rows2 then
// crashed in page.waitForTimeout): say it as a red step and end the pass, never an uncaught error
let ending = false;
const closed = () => {
  if (ending) return;
  unhold();
  step('the window stayed open through the pass', false, { what: 'the page closed', at: new Date().toISOString() });
  process.exit(3);
};
const until = async <T,>(fn: () => Promise<T>, ms: number, every = 1000): Promise<T | null> => {
  const t0 = Date.now();
  try {
    for (;;) {
      if (page.isClosed()) closed();
      const v = await fn().catch(() => null);
      if (v) return v;
      if (Date.now() - t0 > ms) return null;
      await new Promise((r) => setTimeout(r, every));
    }
  } finally {
    WAITED_MS += Date.now() - t0;
  }
};
// the alarm rows of the right bar's SCHEDULED & WATCHING whose words hold `words` (agent-waiting-states
// A: app.right.watching.<agent>.<id>, never its .detail/.stop/.open parts): {id, text, title, g, sub, end}
const alarmRows = (words: string) => page.evaluate((w) => [...document.querySelectorAll('[data-testid^="app.right.watching."]')]
  .filter((e) => /\.\d+$/.test(e.getAttribute('data-testid') ?? '') && (e.textContent ?? '').includes(w))
  // its hover words: the ds tip layer's data-tip since hover-tips (984699494), no native title
  .map((e) => ({ id: e.getAttribute('data-testid'), text: (e.textContent ?? '').replace(/\s+/g, ' ').trim(), title: e.getAttribute('data-tip') ?? e.getAttribute('title'),
    g: e.querySelector('.app-rb-g')?.textContent?.trim() ?? null, sub: e.querySelector('.app-rb-sub')?.textContent?.trim() ?? null,
    end: e.querySelector('.app-rb-end, .app-rb-time')?.textContent?.trim() ?? null })), words).catch(() => []);

await page.locator('[data-testid="app.sidebar"]').first().waitFor({ timeout: 20000 }).catch(() => {});
// the status pill (e41cd3c2): 'starting bise…' and the core's words sit IN the title bar, no row
// under it, nothing below moves when it shows or goes. 5 s of samples from the window's first draw.
{
  const seen: { samples: number; pill: number; outside: number; words: string[]; mainTops: number[] } = { samples: 0, pill: 0, outside: 0, words: [], mainTops: [] };
  for (let i = 0; i < 20; i++) {
    const s = await page.evaluate(() => {
      const r = (sel: string) => document.querySelector(sel)?.getBoundingClientRect() ?? null;
      const p = r('[data-testid="app.core"]');
      // the title bar: app.bar since native-mac (nm-frame), its toggles before
      const bar = r('[data-testid="app.bar"]') ?? r('[data-testid="app.title.panel"]') ?? r('[data-testid="app.title.sidebar"]');
      const m = r('[data-testid="app.main"]');
      return { p: p && p.width > 0 ? { top: p.top, bottom: p.bottom } : null, barBottom: bar ? bar.bottom : null, mainTop: m ? Math.round(m.top) : null,
        words: document.querySelector('[data-testid="app.core"]')?.textContent?.trim().slice(0, 80) ?? null };
    }).catch(() => null);
    if (s) {
      seen.samples++;
      if (s.mainTop !== null && !seen.mainTops.includes(s.mainTop)) seen.mainTops.push(s.mainTop);
      if (s.p) {
        seen.pill++;
        if (s.words && !seen.words.includes(s.words)) seen.words.push(s.words);
        if (s.barBottom !== null && s.p.top > s.barBottom + 4) seen.outside++;
      }
    }
    if (i === 1) await shot('launch-pill');
    await new Promise((r) => setTimeout(r, 250));
  }
  step('the status pill sits in the title bar; nothing moves under it (5 s from first draw)', seen.outside === 0 && seen.mainTops.length <= 1, seen);
}
await shot('open');
const upd = await text('[data-testid="app.update.text"]');
step("no 'didn't start right' banner", !(upd ?? '').includes("didn't start right"), upd);
step('core line', true, await text('[data-testid="app.core"]'));
const side = await text('[data-testid="app.sidebar"]');
step('sidebar', !!side, side);

// the window's frame (amb-mac 9f7c2f72, ambient-lead m_21559): 1280x720 on first open, clamped to the
// work area minus 24 px a side, kept per display in userData/window-frame.json. An off-screen run
// sees only the off-screen display (windows.ts OFF_SCREEN, 1920x1200 far from his), so it never
// lands on his screen. The bounds come from the page (outer size) and the kept file.
const FRAMES = process.env.AS_HE_DOES_USERDATA ? path.join(process.env.AS_HE_DOES_USERDATA, 'window-frame.json') : null;
const winBox = () => page.evaluate(() => ({ x: window.screenX, y: window.screenY, w: window.outerWidth, h: window.outerHeight, iw: window.innerWidth, ih: window.innerHeight, aw: window.screen.availWidth, ah: window.screen.availHeight })).catch(() => null);
// userData/window-frame.json as the app keeps it (JSON): read by the frame rows
const keptFrames = (): unknown => { try { return JSON.parse(fs.readFileSync(FRAMES ?? '', 'utf8')); } catch { return null; } };
// resize from CDP: the browser's Browser.setWindowBounds when Electron answers it, else window.resizeTo
const resizeWin = async (width: number, height: number) => {
  const how = await (async () => {
    const b = await browser.newBrowserCDPSession().catch(() => null);
    if (!b) return null;
    const t = await page.context().newCDPSession(page).then((s) => s.send('Target.getTargetInfo')).catch(() => null);
    const w = await b.send('Browser.getWindowForTarget', t?.targetInfo?.targetId ? { targetId: t.targetInfo.targetId } : {}).catch(() => null);
    if (!w) return null;
    return b.send('Browser.setWindowBounds', { windowId: w.windowId, bounds: { width, height } }).then(() => 'Browser.setWindowBounds', () => null);
  })();
  if (how) return how;
  await page.evaluate(([w, h]) => window.resizeTo(w, h), [width, height]).catch(() => {});
  return 'window.resizeTo';
};
if (mode === 'first') {
  const box = await winBox();
  const wantW = box && box.aw > 0 ? Math.min(1280, box.aw - 48) : 1280;
  const wantH = box && box.ah > 0 ? Math.min(720, box.ah - 48) : 720;
  await shot('window-1280');
  step('the first launch opens at 1280x720, clamped to the work area (a frame for designer)', !!box && Math.abs(box.w - wantW) <= 2 && Math.abs(box.h - wantH) <= 2, { box, want: { w: wantW, h: wantH } });
}
if (mode === 'draft-type' && !tag.endsWith('atonce')) {
  // resized now; the restart that follows must bring this frame back (checked in draft-check)
  const before = await winBox();
  const how = await resizeWin(1180, 760);
  const after = await until(async () => { const b = await winBox(); return b && b.w !== before?.w ? b : null; }, 4000, 300);
  await page.waitForTimeout(800); // windows.ts keeps it 300 ms after the resize settles
  const kept = keptFrames();
  if (after) fs.writeFileSync(path.join(out, 'win-resized.json'), JSON.stringify({ w: after.w, h: after.h }));
  await shot('resized-window');
  step('the window resizes from CDP and its frame is kept (window-frame.json)', !!after && !!kept, { how, before, after, kept });
}
if (mode === 'draft-check' && fs.existsSync(path.join(out, 'win-resized.json')) && !tag.endsWith('atonce')) {
  const want = JSON.parse(fs.readFileSync(path.join(out, 'win-resized.json'), 'utf8'));
  const box = await until(async () => { const b = await winBox(); return b && Math.abs(b.w - want.w) <= 2 && Math.abs(b.h - want.h) <= 2 ? b : null; }, 5000, 300);
  await shot('frame-back');
  step('after a resize and a restart, the window comes back at its kept frame', !!box, { want, now: box ?? (await winBox()), kept: keptFrames() });
}

if (mode === 'first') {
  // the sidebar's '+ add a project' opens amb-home's add sheet in place (amb-win a83184e3), as he
  // does; when it doesn't, settings › projects and its own 'add a project'
  const sheet = page.locator('[data-testid="settings.add"]').first();
  let opened = await click('add a project (sidebar)', page.locator('[data-testid="app.side.add"]').first());
  let shown = opened && await sheet.waitFor({ timeout: 10000 }).then(() => true, () => false);
  if (!shown) {
    await click('open settings', page.locator('[data-testid="app.rail.settings"]').first());
    const root = page.locator('[data-testid="settings.root"]').first();
    await root.waitFor({ timeout: 8000 }).catch(() => {});
    await click('settings › projects', root.getByText('projects', { exact: true }).first());
    await page.locator('[data-testid="settings.projects"]').first().waitFor({ timeout: 8000 }).catch(() => {});
    await shot('settings-projects');
    opened = await click('add a project (settings)', page.locator('[data-testid="settings.project.add"]').first());
    shown = opened && await sheet.waitFor({ timeout: 10000 }).then(() => true, () => false);
  }
  step('the add sheet opens from the sidebar', shown, null);
  step('the add sheet opens', shown, await text('[data-testid="settings.add"]'));
  // the scan: its found list with a count (never the folder picker)
  const found = page.locator('[data-testid="settings.add.found"]').first();
  const ft = shown ? await until(async () => {
    const t = await found.innerText({ timeout: 1000 });
    return t && /\d/.test(t) && !/scanning|looking/i.test(t) ? t : null;
  }, 120000) : null;
  await shot('scan');
  step('the scan answers with its count', !!ft, (ft ?? (await text('[data-testid="settings.add"]')) ?? '').slice(0, 800));
  // designer's final sign (ambient-lead m_19035): the sheet open after the scan answered, light
  if (ft) {
    // back to the scheme the page had, said explicitly: emulateMedia(null) can leave it on the other one
    const was = await page.evaluate(() => (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')).catch(() => null);
    await page.emulateMedia({ colorScheme: 'light' }).catch(() => {});
    await page.waitForTimeout(400);
    await shot('add-sheet-light');
    if (was !== 'light') await page.emulateMedia({ colorScheme: was ?? null }).catch(() => {});
  }
  // one found repo, then add. a found row is `settings.add.found.<path>` (the Group's row, its text does
  // nothing); its tick box is the role=checkbox `settings.add.found.<name>` inside it (AddSheet.tsx)
  const boxes = page.locator('[role="checkbox"][data-testid^="settings.add.found."]');
  const count = await boxes.count().catch(() => 0);
  let picked = null;
  let ticked = false;
  if (count > 0) {
    const box = boxes.first();
    picked = await box.getAttribute('data-testid').catch(() => null);
    await click('tick a found repo', box);
    ticked = (await until(async () => (await box.getAttribute('aria-checked').catch(() => null)) === 'true' || null, 5000, 250)) === true;
  }
  const go = page.locator('[data-testid="settings.add.go"]').first();
  const goOn = await until(async () => (await go.isEnabled({ timeout: 1000 }).catch(() => false)) || null, 5000, 250) === true;
  const say = async () => await text('[data-testid="settings.add.say"]');
  if (goOn) await click('add', go);
  const grew = goOn ? await until(async () => {
    const s = await text('[data-testid="app.sidebar"]');
    return s && s !== side ? s : null;
  }, 60000) : null;
  const why = grew ? null : await say();
  await shot('added');
  step("'add a project' adds one (the sidebar shows it) or says why", goOn ? !!grew || !!why : false,
    { found_rows: count, picked, ticked, go_enabled: goOn, sidebar_after: grew, said: why, sheet: await text('[data-testid="settings.add"]') });
  // the sheet must not stay over the window: an open sheet takes the clicks the message step needs
  const still = await page.locator('[data-testid="settings.add"]').first().isVisible().catch(() => false);
  if (still) {
    await click('cancel the add sheet', page.locator('[data-testid="settings.add.cancel"]').first());
    await page.locator('[data-testid="settings.add"]').first().waitFor({ state: 'hidden', timeout: 5000 }).catch(() => {});
  }
}
if (mode === 'smoke') {
  // cand.sh --smoke (fast-try p3): the core comes up. CoreLine.tsx shows nothing once the core is
  // up (its 'starting bise…' goes) and a ▲ line when it can't start or can start no hub
  const up = await until(async () => ((await page.locator('[data-testid="app.core"]').count()) === 0 ? 'gone' : null), 60000, 500);
  step("the core comes up: the core line goes, no ▲ line", !!up, up ?? await text('[data-testid="app.core"]'));
}
if (mode === 'first' || mode === 'smoke') {
  // a message to a project's main, and its answer
  await openProject('harness');
  const composer = page.locator('[data-testid="composer"] [contenteditable="true"], [data-testid="composer"] textarea').first();
  const typed = await click('the composer', composer);
  if (typed) {
    await page.keyboard.type(MESSAGE, { delay: 15 });
    await page.keyboard.press('Enter');
  }
  // the answer: a thread row (thread.entry.<pos>, rows.tsx) after his row with the message, whose
  // kind isn't 'you' and that has words (a tools or thinking row alone isn't an answer). Never the
  // text after the message in the page: his row's own '11:47 ✓✓' read as an answer in run 3
  const answer = typed ? await until(() => page.evaluate((msg) => {
    const rows = [...document.querySelectorAll('[data-testid^="thread.entry."]')];
    const mine = rows.findIndex((r) => r.getAttribute('data-kind') === 'you' && (r.textContent ?? '').includes(msg));
    if (mine < 0) return null;
    for (const r of rows.slice(mine + 1)) {
      const kind = r.getAttribute('data-kind') ?? '';
      const words = (r.textContent ?? '').trim();
      if (kind !== 'you' && kind !== 'tools' && kind !== 'thinking' && words.length > 0) return `${kind}: ${words.slice(0, 400)}`;
    }
    return null;
  }, MESSAGE), ANSWER_MS, 2000) : null;
  await shot('message');
  step("a message to a project's main gets its answer", !!answer, answer ?? (await text('[data-testid="app.main"]'))?.slice(-800));
}
// this cut's rows (ambient-lead m_20075), seeded by one message to harness' main (the fake provider
// runs its [[bash: …]] calls): a turn of several runs, an agent kept and one dropped, a local .html
// artifact. Then: the runs fold, /agents lists the kept one and not the dropped, /archived lists the
// dropped, the artifact's frame has its pixels, the sidebar resizes by drag (its width kept for the
// restart's check), ctrl+r from the thread starts dictation's line (no mic under the knob), esc ends it.
// an open artifact's 3-column view folds the left bar under 1200 px: close it before the next pass
const closeArtifact = async () => {
  const pg = page.locator('[data-testid="artifact.page"]').first();
  if (!(await pg.count().catch(() => 0))) return;
  await page.locator(ART_CLOSE).first().click({ timeout: 3000 }).catch(() => page.keyboard.press('Escape'));
  await pg.waitFor({ state: 'detached', timeout: 5000 }).catch(() => {});
};

if (mode === 'cut') {
  const composerOf = () => page.locator('[data-testid="composer"] [contenteditable="true"], [data-testid="composer"] textarea').first();
  const say = async (words: string) => {
    if (!(await click(`say ${words.slice(0, 30)}`, composerOf()))) return false;
    // b51422c0: a '/diff' left in the composer took the next message ('/diff[[bash: …'): start empty
    await page.keyboard.press('Meta+a');
    await page.keyboard.press('Backspace');
    const parts = words.split('\n');
    for (let i = 0; i < parts.length; i++) {
      if (i) await page.keyboard.press('Shift+Enter');
      await page.keyboard.type(parts[i], { delay: 5 });
    }
    await page.keyboard.press('Enter');
    // a slash command with args (/diff) spends the first Enter on the slash menu: send it with a second
    if (words.startsWith('/')) {
      await page.waitForTimeout(400);
      const left = ((await composerOf().innerText().catch(() => '')) ?? '').trim();
      if (left.startsWith('/')) await page.keyboard.press('Enter');
    }
    return true;
  };
  await openProject('harness');
  const html = "<html><body style='margin:0;background:#2e8b57'><h1 style='color:#fff;font:40px sans-serif'>proof html</h1></body></html>";
  const seed = [
    'set up the proof rows',
    '[[bash: echo one]]', '[[bash: echo two]]', '[[bash: echo three]]',
    "[[bash: sb spawn keeper --objective 'hold the proof']]",
    "[[bash: sb spawn gone --objective 'an old one']]",
    `[[bash: printf "%s" "${html.replaceAll('"', "'")}" > proof.html && sb artifact add proof.html --title proof-html]]`,
  ].join(' ');
  const seeded = await say(seed);
  const done = seeded ? await until(() => page.evaluate(() =>
    [...document.querySelectorAll('[data-testid^="thread.entry."]')].some((r) => r.getAttribute('data-kind') === 'artifact') || null), ANSWER_MS, 2000) : null;
  await shot('seeded');
  const kinds = await page.evaluate(() => [...document.querySelectorAll('[data-testid^="thread.entry."]')].map((r) => r.getAttribute('data-kind')));
  // 'gone' is dropped in a LATER turn, once it is idle (amb-home m_20141: a drop in the turn that
  // spawned it makes the hub ask a 'drop' card instead of archiving); a drop card that still comes
  // is answered yes, which also proves the card path (ambient-lead m_20143)
  const goneIdle = await until(async () => {
    const s = (await text('[data-testid="app.sidebar"]')) ?? '';
    const i = s.indexOf('gone');
    return i >= 0 && /idle|done/.test(s.slice(i, i + 40)) ? true : null;
  }, ANSWER_MS, 2000);
  // its idle state is only noted: a drop that archives anyway (or asks its card) is the proof
  await say('[[bash: sb drop gone]]');
  const card = page.locator('[data-testid^="dock.opt"]').filter({ hasText: /yes|drop/i }).first();
  const asked = await card.waitFor({ timeout: 20000 }).then(() => true, () => false);
  let answered = null;
  if (asked) {
    await shot('drop-card');
    answered = await click("answer the drop card 'yes'", card);
    await page.locator('[data-testid^="dock.opt"]').first().waitFor({ state: 'detached', timeout: 20000 }).catch(() => {});
  }
  const dropped = await until(async () => {
    const s = (await text('[data-testid="app.sidebar"]')) ?? '';
    return /\bgone\b/.test(s) ? null : true;
  }, 60000, 1000);
  step("'gone' dropped (a drop card answered yes when asked)", !!dropped, { idle_first: !!goneIdle, card: asked, answered });
  // a thought of its own message (the fake provider's [[think: …]]): opened, it must never draw its
  // signature (BENDSIG::…, c28260a9's window guard)
  await say('think it over [[think: weighing the proof rows]]');
  const thoughts = page.locator('[data-testid^="thread.entry."]').filter({ has: page.locator('[data-testid^="thread.think."]') });
  const nThoughts = (await until(async () => ((await thoughts.count()) > 0 ? await thoughts.count() : null), ANSWER_MS, 1000)) ?? 0;
  for (let i = 0; i < nThoughts; i++) {
    const t = thoughts.nth(i);
    await t.getByText(/thought for/).first().click({ timeout: 3000 }).catch(() => t.click({ timeout: 3000 }).catch(() => {}));
  }
  if (nThoughts) await page.waitForTimeout(800);
  const thoughtText = nThoughts ? await thoughts.evaluateAll((rs) => rs.map((r) => r.textContent ?? '').join(' --- ')) : '';
  const mainText = (await text('[data-testid="app.main"]')) ?? '';
  await shot('thought-open');
  // the TUI folds a thought too ('∴ thought for 0.0s ▸', words on open): the row is there and opens to its words
  step('a short thought: its row is there and opens to its words', nThoughts > 0 && /weighing the proof rows/.test(thoughtText), { thoughts: nThoughts, text: thoughtText.slice(0, 200) });
  step('no BENDSIG text in an opened thought', nThoughts > 0 && !/BENDSIG/.test(thoughtText) && !/BENDSIG/.test(mainText), { thoughts: nThoughts, text: thoughtText.slice(0, 300) });
  step('the seed ran (runs, agents, artifact)', !!done, { kinds });
  step('the runs fold (tools rows, not one row per call)', kinds.filter((k) => k === 'tools').length >= 1 && kinds.filter((k) => k === 'tools').length < 6, kinds);
  // /agents and /archived
  const lists: [string, string, string | null][] = [['/agents', 'keeper', 'gone'], ['/archived', 'gone', null]];
  for (const [cmd, want, not] of lists) {
    await openProject('harness', 'back to harness');
    await say(cmd);
    const t = await until(async () => { const s = await text('[data-testid="app.main"]'); return s && s.includes(want) ? s : null; }, 20000, 1000);
    await shot(cmd.slice(1));
    const body = t ?? (await text('[data-testid="app.main"]')) ?? '';
    step(`${cmd} lists ${want}${not ? ` and not ${not}` : ''}`, !!t && (!not || !new RegExp(`\\b${not}\\b`).test(body)), body.slice(0, 500));
    await page.keyboard.press('Escape').catch(() => {});
  }
  // the local .html artifact's pixels, and what its frame loads (ambient-lead m_20609: for amb-web +
  // task-panel when it is blank): every request, response, failure and console line from here
  const net: Record<string, unknown>[] = [];
  const onReq = (r: Request) => net.push({ ev: 'request', url: r.url().slice(0, 300), type: r.resourceType(), frame: r.frame()?.url().slice(0, 200) });
  const onRes = (r: Response) => net.push({ ev: 'response', url: r.url().slice(0, 300), status: r.status(), type: r.headers()['content-type'] ?? '' });
  const onFail = (r: Request) => net.push({ ev: 'failed', url: r.url().slice(0, 300), error: r.failure()?.errorText ?? '' });
  const onCon = (m: ConsoleMessage) => net.push({ ev: 'console', type: m.type(), text: m.text().slice(0, 300), url: m.location()?.url?.slice(0, 200) });
  const onPerr = (e: Error) => net.push({ ev: 'pageerror', text: String(e).slice(0, 300) });
  page.on('request', onReq); page.on('response', onRes); page.on('requestfailed', onFail); page.on('console', onCon); page.on('pageerror', onPerr);
  await click('artifacts', page.locator('[data-testid="app.rail.artifacts"]').first());
  // the library page (nm-pages): its Recent row's card opens the artifact at a click (Recent.tsx),
  // else a card or row of its grid (a click picks, ⏎ opens)
  await page.locator('[data-testid="artifacts.screen"]').first().waitFor({ timeout: 8000 }).catch(() => {});
  const recent = page.locator('[data-testid^="artifacts.recent.item."]').filter({ hasText: 'proof-html' }).first();
  const item = (await recent.isVisible().catch(() => false)) ? recent
    : page.locator('[data-testid="artifacts.screen"] [data-row]').filter({ hasText: 'proof-html' }).first();
  const opened = await click('select proof-html', item);
  if (opened && !(await page.locator('[data-testid="artifact.page"]').first().waitFor({ timeout: 3000 }).then(() => true, () => false))) await page.keyboard.press('Enter');
  const frame = page.locator('[data-testid="artifact.page"] iframe, iframe').first();
  const fr = opened ? await frame.waitFor({ timeout: 15000 }).then(() => true, () => false) : false;
  let green = 0;
  if (fr) {
    await page.waitForTimeout(1500);
    const png = (await frame.screenshot({ timeout: 15000 }).catch(() => null))?.toString('base64');
    green = png ? await page.evaluate(async (b64) => {
      const img = new Image(); img.src = `data:image/png;base64,${b64}`; await img.decode();
      const c = document.createElement('canvas'); c.width = img.width; c.height = img.height;
      const g = c.getContext('2d'); if (!g) return 0; g.drawImage(img, 0, 0);
      const d = g.getImageData(0, 0, c.width, c.height).data; let n = 0;
      for (let i = 0; i < d.length; i += 4) if (Math.abs(d[i] - 46) < 12 && Math.abs(d[i + 1] - 139) < 12 && Math.abs(d[i + 2] - 87) < 12) n++;
      return n / (d.length / 4);
    }, png) : 0;
  }
  const frames = await page.evaluate(() => [...document.querySelectorAll('iframe, webview, object, embed')].map((f) => ({
    tag: f.tagName.toLowerCase(), src: f.getAttribute('src'), srcdoc: (f.getAttribute('srcdoc') ?? '').slice(0, 200), sandbox: f.getAttribute('sandbox'),
    w: f.clientWidth, h: f.clientHeight })));
  const docs = [];
  for (const f of page.frames()) {
    if (f === page.mainFrame()) continue;
    docs.push({ url: f.url().slice(0, 300), body: (await f.evaluate(() => document.documentElement?.outerHTML?.slice(0, 300) ?? '').catch((e) => `unreadable: ${String(e).slice(0, 120)}`)) });
  }
  page.off('request', onReq); page.off('response', onRes); page.off('requestfailed', onFail); page.off('console', onCon); page.off('pageerror', onPerr);
  fs.writeFileSync(path.join(out, `${mode}-artifact-frame.json`), JSON.stringify({ frames, docs, net }, null, 1));
  await shot('artifact-html');
  step('a local .html artifact renders (its green pixels)', green > 0.3, { opened, frame: fr, green_share: Math.round(green * 100) / 100, frames, docs: docs.slice(0, 3), net_lines: net.length });
  // the sidebar resizes by drag; its width is kept for the restart's check. The open artifact folds
  // the left bar to a rail (no split, no 'harness' label): close it first (run 40fc444d's 2 reds)
  await closeArtifact();
  const split = page.locator('[data-testid="app.split.side"]').first();
  const w0 = Number(await split.getAttribute('aria-valuenow').catch(() => null));
  const box = await split.boundingBox().catch(() => null);
  if (box) {
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.move(box.x + box.width / 2 + 60, box.y + box.height / 2, { steps: 8 });
    await page.mouse.up();
  }
  const w1 = Number(await split.getAttribute('aria-valuenow').catch(() => null));
  fs.writeFileSync(path.join(out, 'side-width.txt'), String(w1));
  await shot('resized');
  step('the sidebar resizes by drag', !!box && w1 > w0, { before: w0, after: w1 });
  // ctrl+r from the thread (not the field): dictation's line shows, esc ends it
  await openProject('harness', 'back to harness');
  await page.locator('[data-testid="app.main"]').first().click({ position: { x: 40, y: 140 } }).catch(() => {});
  const before = (await text('[data-testid="composer"]')) ?? '';
  await page.keyboard.press('Control+r');
  const line = await until(async () => { const t = (await text('[data-testid="composer"]')) ?? ''; return t !== before && /dictat|listen|mic|speak|voice|any key stop|esc cancel/i.test(t) ? t : null; }, 8000, 500);
  // (with AS_HE_DOES_FAKE_VOICE=1 the take really records: the composer is the recording pill, '0:03',
  // 'any key stop', 'esc cancel', none of the no-mic words: try-gate-0170's full pass)
  await shot('ctrl-r');
  await page.keyboard.press('Escape');
  const after = (await text('[data-testid="composer"]')) ?? '';
  step('ctrl+r from the thread starts dictation; esc ends it', !!line && !/listening/i.test(after), { line: line?.slice(0, 300), after_esc: after.slice(0, 200) });
  await closeArtifact();
}

// the first cut from main (ambient-lead m_20586): the right bar, an artifact opened from it, back /
// forward, the working row, the inbox block with another project's item, a short thought's words
if (mode === 'rows2') {
  const composerOf = () => page.locator('[data-testid="composer"] [contenteditable="true"], [data-testid="composer"] textarea').first();
  const say = async (words: string) => {
    if (!(await click(`say ${words.slice(0, 30)}`, composerOf()))) return false;
    // b51422c0: a '/diff' left in the composer took the next message ('/diff[[bash: …'): start empty
    await page.keyboard.press('Meta+a');
    await page.keyboard.press('Backspace');
    const parts = words.split('\n');
    for (let i = 0; i < parts.length; i++) {
      if (i) await page.keyboard.press('Shift+Enter');
      await page.keyboard.type(parts[i], { delay: 5 });
    }
    await page.keyboard.press('Enter');
    // a slash command with args (/diff) spends the first Enter on the slash menu: send it with a second
    if (words.startsWith('/')) {
      await page.waitForTimeout(400);
      const left = ((await composerOf().innerText().catch(() => '')) ?? '').trim();
      if (left.startsWith('/')) await page.keyboard.press('Enter');
    }
    return true;
  };
  const harness = () => openProject('harness');
  await closeArtifact();
  // one waiting item in another project: ai-docs' main opens a card for him (for bise's inbox block)
  await openProject('ai-docs');
  await say("[[bash: sb notify 'publish the ai-docs guide now?\n1. publish\n2. later']]");
  const carded = await until(async () => (/done: tool bash ok/.test((await text('[data-testid="app.main"]')) ?? '') ? true : null), ANSWER_MS, 1000);
  step("ai-docs' main opened a card for him (the inbox item)", !!carded, ((await text('[data-testid="app.main"]')) ?? '').slice(-300));
  await harness();
  // (g)'s maker (ambient-lead m_21810): an agent with a long name and purpose makes an artifact from
  // its own brief ({{bash: …}}: the fake provider runs those when the message has no [[…]] marker),
  // dropped at the end of this pass. Its page has no quote in it (the brief is in double quotes).
  const MAKER = 'release-ts-pattern';
  const makerHtml = '<html><body style=margin:0;background:#2e8b57><h1>release notes</h1></body></html>';
  await say(`[[bash: sb spawn ${MAKER} --objective "Release ts-pattern on npm: the changelog, the tag, the GitHub release notes and the docs site {{bash: printf '%s' '${makerHtml}' > rel-ts.html && sb artifact add rel-ts.html --title rel-ts-notes}}"]]`);
  // the right side (native-mac d1, app/side3d; the right bar is gone), in either of its forms
  // (Host.tsx coinsAt): the outer card (app.outer.<id> sections, what a 1280 window shows) or its
  // coins (app.bubble.<id> / app.mini.<id>), each a float by ⌥⌘1-4; the same four, in order
  const SIDE_IDS = ['agents', 'artifacts', 'scheduled', 'processes'];
  const sideOrder = () => page.evaluate(() => [...document.querySelectorAll('[data-testid^="app.outer."], [data-testid^="app.bubble."], [data-testid^="app.mini."]')]
    .map((e) => /^app\.(?:outer|bubble|mini)\.(agents|artifacts|scheduled|processes)$/.exec(e.getAttribute('data-testid') ?? '')?.[1] ?? null).filter(Boolean)).catch((): (string | null)[] => []);
  const order = await until(async () => { const o = await sideOrder(); return o.length >= SIDE_IDS.length ? o : null; }, 10000, 500) ?? await sideOrder();
  const sideCard = (await page.locator('[data-testid="app.outer"]').count()) > 0;
  await shot('right-side');
  step('the right side: agents, artifacts, scheduled, processes, in order', order.join(',') === SIDE_IDS.join(','), { order, form: sideCard ? 'outer card' : 'coins' });
  const float = (id: string) => page.locator(`[data-testid="app.flyout.${id}"]`).first();
  // a float by its key (⌥⌘1-4), from the thread, so the key never lands in a field; the outer
  // card has no float (its sections are the lists): true when that section is there
  const openFloat = async (n: number, id: string) => {
    if (await page.locator(`[data-testid="app.outer.${id}"]`).first().isVisible().catch(() => false)) return true;
    if (await float(id).isVisible().catch(() => false)) return true;
    // its coin (a mini in the bar when the panel is open, else a bubble on the side), clicked as he
    // does; else its key
    const coin = page.locator(`[data-testid="app.mini.${id}"], [data-testid="app.bubble.${id}"]`).first();
    if (await coin.isVisible().catch(() => false)) await coin.click({ timeout: 3000 }).catch(() => {});
    else {
      await page.locator('[data-testid="app.main"]').first().click({ position: { x: 40, y: 140 } }).catch(() => {});
      await page.keyboard.press(`Alt+Meta+${n}`);
    }
    return float(id).waitFor({ timeout: 5000 }).then(() => true, () => false);
  };
  // a float closes on esc; the outer card stays
  const closeFloat = async (id: string) => {
    if (!(await float(id).isVisible().catch(() => false))) return;
    await page.keyboard.press('Escape');
    await float(id).waitFor({ state: 'detached', timeout: 3000 }).catch(() => {});
  };
  // the side's agents list: the outer card's rows, else the agents float's
  const agentsList = () => page.locator('[data-testid="app.outer.agents.rows"], [data-testid="app.flyout.agents"]').first();
  const agentsOpen = await openFloat(1, 'agents');
  const agentsText = agentsOpen ? ((await agentsList().innerText({ timeout: 3000 }).catch(() => '')) ?? '') : '';
  await shot('side-agents');
  step('the right side lists the agents (keeper; never the dropped one)', agentsOpen && /\bkeeper\b/.test(agentsText) && !/\bgone\b/.test(agentsText), { open: agentsOpen, text: agentsText.slice(0, 300) });
  await closeFloat('agents');
  const leftAgents = await page.locator('[data-testid="app.sidebar"] [data-testid*=".agent."]').count().catch(() => 0);
  step('the left bar holds places only (no agent rows)', leftAgents === 0 && !/\bkeeper\b/.test((await text('[data-testid="app.sidebar"]')) ?? ''), { agent_rows: leftAgents });
  // ⌥⌘B (⌥⌘0 kept as its alias) and the bar's panel button (app.panel.toggle) hide and bring back
  // the panel: each flips it, a second press puts it back as it was
  const panel = page.locator('[data-testid="app.panel"]').first();
  const panelOn = async () => (await panel.count()) > 0 && await panel.isVisible().catch(() => false);
  const flips = async (press: () => Promise<unknown>) => {
    const was = await panelOn();
    await press();
    const flipped = await until(async () => ((await panelOn()) !== was ? true : null), 5000, 300);
    await press();
    const back = await until(async () => ((await panelOn()) === was ? true : null), 5000, 300);
    return { was, flipped: !!flipped, back: !!back };
  };
  const byKey = await flips(() => page.keyboard.press('Alt+Meta+b'));
  const byAlias = await flips(() => page.keyboard.press('Alt+Meta+0'));
  await shot('panel-toggled');
  step('⌥⌘B (and ⌥⌘0) hides and brings back the panel', byKey.flipped && byKey.back && byAlias.flipped && byAlias.back, { key: byKey, alias: byAlias });
  const tog = page.locator('[data-testid="app.panel.toggle"]').first();
  const byButton = await tog.waitFor({ timeout: 5000 }).then(() => flips(() => tog.click({ timeout: 3000 }).catch(() => {})), () => null);
  step("the bar's panel button hides and brings back the panel", !!byButton && byButton.flipped && byButton.back, byButton ?? 'no app.panel.toggle');
  // an artifact from the artifacts float (⌥⌘2): it opens in the panel's tab beside the thread,
  // its page drawn in the frame; ✕ and esc close it
  // an artifact's row on the side (the outer card's and the float's rows are both
  // app.flyout.artifacts.<project>.<id>): shown, the outer card's 'more' opened when it hides there
  const sideArt = async (title: string) => {
    const r = page.locator('[data-testid^="app.flyout.artifacts."]').filter({ hasText: title }).first();
    if (!(await openFloat(2, 'artifacts'))) return null;
    if (await r.isVisible().catch(() => false)) return r;
    // a float lists the newest few only (its 'all ›' goes to the library): with the panel open,
    // hide it (⌥⌘B) so the side is the outer card again, whose 'more' lists them all
    if (await float('artifacts').isVisible().catch(() => false) && await page.locator('[data-testid="app.panel"]').first().isVisible().catch(() => false)) {
      await closeFloat('artifacts');
      await page.keyboard.press('Alt+Meta+b');
      await page.locator('[data-testid="app.outer.artifacts"]').first().waitFor({ timeout: 5000 }).catch(() => {});
      if (await r.isVisible().catch(() => false)) return r;
    }
    const more = page.locator('[data-testid="app.outer.artifacts.more"]').first();
    if (await more.isVisible().catch(() => false)) await more.click({ timeout: 3000 }).catch(() => {});
    return (await r.waitFor({ timeout: 3000 }).then(() => true, () => false)) ? r : null;
  };
  const fromSide = async (name: string) => {
    const r = await sideArt('proof-html');
    if (!r) { step(name, false, 'no proof-html row on the right side (outer card or artifacts float)'); return false; }
    return click(name, r);
  };
  const opened = await fromSide('proof-html from the right side');
  const pageEl = page.locator('[data-testid="artifact.page"]').first();
  const drawn = opened && await pageEl.waitFor({ timeout: 15000 }).then(() => true, () => false);
  let green = 0;
  if (drawn) {
    await page.waitForTimeout(1500);
    const png = (await pageEl.screenshot({ timeout: 15000 }).catch(() => null))?.toString('base64');
    green = png ? await page.evaluate(async (b64) => {
      const img = new Image(); img.src = `data:image/png;base64,${b64}`; await img.decode();
      const c = document.createElement('canvas'); c.width = img.width; c.height = img.height;
      const g = c.getContext('2d'); if (!g) return 0; g.drawImage(img, 0, 0);
      const d = g.getImageData(0, 0, c.width, c.height).data; let n = 0;
      for (let i = 0; i < d.length; i += 4) if (Math.abs(d[i] - 46) < 12 && Math.abs(d[i + 1] - 139) < 12 && Math.abs(d[i + 2] - 87) < 12) n++;
      return n / (d.length / 4);
    }, png) : 0;
  }
  const cols = await page.evaluate(() => {
    const vis = (sel: string) => { const e = document.querySelector(sel); if (!e) return false; const r = e.getBoundingClientRect(); return r.width > 0 && r.height > 0; };
    // the left bar: shown, and left of the thread (1c5659b8 run: the dragged sidebar is ~240 px, a
    // fixed 160 px rail check read it as missing)
    const side = document.querySelector('[data-testid="app.sidebar"]')?.getBoundingClientRect();
    const main = document.querySelector('[data-testid="app.main"]')?.getBoundingClientRect();
    // (native-mac: the places rail sits left of the sidebar, so the sidebar starts at ~56 px)
    const rail = !!side && side.width > 0 && side.left < 80 && (!main || side.right <= main.left + 2);
    // native-mac: the artifact opens in the panel's tab, right of the thread (no 3-column view)
    const pg = document.querySelector('[data-testid="artifact.page"]');
    const inPanel = !!pg?.closest('[data-testid="app.panel"]');
    const pr = pg?.getBoundingClientRect();
    const right = !!pr && !!main && pr.left >= main.right - 2;
    return { rail, rail_w: side ? Math.round(side.width) : null, thread: vis('[data-testid="app.main"]'), page: vis('[data-testid="artifact.page"]'), in_panel: inPanel, right_of_thread: right };
  });
  await shot('artifact-from-side');
  step('an artifact from the right side: in the panel beside the thread, its page drawn (green pixels)', drawn && green > 0.3 && cols.rail && cols.thread && cols.page && cols.in_panel && cols.right_of_thread, { opened, drawn, green: Math.round(green * 100) / 100, cols });
  const closed = await click('✕', page.locator(ART_CLOSE).first());
  const gone1 = closed && await pageEl.waitFor({ state: 'detached', timeout: 5000 }).then(() => true, () => false);
  await fromSide('proof-html again');
  await pageEl.waitFor({ timeout: 10000 }).catch(() => {});
  // esc where he is: the thread's composer beside the page (the row he just clicked is the side's)
  await page.locator('[data-testid="composer.field"]:visible').first().click({ timeout: 3000 }).catch(() => {});
  await page.keyboard.press('Escape');
  const gone2 = await pageEl.waitFor({ state: 'detached', timeout: 5000 }).then(() => true, () => false);
  step('✕ and esc close the artifact', gone1 && gone2, { x: gone1, esc: gone2 });
  // e5759db9 (his ask, main m_21531), as app/artifact-view.spec checks it: words in harness' main
  // composer, then the artifact open and words in the composer beside it ('about' the page). One esc
  // closes the artifact: main's words and label as he left them; back on the artifact, its words and
  // label too. A second esc does today's thing (a composer with words keeps it: no move, nothing reopens).
  const MAIN_WORDS = 'half a thought for main';
  const ART_WORDS = 'and the green, is it on brand?';
  const field = page.locator('[data-testid="composer.field"]:visible').first();
  // the composer's label: whom his words go to (composer.ctx.to since native-mac), never its state
  const lbl = () => page.locator('[data-testid="composer.ctx.to"]:visible, .b-composer .lbl:visible').first().innerText({ timeout: 1000 })
    .then((s) => s.replace(/[∿≈~∼∽·]+/g, ' ').replace(/\s+/g, ' ').trim(), () => null);
  const fieldText = async () => ((await field.innerText({ timeout: 1000 }).catch(() => '')) ?? '').trim();
  const typeIn = async (words: string) => {
    if (!(await click('the composer', field))) return false;
    await page.keyboard.press('Meta+a');
    await page.keyboard.press('Backspace');
    await page.keyboard.type(words, { delay: 5 });
    return true;
  };
  await harness();
  const mainTyped = await typeIn(MAIN_WORDS);
  const mainLabel = await lbl();
  await fromSide('proof-html for the esc row');
  const reopened = await pageEl.waitFor({ timeout: 10000 }).then(() => true, () => false);
  // native-mac: the artifact opens in the panel beside the thread, and the thread's one composer
  // stays (Open.tsx draws its own composer only with no thread beside the page): his words and its
  // label are main's before, with the page open, after esc closes it and when it opens again
  let escRow: Record<string, unknown> = { main_typed: mainTyped, reopened };
  if (mainTyped && reopened && await click('the composer beside the artifact', field)) {
    const beside = { words: await fieldText(), label: await lbl() };
    await page.keyboard.press('Escape');
    const closed1 = await pageEl.waitFor({ state: 'detached', timeout: 5000 }).then(() => true, () => false);
    await page.waitForTimeout(300);
    const back = { words: await fieldText(), label: await lbl() };
    await shot('esc-artifact-main-kept');
    await fromSide('proof-html again');
    const again = await pageEl.waitFor({ timeout: 10000 }).then(() => true, () => false);
    const onArt = again ? { words: await fieldText(), label: await lbl() } : null;
    await shot('esc-artifact-words-back');
    escRow = { closed: closed1, before: { words: MAIN_WORDS, label: mainLabel }, beside, after_esc: back, reopened_again: onArt };
    step("esc with an artifact open closes it first; his words and the composer's label stay (the thread's one composer beside the page)",
      closed1 && [beside, back, onArt].every((x) => !!x && x.words === MAIN_WORDS && x.label === mainLabel), escRow);
    // close it again (esc from the artifact's composer), then a second esc on harness' main with words
    await click('the composer beside the artifact', field);
    await page.keyboard.press('Escape');
    await pageEl.waitFor({ state: 'detached', timeout: 5000 }).catch(() => {});
    await click('the composer', field);
    await page.keyboard.press('Escape');
    await page.waitForTimeout(800);
    const second = {
      artifact_reopened: (await pageEl.count()) > 0,
      went_up_to_bise: (await page.locator('[data-testid="app.side.bise"][aria-current]').count()) > 0,
      words: await fieldText(),
      label: await lbl(),
    };
    await shot('esc-artifact-second-esc');
    step("a second esc does today's thing (a composer with words keeps it: no move, nothing reopens)", !second.artifact_reopened && !second.went_up_to_bise, second);
  } else step("esc with an artifact open closes it first; his words and the composer's label stay (the thread's one composer beside the page)", false, escRow);
  // both composers emptied, so the next rows start clean
  await closeArtifact();
  await harness();
  if (await click('the composer', field)) { await page.keyboard.press('Meta+a'); await page.keyboard.press('Backspace'); }
  await fromSide('proof-html to empty its composer');
  if (await pageEl.waitFor({ timeout: 10000 }).then(() => true, () => false) && await click('the composer beside the artifact', field)) {
    await page.keyboard.press('Meta+a');
    await page.keyboard.press('Backspace');
  }
  await closeArtifact();
  // back / forward: harness -> artifacts -> ⌘[ harness -> ⌘] artifacts; esc from harness' main goes up to bise
  await harness();
  await click('artifacts', page.locator('[data-testid="app.rail.artifacts"]').first());
  await page.locator('[data-testid="artifacts.screen"]').first().waitFor({ timeout: 8000 }).catch(() => {});
  await page.keyboard.press('Meta+BracketLeft');
  const wentBack = await until(async () => ((await page.locator('[data-testid="artifacts.screen"]').count()) === 0 && (await projectRow('harness').getAttribute('aria-current').catch(() => null)) !== null ? true : null), 5000, 300);
  await page.keyboard.press('Meta+BracketRight');
  const wentFwd = await page.locator('[data-testid="artifacts.screen"]').first().waitFor({ timeout: 5000 }).then(() => true, () => false);
  step('⌘[ goes back, ⌘] forward', !!wentBack && wentFwd, { back: !!wentBack, forward: wentFwd });
  await harness();
  await page.locator('[data-testid="app.main"]').first().click({ position: { x: 40, y: 140 } }).catch(() => {});
  await page.keyboard.press('Escape');
  const upToBise = await until(async () => (/every project|to bise\s*$/m.test((await text('[data-testid="app.main"]')) ?? '') || (await page.locator('[data-testid="app.side.bise"][aria-current]').count()) > 0 ? true : null), 5000, 300);
  await shot('esc-up');
  step("esc from a project's main goes up to bise", !!upToBise, ((await text('[data-testid="app.main"]')) ?? '').slice(0, 200));
  // the working row while an agent works (a slow turn), then a short thought with its words
  // 96750f52 (ambient-lead m_21810 (b)): the working mark, the TUI's gust after 'to main' on the
  // composer's label line; no 'is working' row in the thread (thread.live / thread.waiting are gone),
  // and nothing in the thread moves with the gust's frames (sampled every 110 ms, its frame step)
  // 491959b8f (designer m_34075, signed m_34467): the mark is the 1-cell breath now, one glyph a
  // frame (thread.mark.breath, 6 frames of 110 ms); 2+ glyphs in 16 samples is it moving
  // native-mac (his m_44719, ComposerTab.tsx): the mark sits in the composer's head, after 'to bise'
  // and the project, as its state item (composer.ctx.state, data-state=working: the breath
  // composer.ctx.state.mark and 'working · 0 s'); the label is composer.ctx.to
  await harness();
  await say('take a moment [[slow: 12]]');
  const mark = page.locator('[data-testid="composer.ctx.state"][data-state="working"]').first();
  const marked = await mark.waitFor({ timeout: 10000 }).then(() => true, () => false);
  type Geo = { list: string | null; mark: string | null; composer: string | null; scroll: string | null; breath: string | null } | null;
  const wk: {
    mark: boolean; label?: string | null; breath_frames?: number; scrolls?: (string | null)[];
    still?: Record<string, boolean>; scroll_jumps?: number; first?: Geo; old_rows?: number;
    is_working_text?: boolean; on_label_line?: boolean; tip?: string | null;
  } = { mark: marked };
  if (marked) {
    wk.label = await page.locator('[data-testid="composer.ctx.to"]:visible').first().innerText({ timeout: 1000 }).catch(() => null);
    const geo = () => page.evaluate(() => {
      const r = (s: string) => { const e = document.querySelector(s); if (!e) return null; const b = e.getBoundingClientRect(); return [b.left, b.top, b.width, b.height].map(Math.round).join(','); };
      const sc = document.querySelector('[data-testid="thread.list"] [data-virtuoso-scroller]') ?? document.querySelector('[data-testid="thread.list"]');
      return { list: r('[data-testid="thread.list"]'), mark: r('[data-testid="composer.ctx.state.mark"]'), composer: r('[data-testid="composer"]'), scroll: sc ? `${Math.round(sc.scrollTop)},${sc.scrollHeight}` : null, breath: document.querySelector('[data-testid="composer.ctx.state.mark"]')?.textContent ?? null };
    }).catch(() => null);
    // 96db3e08 run: one 18 px scrollTop move right after the send (the send's bottom settle, not
    // the gust?). Let the send settle 500 ms before sampling, so a move left is the gust's (ambient-lead m_22035)
    await page.waitForTimeout(500);
    const g: Geo[] = [];
    for (let i = 0; i < 16; i++) { g.push(await geo()); await page.waitForTimeout(110); }
    const same = (k: 'list' | 'mark' | 'composer') => new Set(g.map((x) => x?.[k] ?? null)).size === 1;
    wk.breath_frames = new Set(g.map((x) => x?.breath ?? null)).size;
    // the scroller may grow when a row lands (new content, kept at the bottom): that is not the gust
    // moving it. Red only when scrollTop moves while scrollHeight stays (the thread jumps by itself).
    const sc = g.map((x) => (x?.scroll ?? '').split(',').map(Number));
    const jumps = sc.slice(1).filter((s, i) => s[1] === sc[i][1] && s[0] !== sc[i][0]).length;
    wk.scrolls = [...new Set(g.map((x) => x?.scroll ?? null))];
    wk.still = { list: same('list'), scroll: jumps === 0, composer: same('composer'), mark_box: same('mark') };
    wk.scroll_jumps = jumps;
    wk.first = g[0];
    wk.old_rows = await page.locator('[data-testid="thread.live"], [data-testid="thread.waiting"]').count();
    wk.is_working_text = /is working/.test((await text('[data-testid="thread.list"]')) ?? '');
    const lb = await page.locator('[data-testid="composer.ctx.to"]:visible').first().boundingBox().catch(() => null);
    const mb = await page.locator('[data-testid="composer.ctx.state.mark"]').first().boundingBox().catch(() => null);
    wk.on_label_line = !!lb && !!mb && Math.abs((lb.y + lb.height / 2) - (mb.y + mb.height / 2)) <= 6;
    await shot('working-mark');
    wk.tip = await mark.innerText({ timeout: 2000 }).catch(() => null);
    await shot('working-mark-tip');
    await page.mouse.move(1, 400);
  }
  // the home thread names its main 'bise' (ambient-lead m_22076): the label reads 'to bise'
  step("the working mark: a breath after 'to bise' while main works, no 'is working' row, the thread still under its frames",
    marked && /^to (bise|main)\b/.test((wk.label ?? '').trim()) && wk.on_label_line && wk.old_rows === 0 && !wk.is_working_text && (wk.breath_frames ?? 0) >= 2 && Object.values(wk.still ?? {}).every(Boolean), wk);
  await until(async () => (/ack: take a moment/.test((await text('[data-testid="app.main"]')) ?? '') ? true : null), ANSWER_MS, 1000);
  await say('one quick thought [[think: weighing the proof rows]]');
  // the TUI folds a thought too (ambient-lead m_20609): the row is there, then opens to its words
  const rows = page.locator('[data-testid^="thread.entry."]').filter({ has: page.locator('[data-testid^="thread.think."]') });
  const nRows = (await until(async () => ((await rows.count()) > 0 ? await rows.count() : null), ANSWER_MS, 1000)) ?? 0;
  const last = rows.nth(Math.max(0, nRows - 1));
  if (nRows) await last.getByText(/thought for/).first().click({ timeout: 3000 }).catch(() => last.click({ timeout: 3000 }).catch(() => {}));
  const words = nRows ? await until(async () => (/weighing the proof rows/.test((await last.textContent().catch(() => '')) ?? '') ? true : null), 5000, 300) : null;
  await shot('short-thought');
  step('a short thought: its row is there and opens to its words', !!words, { rows: nRows, text: nRows ? ((await last.textContent().catch(() => '')) ?? '').slice(0, 200) : '' });
  // the inbox block in bise's thread: another project's item answered there; ⌘I opens it in place
  await openBise();
  // a long reply in bise's own thread, so it scrolls for the block's height row (1c5659b8 run: the
  // thread was empty, nothing scrolled)
  // regress-25e5: a path naming a project routes these words there (fx.ts), and the rows below
  // read a reply that never comes to bise's thread
  // the project rows only (app.side.project.<id>, not their parts), their first line: the name
  const projects = (await page.evaluate(() => [...document.querySelectorAll('[data-testid^="app.side.project."]')]
    .filter((r) => /^app\.side\.project\.[^.]+$/.test(r.getAttribute('data-testid') ?? ''))
    .map((r) => ((r instanceof HTMLElement ? r.innerText : '') ?? '').split(String.fromCharCode(10))[0]?.trim() ?? '')).catch((): string[] => [])).filter(Boolean);
  const hits = named(FX, projects);
  step("the files the words carry name no world project (else bise's thread routes them there)", projects.length > 0 && hits.length === 0, { fx: FX, projects, named: hits });
  const longMd = path.join(FX, 'long-reply.md');
  fs.writeFileSync(longMd, Array.from({ length: 60 }, (_, i) => `scroll line ${i + 1}: a long reply so bise's thread scrolls.`).join('\n\n') + '\n');
  await say(`a long one please [[reply: ${longMd}]]`);
  const longIn = await until(async () => (/scroll line 60/.test((await text('[data-testid="thread.list"]')) ?? '') ? true : null), ANSWER_MS, 1000);
  step("a long reply in bise's thread (so it scrolls)", !!longIn, null);
  const block = page.locator('[data-testid^="inbox.block"]').first();
  const hasBlock = await block.waitFor({ timeout: 15000 }).then(() => true, () => false);
  const blockText = hasBlock ? ((await block.innerText().catch(() => '')) ?? '') : '';
  await shot('inbox-block');
  step("bise's thread shows the inbox block with another project's item", hasBlock && /shop|telemetry|docs|ai-docs|folder/.test(blockText), blockText.slice(0, 300));
  // f29e895c (designer m_21505): the block never folds or changes height when the thread scrolls
  const blockH = () => page.locator('[data-testid="inbox.block"]').first().boundingBox({ timeout: 2000 }).then((b) => (b ? Math.round(b.height) : null), () => null);
  const listTop = () => page.evaluate(() => {
    const list = document.querySelector('[data-testid="thread.list"]');
    const scrolls = (x: Element) => x.scrollHeight > x.clientHeight + 2 && /auto|scroll/.test(getComputedStyle(x).overflowY);
    let e = list && [list, ...list.querySelectorAll('*')].find(scrolls);
    if (!e) { e = list; while (e && !scrolls(e)) e = e.parentElement; }
    return e ? Math.round(e.scrollTop) : null;
  }).catch(() => null);
  if (hasBlock) {
    const h0 = await blockH();
    const top0 = await listTop();
    const lb = await page.locator('[data-testid="thread.list"]').first().boundingBox({ timeout: 3000 }).catch(() => null);
    if (lb) await page.mouse.move(lb.x + lb.width / 2, lb.y + lb.height * 0.7);
    const hs = [];
    const tops = [];
    // 8b00ed2b (ambient-lead m_21810 (c), as thread/steady-bottom.spec): the composer, its keys line
    // and the inbox block keep their boxes while the thread scrolls, moving and stopped
    const bottom = () => page.evaluate(() => {
      const at = (sel: string) => { const e = document.querySelector(sel); if (!e) return null; const r = e.getBoundingClientRect(); return [r.left, r.top, r.width, r.height].map(Math.round).join(','); };
      return { composer: at('.th-screen .b-composer') ?? at('[data-testid="composer"]'), keys: at('[data-testid="composer.keys"]'), block: at('[data-testid="inbox.block"]') };
    }).catch(() => null);
    const rest = await bottom();
    const moving = [];
    const stopped = [];
    for (const dy of [-200, -200, -200, -200, 200, 200, 800]) {
      await page.mouse.wheel(0, dy);
      moving.push(await bottom());
      await page.waitForTimeout(150);
      hs.push(await blockH());
      tops.push(await listTop());
      await page.waitForTimeout(250);
      stopped.push(await bottom());
    }
    await shot('inbox-block-scrolled');
    const moved = tops.some((t) => t !== null && t !== top0);
    step('the inbox block keeps its height on scroll', h0 !== null && moved && hs.every((x) => x !== null && Math.abs(x - h0) <= 1), { height: h0, during: hs, scroll_top: [top0, ...tops], thread_scrolled: moved });
    const key = (b: unknown) => JSON.stringify(b);
    const off = [...moving, ...stopped].map((b, i) => (key(b) === key(rest) ? null : { at: i < moving.length ? `moving ${i}` : `stopped ${i - moving.length}`, box: b })).filter(Boolean);
    // native-mac: the keys line is gone (a held modifier lists its keys in the composer's own row,
    // composer.held), so the composer and the inbox block are the bottom's boxes
    step('steady bottom: wheel up and down on a long thread, the composer and inbox block boxes unchanged',
      !!rest && !!rest.composer && !!rest.block && moved && off.length === 0, { rest, thread_scrolled: moved, changed: off.slice(0, 6) });
  } else step('the inbox block keeps its height on scroll', false, 'no inbox block');
  // 82cd5263 + a6efc5cb (ambient-lead m_21810 (d), as app/cmd-hints.spec): ⌘ held alone 400 ms shows
  // the keys where they act: bise's '0', each project's digit in place of its count, ⌘[ under ‹ (a
  // place behind), ⌘] under › (one ahead), ⌘I on the inbox head; released, all gone. A quick ⌘: none.
  // native-mac (nm-frame, rail-keys): the sidebar's held digits are <row>.digit ('⌘0' on bise,
  // '⌘1'-'⌘9' on the projects, in place of the row's pill), the bar's history arrows'
  // app.bar.back/forward.hint, the inbox block's head inbox.block.hint
  const hintsNow = () => page.evaluate(() => {
    const t = (s: string) => document.querySelector(s)?.textContent?.trim() ?? null;
    const rows = [...document.querySelectorAll('[data-testid^="app.side.project."]')].filter((r) => /^app\.side\.project\.[^.]+$/.test(r.getAttribute('data-testid') ?? ''));
    return {
      n: document.querySelectorAll('.app-keyhint, [data-testid$=".digit"]').length,
      bise: t('[data-testid="app.side.bise.digit"]'),
      projects: rows.map((r) => ({ name: (r.querySelector('.nm')?.textContent ?? r.textContent ?? '').trim().slice(0, 24), hint: r.querySelector('[data-testid$=".digit"]')?.textContent?.trim() ?? null, count: !!r.querySelector('[data-testid$=".pill"]') })),
      back: t('[data-testid="app.bar.back.hint"]'), forward: t('[data-testid="app.bar.forward.hint"]'), inbox: t('[data-testid="inbox.block.hint"]'),
    };
  }).catch(() => null);
  const holdCmd = async (name: string) => {
    await page.keyboard.down('Meta');
    await page.waitForTimeout(700);
    const held = await hintsNow();
    await shot(name);
    await page.keyboard.up('Meta');
    await page.waitForTimeout(300);
    return { held, released: await hintsNow() };
  };
  await page.locator('[data-testid="app.main"]').first().click({ position: { x: 40, y: 140 } }).catch(() => {});
  // e4394a9e: the hints show after 150 ms (the TUI's ctrl hint delay), so a quick ⌘ is 60 ms
  await page.keyboard.down('Meta');
  await page.waitForTimeout(60);
  const quick = await hintsNow();
  await page.keyboard.up('Meta');
  await page.waitForTimeout(400);
  const onBise = await holdCmd('cmd-held-bise');
  const ph = onBise.held?.projects ?? [];
  const digitsOk = ph.length > 0 && ph.slice(0, 9).every((p, i) => p.hint === `⌘${i + 1}` && !p.count);
  step('hold ⌘ 400 ms on bise: digits on bise and the projects, ⌘[ on the back arrow, ⌘I on the inbox head; all gone on release',
    !!onBise.held && quick?.n === 0 && onBise.held.bise === '⌘0' && digitsOk && onBise.held.back === '⌘[' && onBise.held.inbox === '⌘I' && onBise.released?.n === 0,
    { quick_cmd_hints: quick?.n ?? null, ...onBise });
  // one place back (harness), so › has a place ahead: ⌘] under it; then ⌘] to bise again
  await page.keyboard.press('Meta+BracketLeft');
  await page.waitForTimeout(800);
  const behind = await holdCmd('cmd-held-forward');
  step('hold ⌘ after ⌘[: ⌘] on the forward arrow, gone on release', behind.held?.forward === '⌘]' && behind.released?.n === 0, behind);
  await page.keyboard.press('Meta+BracketRight');
  await page.waitForTimeout(800);
  if (!(await page.locator('[data-testid="app.side.bise"][aria-current]').count())) await openBise();
  // ⌘I is the native menu's accelerator (src/main/menu.ts): CDP keys go to the page and never reach
  // the menu, so off-screen it can't be pressed. ⌃1 (the block's own key) opens the same item in place.
  await page.locator('[data-testid="app.main"]').first().click({ position: { x: 40, y: 140 } }).catch(() => {});
  await page.keyboard.press('Control+1');
  // the dock's options carry the composer's id (composer.opt.<n>, ds/composer.tsx); dock.opt before 6ac92eb4
  // b4a25acb: the item opens as the inbox card in the strip's dock, its options strip.card.<key>.opt.<n> (InboxCard.tsx)
  // b51422c0: in bise's thread the item opens in the inbox block itself (Block.tsx's InboxCard,
  // default id inbox.card.<project>-<id>, options inbox.card.<project>-<id>.opt.<n>)
  // b51422c0 run: Block.tsx passes testId inbox.block.card.<key> (inbox.block.notify for a notice),
  // so the options there are inbox.block.card.<key>.opt.<n>
  const OPT = '[data-testid^="strip.card"][data-testid*=".opt."], [data-testid^="inbox.card"][data-testid*=".opt."], [data-testid^="inbox.block.card"][data-testid*=".opt."], [data-testid^="inbox.block.notify.opt."], [data-testid^="composer.opt"], [data-testid^="dock.opt"]';
  const inPlace = await until(async () => ((await page.locator(OPT).count()) > 0 ? true : null), 8000, 400);
  await shot('inbox-ctrl-1');
  step('⌃1 opens the item in place (its options in the dock; ⌘I = native menu, not reachable by CDP)', !!inPlace, ((await text('[data-testid="app.main"]')) ?? '').slice(-300));
  if (inPlace) {
    const opt = page.locator(OPT).first();
    const label = await opt.innerText().catch(() => '');
    await click('answer it there', opt);
    const answered = await until(async () => ((await page.locator(OPT).count()) === 0 ? true : null), 15000, 500);
    await shot('inbox-answered');
    step("another project's item answered from bise's thread", !!answered, { option: label.slice(0, 80) });
  }
  // 1c5659b8 + the hub's 5b19b586 (main m_21514): his free words on another project's docked drafts
  // card, from bise's thread, show the hub's notice as one row above the composer (composer.answer.note).
  // Seeded as amb-core's answer-drafts-real check does: harness' main publishes a page of two drafted
  // mails, so its hub opens one batch card. Never '1. review' (it opens the page in a browser).
  const DRAFTS = 'release replies proof';
  const em = (i: number | string, to: string) => `<section data-kit="email" data-id="${i}" data-verb="draft"><p data-field="to">${to}</p><p data-field="subject">v5.9.1</p><p>x</p></section>`;
  const draftsHtml = '<section data-kit="checklist" data-id="plan" data-plan><ol>'
    + '<li data-id="b1" data-who="bise" data-did="drafted" data-draft="mail-lucas">reply to Lucas</li>'
    + '<li data-id="b2" data-who="bise" data-did="drafted" data-draft="mail-marc">reply to Marc</li></ol></section>'
    + em('mail-lucas', 'lucas@acme.test') + em('mail-marc', 'marc@acme.test');
  await harness();
  await say(`[[bash: printf '%s' '${draftsHtml}' > rel-proof.html && sb page publish rel-proof.html --id rel-proof --title '${DRAFTS}']]`);
  await openBise();
  const draftsRow = page.locator('[data-testid^="inbox.block.row."]', { hasText: DRAFTS }).first();
  // the card by its page's title: its words say 'send these 2 emails?' since the k-pick cards
  // (1de5fc75f), no 'draft' in them
  const draftsCard = page.locator('[data-testid^="inbox.block.card."]').filter({ hasText: DRAFTS }).first();
  const answeringIt = async () => /answering/.test(((await page.locator('[data-testid="composer"]').first().innerText({ timeout: 1000 }).catch(() => '')) ?? ''));
  // the card is there within 90 s; then its row's click, else '+ N more' in turn, until it is the open one
  const listed = await until(async () => ((await draftsRow.isVisible().catch(() => false)) || (await draftsCard.isVisible().catch(() => false)) || /release replies proof|2 drafts/.test(((await text('[data-testid="inbox.block"]')) ?? '')) ? true : null), 90000, 1000);
  for (let i = 0; listed && i < 40 && !(await draftsCard.isVisible().catch(() => false)); i++) {
    if (await draftsRow.isVisible().catch(() => false)) await draftsRow.click({ timeout: 2000 }).catch(() => {});
    else await page.locator('[data-testid="inbox.block.more"]').first().click({ timeout: 2000 }).catch(() => {});
    await page.waitForTimeout(500);
  }
  const docked = await draftsCard.isVisible().catch(() => false);
  const answering = docked && await answeringIt();
  await shot('drafts-card-docked');
  let note = null;
  if (docked && answering) {
    const f = page.locator('[data-testid="composer.field"]:visible').first();
    await f.click({ timeout: 3000 }).catch(() => {});
    await page.keyboard.press('Meta+a');
    await page.keyboard.press('Backspace');
    await page.keyboard.type('where are the drafts?', { delay: 10 });
    await page.keyboard.press('Enter');
    note = await until(async () => {
      const n = page.locator('[data-testid="composer.answer.note"]').first();
      if (!(await n.isVisible().catch(() => false))) return null;
      return { level: await n.getAttribute('data-level').catch(() => null), text: ((await n.innerText().catch(() => '')) ?? '').slice(0, 300) };
    }, 30000, 500);
    await shot('drafts-card-notice');
  }
  step("free words on a docked drafts card: the hub's notice row above the composer", !!note && note.level !== 'err' && /sent to/.test(note.text), { listed: !!listed, docked, answering, note, block: ((await text('[data-testid="inbox.block"]')) ?? '').slice(0, 300) });
  // the two-finger swipe (swipe.ts reads the renderer's wheel events): a horizontal wheel stream
  // through CDP, fingers right (deltaX < 0) walks back, fingers left forward. Events 20 ms apart
  // stay one gesture (GAP_MS 220); 8 x 30 px passes THRESHOLD_PX 140.
  const swipe = async (dx: number) => {
    const m = await page.locator('[data-testid="app.main"]').first().boundingBox().catch(() => null);
    if (m) await page.mouse.move(m.x + m.width / 2, m.y + m.height / 3);
    for (let i = 0; i < 8; i++) { await page.mouse.wheel(dx, 0); await new Promise((r) => setTimeout(r, 20)); }
    await new Promise((r) => setTimeout(r, 400));
  };
  await harness();
  await click('artifacts', page.locator('[data-testid="app.rail.artifacts"]').first());
  const onList = await page.locator('[data-testid="artifacts.screen"]').first().waitFor({ timeout: 8000 }).then(() => true, () => false);
  // his no-swipe (316c1391c): a sideways swipe never walks, either way (it took him away from his
  // words); ⌘[ ⌘], the side buttons and ‹ › walk. Fingers right, then left: still on /artifacts
  await swipe(-30);
  await page.waitForTimeout(600);
  const stayBack = (await page.locator('[data-testid="artifacts.screen"]').count()) > 0;
  await shot('swipe-back');
  await swipe(30);
  await page.waitForTimeout(600);
  const stayFwd = (await page.locator('[data-testid="artifacts.screen"]').count()) > 0;
  await shot('swipe-forward');
  step('two-finger swipe (CDP wheel deltaX) either way never walks: still on /artifacts', onList && stayBack && stayFwd, { on_list: onList, after_right: stayBack, after_left: stayFwd });
  // 3f58f843 (ambient-lead m_21499): ⌘K finds pages by name: 'set' shows the pages group, and ⏎ on
  // its row opens settings (the selection walked down to it with ↓, as he would)
  await harness();
  await page.keyboard.press('Meta+k');
  const pal = await page.locator('[data-testid="app.palette"]').first().waitFor({ timeout: 5000 }).then(() => true, () => false);
  let kRow: { palette: boolean; groups?: string[] | null; row?: unknown; settings?: boolean } = { palette: pal };
  if (pal) {
    await page.keyboard.type('set', { delay: 40 });
    const groups = await until(async () => {
      const g = await page.locator('[data-testid^="app.palette.group."]').evaluateAll((els) => els.map((e) => (e.getAttribute('data-testid') ?? '').slice('app.palette.group.'.length)));
      return g.includes('pages') ? g : null;
    }, 5000, 300);
    const at = await page.evaluate(() => {
      const num = (e: Element | null) => (e ? Number((e.getAttribute('data-testid') ?? '').split('.').pop()) : null);
      const first = document.querySelector('[data-testid="app.palette.group.pages"] [data-testid^="app.palette.item."]');
      return { want: num(first), cur: num(document.querySelector('[data-testid^="app.palette.item."][aria-selected="true"]')), name: first?.textContent?.slice(0, 80) ?? null };
    });
    await shot('palette-set');
    if (at.want !== null) for (let i = at.cur ?? 0; i < at.want; i++) await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Enter');
    const settings = await page.locator('[data-testid="settings.root"]').first().waitFor({ timeout: 5000 }).then(() => true, () => false);
    await shot('palette-set-enter');
    kRow = { palette: pal, groups, row: at, settings };
  }
  step("⌘K with 'set' shows the pages group, ⏎ opens settings", !!kRow.groups && !!kRow.settings, kRow);
  if (pal && !kRow.settings) await page.keyboard.press('Escape');
  // spacing (designer's desktop-spacing, steps 1-4): a frame per screen at 1100 wide. The viewport
  // is narrowed by CDP emulation, the native window doesn't move or resize.
  const cdp = await page.context().newCDPSession(page);
  const vh = await page.evaluate(() => window.innerHeight);
  const narrowed = await cdp.send('Emulation.setDeviceMetricsOverride', { width: 1100, height: vh, deviceScaleFactor: 0, mobile: false }).then(() => true, () => false);
  await new Promise((r) => setTimeout(r, 600));
  const vw = await page.evaluate(() => window.innerWidth);
  const overflow = async () => page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth + 1);
  // each screen at 1100 wide: does the page overflow, is the screen there (and what it adds)
  const at1100: Record<string, { overflow: boolean; there?: boolean; [k: string]: unknown }> = {};
  await harness();
  await page.waitForTimeout(800);
  await shot('w1100-thread');
  at1100.thread = { overflow: await overflow() };
  // the frame: the title bar, both bars and the status line, with the right bar open
  if (!(await page.locator('[data-testid="app.panel"]').count())) await click('the panel', page.locator('[data-testid="app.panel.toggle"]').first());
  await page.waitForTimeout(600);
  await shot('w1100-frame');
  at1100.frame = { overflow: await overflow(), panel: (await page.locator('[data-testid="app.panel"]').count()) > 0 };
  await openBise();
  await page.locator('[data-testid^="inbox.block"]').first().waitFor({ timeout: 8000 }).catch(() => {});
  await page.waitForTimeout(600);
  await shot('w1100-inbox');
  at1100.inbox = { overflow: await overflow(), block: (await page.locator('[data-testid^="inbox.block"]').count()) > 0 };
  await harness();
  await say('/agents');
  const agents = await page.locator('[data-testid="agents.screen"]').first().waitFor({ timeout: 10000 }).then(() => true, () => false);
  await page.waitForTimeout(600);
  await shot('w1100-agents');
  at1100.agents = { overflow: await overflow(), screen: agents };
  // b4a25acb's spacing batch: artifacts, scheduled, /log, /diff (changes), ⌘K, settings and a menu
  const screen1100 = async (name: string, open: () => Promise<unknown>, sel: string | null) => {
    await open();
    const there = sel ? await page.locator(sel).first().waitFor({ timeout: 10000 }).then(() => true, () => false) : true;
    await page.waitForTimeout(600);
    await shot(`w1100-${name}`);
    at1100[name] = { overflow: await overflow(), there };
  };
  await screen1100('artifacts', () => click('artifacts', page.locator('[data-testid="app.rail.artifacts"]').first()), '[data-testid="artifacts.screen"]');
  await screen1100('scheduled', () => click('scheduled', page.locator('[data-testid="app.rail.scheduled"]').first()), null);
  await screen1100('log', async () => { await harness(); await say('/log'); }, '[data-testid="log.list"]');
  await screen1100('changes', async () => { await harness(); await say('/diff'); }, '[data-testid="changes.screen"], [data-testid="review.screen"]');
  await page.keyboard.press('Escape');
  await screen1100('palette', async () => { await harness(); await page.keyboard.press('Meta+k'); }, '[data-testid="app.palette"]');
  await page.keyboard.press('Escape');
  await screen1100('settings', () => click('settings', page.locator('[data-testid="app.rail.settings"]').first()), '[data-testid="settings.root"]');
  const menuBtn = page.locator('[data-testid="settings.root"] [aria-haspopup]').first();
  const hasMenu = (await menuBtn.count()) > 0;
  if (hasMenu) await screen1100('settings-menu', () => click('a settings menu', menuBtn), '[role="menu"], [role="listbox"]');
  await page.keyboard.press('Escape');
  // b51422c0's batch (ambient-lead m_21481): settings' key field and its caption, the terminal
  // panel, a thread with a tool row and a code block, the selection tint and a quote chip
  // the key row lives on the keys page (accounts.tsx), not the page settings opens on
  await click('settings', page.locator('[data-testid="app.rail.settings"]').first());
  const keysNav = page.locator('[data-testid="settings.nav.keys"]').first();
  if (await keysNav.waitFor({ timeout: 5000 }).then(() => true, () => false)) await click('settings keys', keysNav);
  const keyAdd = page.locator('[data-testid="settings.key.add"]').first();
  if (await keyAdd.waitFor({ timeout: 5000 }).then(() => true, () => false)) {
    await screen1100('settings-key', () => click('add a key', keyAdd), '[data-testid="settings.key.field"]');
    await page.keyboard.press('Escape');
  } else at1100['settings-key'] = { overflow: false, there: false, why: 'no settings.key.add on this world' };
  // native-mac: ⌃` opens the project's shell as a tab of the panel (app.panel.body.term:<p>#<n>,
  // its terminal.screen inside), the bottom terminal.panel before; ⌃` again hides it
  await screen1100('terminal', async () => { await harness(); await page.keyboard.press('Control+Backquote'); }, TERM_SEL);
  if (at1100.terminal.there) {
    await page.keyboard.press('Control+Backquote');
    if (await page.locator('[data-testid="terminal.hide"]').first().isVisible().catch(() => false)) await click('hide the terminal', page.locator('[data-testid="terminal.hide"]').first());
  }
  step('the terminal panel opens at 1100 (⌃`)', at1100.terminal.there && !at1100.terminal.overflow, at1100.terminal);
  const md = path.join(FX, 'code-reply.md');
  fs.writeFileSync(md, 'here is the proof, quote me if you like.\n\n```sh\necho proof-code\nls -la | head -3\n```\n\nthat was the code block.\n');
  await harness();
  await say(`[[bash: echo tool-row-proof]] [[reply: ${md}]]`);
  const coded = await until(async () => page.evaluate(() => {
    const list = document.querySelector('[data-testid="thread.list"]');
    const pre = [...(list?.querySelectorAll('pre') ?? [])].some((p) => p.textContent.includes('proof-code'));
    const tool = (list?.querySelectorAll('[data-testid^="thread.toolrow"]').length ?? 0) > 0;
    return pre && tool ? { pre, tool } : null;
  }), ANSWER_MS, 1000);
  await page.waitForTimeout(600);
  await shot('w1100-thread-code');
  at1100['thread-code'] = { overflow: await overflow(), there: !!coded };
  step('a thread with a tool row and a code block (the thread pass at 1100)', !!coded && !at1100['thread-code'].overflow, coded ?? 'no pre with proof-code, or no tool row');
  // select words of the reply: the tint is the theme's (no system blue), the pop offers the quote
  const tint = await page.evaluate(() => {
    const list = document.querySelector('[data-testid="thread.list"]');
    const w = document.createTreeWalker(list ?? document.body, NodeFilter.SHOW_TEXT);
    let n: Node | null = null;
    while (w.nextNode()) if ((w.currentNode.textContent ?? '').includes('quote me if you like')) n = w.currentNode;
    if (!n || !n.parentElement) return null;
    const i = (n.textContent ?? '').indexOf('quote me');
    const r = document.createRange();
    r.setStart(n, i);
    r.setEnd(n, i + 'quote me if you like'.length);
    const s = window.getSelection();
    if (!s) return null;
    s.removeAllRanges();
    s.addRange(r);
    n.parentElement.dispatchEvent(new MouseEvent('mouseup', { bubbles: true }));
    return { bg: getComputedStyle(n.parentElement, '::selection').backgroundColor, text: s.toString() };
  });
  const pop = await page.locator('[data-testid="thread.quote.pop"]').first().waitFor({ timeout: 5000 }).then(() => true, () => false);
  await shot('w1100-selection');
  step('the selection tint on thread text is the theme\'s, not the system blue', !!tint && !/rgba?\(0, 9\d, 2\d\d/.test(tint.bg) && tint.bg !== 'rgba(0, 0, 0, 0)', tint);
  let chip = null;
  if (pop) {
    await click('quote it', page.locator('[data-testid="thread.quote.pop.ask"]').first());
    chip = await until(async () => page.evaluate(() => {
      const c = document.querySelector('[data-testid="composer"]');
      const q = c?.querySelector('[data-testid*="quote"], .b-quote, [class*="quote"]');
      return q ? (q.textContent || '').slice(0, 120) : null;
    }), 5000, 300);
  }
  await page.waitForTimeout(400);
  await shot('w1100-quote-chip');
  at1100['quote-chip'] = { overflow: await overflow(), there: !!chip };
  step('selected words go to the composer as a quote chip', pop && !!chip, { pop, chip });
  await page.locator('[data-testid="composer"] [contenteditable="true"]').first().click().catch(() => {});
  await page.keyboard.press('Meta+a');
  await page.keyboard.press('Backspace');
  await cdp.send('Emulation.clearDeviceMetricsOverride').catch(() => {});
  // scroll up right after a thread opens stays up (dae80774: the list goes to its end once, then
  // leaves him where he scrolled)
  await click('artifacts', page.locator('[data-testid="app.rail.artifacts"]').first());
  await page.waitForTimeout(500);
  await harness();
  const listBox = await page.locator('[data-testid="thread.list"]').first().boundingBox({ timeout: 5000 }).catch(() => null);
  if (listBox) await page.mouse.move(listBox.x + listBox.width / 2, listBox.y + listBox.height / 2);
  await page.waitForTimeout(150);
  for (let i = 0; i < 6; i++) { await page.mouse.wheel(0, -150); await new Promise((r) => setTimeout(r, 30)); }
  const fromEnd = () => page.evaluate(() => {
    // the scroller is inside thread.list (ThreadList's scrollerRef, b4a25acb), on older builds around it
    const list = document.querySelector('[data-testid="thread.list"]');
    const scrolls = (x: Element) => x.scrollHeight > x.clientHeight + 2 && /auto|scroll/.test(getComputedStyle(x).overflowY);
    let e = list && [list, ...list.querySelectorAll('*')].find(scrolls);
    if (!e) { e = list; while (e && !scrolls(e)) e = e.parentElement; }
    return e ? { fromEnd: Math.round(e.scrollHeight - e.scrollTop - e.clientHeight), height: e.scrollHeight } : null;
  });
  const up0 = await fromEnd();
  await page.waitForTimeout(3000);
  const up1 = await fromEnd();
  await shot('scrolled-up-stays');
  step('scroll up right after a thread opens stays up (3 s later)', !!up0 && !!up1 && up0.fromEnd > 100 && up1.fromEnd > 100, { right_after: up0, three_s_later: up1 });
  step('the spotlight at 1100: not run (it opens a floating window over his screen)', true, 'not proven here');
  step('a frame per screen at 1100 wide (thread, frame, inbox, agents), nothing wider than the window',
    narrowed && vw === 1100 && agents && Object.values(at1100).every((s) => !s.overflow), { width: vw, ...at1100 });
  // the tray menu is native (Electron Tray in the main process): CDP reaches the renderer only
  step('the tray menu\'s inbox rows: not reachable off-screen (native menu, no CDP)', true, 'not proven here');
  // (e) and (f) seeds (ambient-lead m_21810): five more artifacts so ARTIFACTS passes its cap of 5,
  // and a PR artifact whose url is 'http://': browser-only for the window (models.ts leaves: a PR,
  // an http url) but refused by main's externalUrl (no host: new URL throws), so his browser can't open
  await harness();
  await say("[[bash: for i in 1 2 3 4 5; do printf 'more %s' $i > more-$i.md && sb artifact add more-$i.md --title more-$i; done; sb artifact add http:// --kind pr --title pr-proof]]");
  // native-mac: the right bar's folding section heads and its '+ N more' / 'show less' pager are
  // gone with the right bar (the floats end on 'all … ›' links to the full pages, native-lead
  // m_47355): their two rows ('right bar: the section heads that fold…', "right bar: '+ N more'…")
  // are dropped, the PR and the maker rows below read the artifacts float
  // (f) 1a79de8a: a PR artifact's click called window.bise.openExternal and opened no panel (its tab since native-mac). Seen
  // from a logpoint on the preload's openExternal (CDP Debugger, a condition that logs and never
  // pauses); the url is refused in main, so nothing reaches his browser (gate A watches anyway)
  // its row in the artifacts float (⌥⌘2), the right bar's row before native-mac
  const prRow = page.locator('[data-testid^="app.flyout.artifacts."]').filter({ hasText: 'pr-proof' }).first();
  const prThere = await until(async () => ((await sideArt('pr-proof')) ? true : null), 20000, 1000) === true;
  const dbg = await page.context().newCDPSession(page).catch(() => null);
  const ext: string[] = [];
  let bp: { id: string; url: string; param: string | undefined } | { id?: undefined; error: string } | null = null;
  if (dbg && prThere) {
    const scripts: { scriptId: string; url: string; startLine: number; startColumn: number }[] = [];
    dbg.on('Debugger.scriptParsed', (e) => scripts.push(e));
    dbg.on('Runtime.consoleAPICalled', (e) => { const v = e.args?.[0]?.value; if (typeof v === 'string' && v.startsWith('AHD-EXT ')) ext.push(v.slice(8)); });
    await dbg.send('Runtime.enable').catch(() => {});
    await dbg.send('Debugger.enable').catch(() => {});
    await page.waitForTimeout(800);
    for (const s of scripts) {
      if (bp) break;
      const src = (await dbg.send('Debugger.getScriptSource', { scriptId: s.scriptId }).catch(() => null))?.scriptSource ?? '';
      const m = /openExternal:\s*async\s*\(?(\w+)\)?\s*=>\s*/.exec(src);
      if (!m) continue;
      const at = m.index + m[0].length;
      const head = src.slice(0, at);
      const line = head.split('\n').length - 1;
      const col = at - (head.lastIndexOf('\n') + 1);
      bp = await dbg.send('Debugger.setBreakpoint', {
        location: { scriptId: s.scriptId, lineNumber: line + (s.startLine ?? 0), columnNumber: col + (line === 0 ? (s.startColumn ?? 0) : 0) },
        condition: `console.log('AHD-EXT ' + String(${m[1]})), false`,
      }).then((r) => ({ id: r.breakpointId, url: s.url, param: m[1] }), (e) => ({ error: String(e).slice(0, 200) }));
    }
  }
  let pr: {
    row: boolean; logpoint: typeof bp; leaves?: string | null; external_calls?: string[]; panel?: number;
    thread_before?: string | null; thread_after?: string | null;
  } = { row: prThere, logpoint: bp };
  if (prThere) {
    pr.leaves = await prRow.innerText({ timeout: 2000 }).catch(() => null);
    const screenBefore = await page.locator('[data-testid="thread.screen"]').first().getAttribute('data-agent').catch(() => null);
    await click('the PR artifact on the right side', prRow);
    await page.waitForTimeout(1500);
    pr = { ...pr, external_calls: [...ext], panel: await page.locator('[data-testid^="app.panel.tab.artifact:"][data-testid$="/pr-proof"]').count(), thread_before: screenBefore,
      thread_after: await page.locator('[data-testid="thread.screen"]').first().getAttribute('data-agent').catch(() => null) };
    await shot('pr-artifact-click');
  }
  if (dbg) {
    if (bp?.id) await dbg.send('Debugger.removeBreakpoint', { breakpointId: bp.id }).catch(() => {});
    await dbg.send('Debugger.disable').catch(() => {});
    await dbg.detach().catch(() => {});
  }
  // native-mac: a PR opens as a tab of the panel (FlyoutLists' row: openTab, its ↗ goes out), so a
  // click never calls openExternal (the logpoint must be set and silent) and the thread stays
  const called = !!bp?.id && pr.external_calls?.length === 0;
  step('a PR artifact: a click opens its panel tab, never his browser (no openExternal, seen in the preload)', called && pr.panel === 1 && pr.thread_after === pr.thread_before, pr);
  await closeArtifact();
  await page.locator('[data-testid^="app.panel.tab.artifact:"][data-testid$="/pr-proof.close"]').first().click({ timeout: 2000 }).catch(() => {});
  // (g) bae706f4: the maker dropped (its own artifact made), the artifact opened from the right bar,
  // the maker's head beside it at 1100: it ends at its column, never under the panel, restore whole
  // inside it, one line
  // the maker's artifact in the artifacts float (⌥⌘2; no cap to open since native-mac)
  const makerArt = page.locator('[data-testid^="app.flyout.artifacts."]').filter({ hasText: 'rel-ts-notes' }).first();
  const expand = async () => { await sideArt('rel-ts-notes'); };
  const made = await until(async () => ((await sideArt('rel-ts-notes')) ? true : null), 60000, 1000) === true;
  await closeFloat('artifacts');
  await say(`[[bash: sb drop ${MAKER}]]`);
  // a drop card's options: the composer's dock (composer.opt) before native-mac, an OptButton's
  // <card>.opt.<n> since
  const dropCard = page.locator('[data-testid^="dock.opt"], [data-testid^="composer.opt"], [data-testid*=".opt."]').filter({ hasText: /yes|drop/i }).first();
  const dropAsked = await dropCard.waitFor({ timeout: 20000 }).then(() => true, () => false);
  if (dropAsked) await click(`answer ${MAKER}'s drop card 'yes'`, dropCard);
  // archived: no live row of it on the right side; in the coins' form, also in the agents float's
  // archived fold (app.flyout.agents.archived). The outer card lists the live ones only: the
  // archived thread itself is read in the row below (thread.archived)
  const archived = await until(async () => {
    if (!(await openFloat(1, 'agents'))) return null;
    const live = await page.evaluate((m) => [...document.querySelectorAll('[data-testid^="app.flyout.agents."][data-testid$=".name"], [data-testid^="app.outer.agents."][data-testid$=".name"]')]
      .filter((e) => !e.closest('[data-testid="app.flyout.agents.archived.rows"]') && (e.textContent ?? '').includes(m)).length, MAKER).catch(() => 1);
    if (live > 0) return null;
    if (await page.locator('[data-testid="app.outer"]').count()) return true;
    const fold = page.locator('[data-testid="app.flyout.agents.archived"]').first();
    if ((await fold.getAttribute('aria-expanded').catch(() => null)) === 'false') await fold.click({ timeout: 2000 }).catch(() => {});
    return /release-ts/.test((await text('[data-testid="app.flyout.agents.archived.rows"]')) ?? '') ? true : null;
  }, 60000, 1000);
  await closeFloat('agents');
  type Box = { x: number; right: number; w: number; h: number } | null;
  let gh: {
    made: boolean; drop_card: boolean; archived: boolean; open?: boolean; agent?: string | null; head?: Box; column_right?: number | null;
    art?: Box; restore?: Box; restore_text?: string | null; restore_cut?: boolean | null; width?: number;
  } = { made, drop_card: dropAsked, archived: !!archived };
  // native-mac: the artifact opens in the panel's tab (artifact.page) beside the thread he is on (it
  // no longer brings its maker's thread). The maker's thread is opened from the agents float's
  // archived fold (the panel open, the side's minis), as he would; archived, it says so and offers
  // restore on its own line (ThreadScreen's History: thread.archived, thread.restore), the head's
  // restore button before
  if (made && archived) {
    await expand();
    await click(`${MAKER}'s artifact from the right side`, makerArt);
    let open = await page.locator('[data-testid="artifact.page"]').first().waitFor({ timeout: 15000 }).then(() => true, () => false);
    if (open && await openFloat(1, 'agents')) {
      const fold = page.locator('[data-testid="app.flyout.agents.archived"]').first();
      if ((await fold.getAttribute('aria-expanded').catch(() => null)) === 'false') await fold.click({ timeout: 2000 }).catch(() => {});
      const old = page.locator('[data-testid="app.flyout.agents.archived.rows"] [data-row]').filter({ hasText: MAKER }).first();
      await click(`${MAKER}'s archived row in the agents float`, old);
      await page.locator(`[data-testid="thread.screen"][data-agent="${MAKER}"]`).first().waitFor({ timeout: 8000 }).catch(() => {});
      open = await page.locator('[data-testid="artifact.page"]').first().isVisible().catch(() => false);
    }
    // two widths (native-lead m_47958): at >= 1176 the panel sits beside the thread, so restore
    // stays inside its column, left of the panel, never under it (1440); under 1176 the panel is a
    // sheet over the thread (designer m_44867's '1100 sheet'): esc closes the artifact, then
    // restore is whole and clickable inside its column (1100)
    const vh2 = await page.evaluate(() => window.innerHeight);
    const measure = () => page.evaluate(() => {
      const b = (s: string) => { const e = document.querySelector(s); if (!e) return null; const r = e.getBoundingClientRect(); return { x: Math.round(r.x), right: Math.round(r.right), w: Math.round(r.width), h: Math.round(r.height) }; };
      const restore = document.querySelector('[data-testid="thread.restore"]');
      const rb = restore?.getBoundingClientRect();
      // what a click at restore's middle reaches: restore itself, never a panel over it
      const hit = rb ? document.elementFromPoint(rb.x + rb.width / 2, rb.y + rb.height / 2) : null;
      return {
        agent: document.querySelector('[data-testid="thread.screen"]')?.getAttribute('data-agent') ?? null,
        head: b('[data-testid="thread.archived"]'), column_right: b('[data-testid="app.main"]')?.right ?? null,
        art: b('[data-testid="artifact.page"]'), restore: b('[data-testid="thread.restore"]'), restore_text: restore?.textContent?.trim() ?? null,
        restore_cut: restore ? restore.scrollWidth > restore.clientWidth : null, clickable: !!restore && !!hit && restore.contains(hit), width: window.innerWidth,
      };
    }).catch(() => null);
    await cdp.send('Emulation.setDeviceMetricsOverride', { width: 1440, height: vh2, deviceScaleFactor: 0, mobile: false }).catch(() => {});
    await page.waitForTimeout(800);
    await page.mouse.move(1, 400);
    const wide = await measure();
    await shot('archived-maker-1440');
    await cdp.send('Emulation.setDeviceMetricsOverride', { width: 1100, height: vh2, deviceScaleFactor: 0, mobile: false }).catch(() => {});
    await page.waitForTimeout(800);
    const sheet = { art_open: await page.locator('[data-testid="artifact.page"]').first().isVisible().catch(() => false) };
    // esc from the window, the focus in no field and not in the page's frame (a click on the
    // thread under the sheet lands in the artifact's frame); else the sheet's ✕ (its tab's close)
    await page.evaluate(() => { (document.activeElement as HTMLElement | null)?.blur?.(); }).catch(() => {});
    await page.keyboard.press('Escape');
    const byEsc = await page.locator('[data-testid="artifact.page"]').first().waitFor({ state: 'detached', timeout: 5000 }).then(() => true, () => false);
    const byX = byEsc ? null : await page.locator(ART_CLOSE).first().click({ timeout: 3000 })
      .then(() => page.locator('[data-testid="artifact.page"]').first().waitFor({ state: 'detached', timeout: 5000 })).then(() => true, () => false);
    const closedAt1100 = byEsc || byX === true;
    Object.assign(sheet, { by_esc: byEsc, by_x: byX });
    // escape.ts's step 7/8 must run with nothing in focus too (native-lead m_48021: a bug today,
    // its own row so the maker's row stays green on the sheet's ✕)
    step('esc with nothing in focus closes an artifact sheet at 1100', byEsc, { by_esc: byEsc, by_x: byX });
    await page.mouse.move(1, 400);
    const narrow = await measure();
    await shot('archived-maker-1100');
    gh = { ...gh, open, agent: narrow?.agent ?? wide?.agent ?? null, wide, sheet: { ...sheet, esc_closed: closedAt1100 }, narrow } as typeof gh;
    await cdp.send('Emulation.clearDeviceMetricsOverride').catch(() => {});
    await closeArtifact();
  }
  type Seen = { head: Box; restore: Box; column_right: number | null; art: Box; restore_cut: boolean | null; clickable: boolean; width: number } | null | undefined;
  const g2 = gh as unknown as { wide?: Seen; narrow?: Seen; sheet?: { esc_closed: boolean } };
  const whole = (s: Seen) => !!s && !!s.head && !!s.restore && s.column_right != null && s.head.right <= s.column_right + 1
    && s.restore.right <= s.head.right + 1 && s.restore_cut === false && s.restore.h < 40 && s.clickable;
  const besideOk = whole(g2.wide) && g2.wide?.width === 1440 && !!g2.wide?.art && !!g2.wide?.restore && g2.wide.restore.right <= g2.wide.art.x + 1;
  const sheetOk = !!g2.sheet?.esc_closed && whole(g2.narrow) && g2.narrow?.width === 1100 && !g2.narrow?.art;
  step("an archived maker beside its artifact: restore inside its column beside the panel at 1440; at 1100 (the panel a sheet) esc closes it and restore is whole and clickable",
    gh.agent === MAKER && besideOk && sheetOk, gh);
  await harness();
  await fileChips(harness);
}

// quick: the quick rows alone, a follow-up pass on a candidate (cand.sh AS_HE_DOES_ONLY=quick)
if (mode === 'quick') {
  const composerOf = () => page.locator('[data-testid="composer"] [contenteditable="true"], [data-testid="composer"] textarea').first();
  const say = async (words: string) => {
    if (!(await click(`say ${words.slice(0, 30)}`, composerOf()))) return false;
    await page.keyboard.press('Meta+a');
    await page.keyboard.press('Backspace');
    await page.keyboard.type(words, { delay: 5 });
    await page.keyboard.press('Enter');
    return true;
  };
  const harness = () => openProject('harness');
  await closeArtifact();
  // The rows left after the e2e cull (docs/test-plan-min.md): the cross-hub answers, each group with
  // its seed. rowsE812 first: the PR poller gets the time the other takes. The other groups went:
  // each of their rows is a desktop check's transform line now, or on the LOST (visual) list.
  const picked = [rowsE812, rowsF2A7];
  // AS_HE_DOES_GROUPS: those groups alone, in that order
  const groups = GROUPS.length ? picked.filter((g) => GROUPS.includes(g.name)) : picked;
  // the pass limit sized to its groups: GROUP_MS each from here (the stall watchdog still ends a
  // hang at 600 s with no step); AS_HE_DOES_WATCHDOG_MS set by hand wins. Measured (review-gates
  // m_29770): 323 s a group on average, 839 s the longest, so 600 s a group is a margin of ~1.9x
  const GROUP_MS = 600000;
  if (process.env.AS_HE_DOES_WATCHDOG_MS === undefined) passLimit(Date.now() - T0 + groups.length * GROUP_MS);
  for (const g of groups) { GROUP = g.name; await g(harness, say); }
  GROUP = null;
}

// 0170ed3f's rows (main m_23135, agent-waiting-states A f5a0394a/7d97d7a2/d6ece40b, the switcher's
// silence a38108e7, the route's paths 0170ed3f), on the real hubs of the world, seeded by one
// message to harness' main: zqbench is paused on a pid the driver started (sb wake --on-exit
// --then), zqsearch waits on zqdocs' reply (an sb ask whose question holds a [[slow]] marker, built
// by printf so no message carries it whole: zqdocs answers 240 s later), main sets an alarm (a file
// watch). Rows: the pause over the composer and in the head, the reply line and its name, the
// alarm only in the right bar (hover words, detail, stop watching), never 'since', and a path that
// holds a project's name stays in bise's thread. Each row is its own try; the pid is killed at the end.
// e812463c's rows (main m_23495): the right bar's ARCHIVED starts folded and an opened one stays
// open after a reload (c4665615); a card answered from another project's thread never comes back
// through a project switch and a reload (store/acted.ts); a merge-ready card's '2 open it on GitHub'
// opens the PR, sends nothing, the card stays and nothing says 'you answered' (his #384, localAct).
// The merge card is the hub's own: the world's GitHub (cand.sh: origin on github.com, the fake gh
// first on the PATH) lists PR #384 on zqmerge's branch, CLEAN and approved, once its place exists.
// '2' calls window.bise.openExternal: a breakpoint in the preload reads its URL and swaps it for
// 'qa-noop:held' (main refuses that scheme), so his browser never opens; a swap that doesn't take
// stops the renderer's call (Runtime.terminateExecution) and the run (BREACH).
async function rowsE812(harness: () => Promise<unknown>, say: (words: string) => Promise<unknown>) {
  const row = async (name: string, fn: () => Promise<unknown>) => {
    if (!wanted(name)) return;
    try { await fn(); } catch (e) { step(name, false, String(e).slice(0, 300)); }
  };
  const flat = (s: string | null | undefined) => (s ?? '').replace(/\s+/g, ' ').trim();
  const bise = () => openBise();
  const HP = process.env.AS_HE_DOES_HARNESS ?? null;
  const GHW = process.env.AS_HE_DOES_GH_WORLD ?? null;
  const git = async (args: string[]) => {
    const { execFileSync } = await import('node:child_process');
    try { return execFileSync('git', ['-C', HP ?? '.', ...args], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim(); } catch { return null; }
  };
  // the projects of the sidebar but bise and harness: {id, name}
  const others = () => page.evaluate(() => [...document.querySelectorAll('[data-testid^="app.side.project."]')]
    .map((e) => ({ id: (e.getAttribute('data-testid') ?? '').slice('app.side.project.'.length), name: (e.querySelector('.nm, .app-side-name, .name')?.textContent ?? e.textContent ?? '').replace(/\s+/g, ' ').trim() }))
    .filter((p) => p.id && !p.id.includes('.') && !/^\W*harness\b/.test(p.name))).catch(() => []);
  const openProject = async (id: string) => {
    await page.locator(`[data-testid="app.side.project.${id}"]`).first().click({ timeout: 5000 }).catch(() => {});
    await page.waitForTimeout(800);
  };
  // a card's words anywhere a card shows (the inbox block, a strip, the dock), never his asides
  // a read that fails (a reload under it) counts as seen: a red to look at, never a silent pass
  const cardShown = (token: string) => page.evaluate((t) => [...document.querySelectorAll('[data-testid^="inbox."], [data-testid^="strip."], [data-testid^="dock."]')]
    .some((e) => (e.textContent ?? '').includes(t)), token).catch(() => 'unread');
  // the block's row or card holding `token`, opened (a click on its row, '+ N more' first)
  const dock = async (token: string, ms: number) => {
    const rowL = page.locator('[data-testid^="inbox.block.row."]', { hasText: token }).first();
    const open = page.locator('[data-testid^="inbox.block.card."]').filter({ hasText: token }).first();
    const listed = await until(async () => {
      if ((await rowL.isVisible().catch(() => false)) || (await open.isVisible().catch(() => false))) return true;
      await page.locator('[data-testid="inbox.block.more"]').first().click({ timeout: 1000 }).catch(() => {});
      return null;
    }, ms, 1000);
    for (let i = 0; listed && i < 20 && !(await open.isVisible().catch(() => false)); i++) {
      if (await rowL.isVisible().catch(() => false)) await rowL.click({ timeout: 2000 }).catch(() => {});
      else await page.locator('[data-testid="inbox.block.more"]').first().click({ timeout: 2000 }).catch(() => {});
      await page.waitForTimeout(400);
    }
    return { listed: !!listed, open: (await open.isVisible().catch(() => false)) ? open : null };
  };
  const answeredAsides = (token: string) => page.evaluate((t) => [...document.querySelectorAll('[data-testid^="thread.aside."], [data-testid^="thread.approval."]')]
    .filter((e) => /^thread\.(aside|approval)\.\d+$/.test(e.getAttribute('data-testid') ?? '') && /you answered/.test(e.textContent ?? '') && (e.textContent ?? '').includes(t))
    .map((e) => (e.textContent ?? '').replace(/\s+/g, ' ').trim().slice(0, 200)), token).catch(() => ['unread']);
  const fakeLines = (token: string) => {
    const LOG = process.env.AS_HE_DOES_FAKE_LOG;
    return LOG && fs.existsSync(LOG) ? fs.readFileSync(LOG, 'utf8').split('\n').filter((l) => l.includes(token)).length : null;
  };

  // the seed, in harness' thread: zqmerge (its place's branch carries the PR), zqold dropped (ARCHIVED)
  await harness();
  // --place new: its own worktree on sb/zqmerge (the world's agents share the workspace otherwise,
  // so no branch for a PR: try-gate-e812's first run)
  await say("[[bash: sb spawn zqmerge --place new --objective 'zq ts-pattern error dx: nothing to do yet' && sb spawn zqold --objective 'zq old: nothing to do' && sleep 3 && sb drop zqold]] seeded e812");
  const tip = HP ? await until(async () => git(['rev-parse', '--verify', '-q', 'refs/heads/sb/zqmerge']), 60000, 1000) : null;
  if (tip && GHW) {
    fs.writeFileSync(path.join(GHW, 'prs-harness.json'), JSON.stringify({ owner: 'qa-world', name: 'harness', prs: [{
      number: 384, branch: 'sb/zqmerge', head: tip, title: 'zq ts-pattern error dx', draft: false, review: 'APPROVED', rollup: 'SUCCESS',
      checks: [{ name: 'ci', conclusion: 'SUCCESS' }], reviews: [{ state: 'APPROVED', author: 'alice' }], merge_state: 'CLEAN', commits: 1,
    }] }, null, 1));
  }
  step("rowsE812 seed: zqmerge's place (branch sb/zqmerge) and PR #384 in the world's GitHub", !!tip && !!GHW, { harness: HP, gh_world: GHW, tip });


  await row('card answered from another project, then a switch and a reload', async () => {
    const [other] = await others();
    const T = `zqcard${String(Date.now() % 100000)}`;
    await harness();
    // harness' main idle first: the seed's turn and its agents' acks can hold the card's words
    // past the wait (try-gate-e812's third run: no card in 90 s)
    await until(async () => (/· idle\b/.test(flat(await text('[data-testid="composer"]'))) ? true : null), 90000, 1000);
    const f = page.locator('[data-testid="composer.field"]:visible').first();
    await f.click({ timeout: 3000 }).catch(() => {});
    await page.keyboard.press('Meta+a');
    await page.keyboard.press('Backspace');
    await page.keyboard.type(`[[bash: sb notify '${T} which size?`, { delay: 5 });
    for (const l of ['1. zqsmall', "2. zqlarge']]"]) { await page.keyboard.press('Shift+Enter'); await page.keyboard.type(l, { delay: 5 }); }
    await page.keyboard.press('Enter');
    // inbox-scope (e76af4925): a project's block lists that project's items only; another
    // project's card is answered from bise's view (or the inbox screen), every project's
    await bise();
    const d = await dock(T, 90000);
    if (d.open) await d.open.locator('[data-testid$=".opt.1"]').first().click({ timeout: 3000 }).catch(() => {});
    const gone = d.open ? await until(async () => (!(await cardShown(T)) ? true : null), 10000, 250) : null;
    await page.mouse.move(1, 300);
    await shot('card-answered-other-project');
    // from here on, sampled every 250 ms: the card must never show again
    const seen: { where: string; at_ms: number }[] = [];
    const watch = async (where: string, ms: number) => {
      const t0 = Date.now();
      while (Date.now() - t0 < ms) { if (await cardShown(T)) seen.push({ where, at_ms: Date.now() - t0 }); await page.waitForTimeout(250); }
    };
    await watch('bise', 3000);
    await harness();
    await watch('harness (a project switch)', 6000);
    await bise();
    await watch('bise (a project switch)', 4000);
    await page.reload().catch(() => {});
    await page.locator('[data-testid="app.sidebar"]').first().waitFor({ timeout: 20000 }).catch(() => {});
    await watch('bise after a reload', 8000);
    if (other) { await openProject(other.id); await watch(`${other.name} after the reload`, 4000); }
    await harness();
    await watch('harness after the reload', 4000);
    await page.mouse.move(1, 300);
    await shot('card-never-back');
    step(`a card of harness answered from bise's view: gone, never back through a project switch (${other ? other.name : 'no other project'}, harness, bise) and a reload (sampled every 250 ms, ~30 s)`,
      d.listed && !!d.open && !!gone && seen.length === 0, { other: other ?? null, listed: d.listed, opened: !!d.open, gone: !!gone, seen: seen.slice(0, 10), asides: await answeredAsides('zqsmall') });
  });

  await harness();
}

// f2a72fd8's rows (ambient-lead m_23555, card #509): every way he answers an inbox card (a digit from
// another project, words from bise's view, a click in the asker's project) leaves '✓ you answered
// <agent> · <project>' where he is, from the hub's AnswerResult, kept across a switch and a reload;
// a refused answer and a refused close bring that card back with the words (the harness hub held
// with SIGSTOP past the core's 30 s ASK_LATE: the core's own refusal), and an answer to a card the
// hub withdrew meanwhile gets the hub's own 'isn't open' (the candidate's ambient-core held with
// SIGSTOP while main withdraws it); a pause under a minute counts its seconds on the line, the head
// and the AGENTS row, then minutes. Every SIGSTOP is the world's or the candidate's own pid, always
// SIGCONT'd. Each row is its own try.
async function rowsF2A7(harness: () => Promise<unknown>, say: (words: string) => Promise<unknown>) {
  const row = async (name: string, fn: () => Promise<unknown>) => {
    if (!wanted(name)) return;
    try { await fn(); } catch (e) { step(name, false, String(e).slice(0, 300)); }
  };
  const flat = (s: string | null | undefined) => (s ?? '').replace(/\s+/g, ' ').trim();
  const bise = () => openBise();
  const field = page.locator('[data-testid="composer.field"]:visible').first();
  const clear = async () => {
    await field.click({ timeout: 3000 }).catch(() => {});
    await page.keyboard.press('Meta+a');
    await page.keyboard.press('Backspace');
  };
  const APP = process.env.AS_HE_DOES_APP ?? null;
  const { execFileSync, spawn } = await import('node:child_process');
  const psLines = () => { try { return execFileSync('/bin/ps', ['-axww', '-o', 'pid=,command='], { encoding: 'utf8' }).split(String.fromCharCode(10)); } catch { return []; } };
  const pidOf = (l: string) => Number(l.trim().split(/\s+/)[0]);
  // the candidate's own ambient-core (under its bundle)
  const coreLines = () => (APP ? psLines().filter((l) => l.includes(`${APP}/Contents/`) && / ambient-core( |$)/.test(l)) : []);
  const signal = (pids: number[], s: NodeJS.Signals) => pids.forEach((p) => {
    if (s === 'SIGSTOP') HELD.add(p); else HELD.delete(p);
    try { process.kill(p, s); } catch { /* gone */ }
  });
  const others = () => page.evaluate(() => [...document.querySelectorAll('[data-testid^="app.side.project."]')]
    .map((e) => ({ id: (e.getAttribute('data-testid') ?? '').slice('app.side.project.'.length), name: (e.querySelector('.nm, .app-side-name, .name')?.textContent ?? e.textContent ?? '').replace(/\s+/g, ' ').trim() }))
    .filter((p) => p.id && !p.id.includes('.') && !/^\W*harness\b/.test(p.name))).catch(() => []);
  const openProject = async (id: string) => {
    await page.locator(`[data-testid="app.side.project.${id}"]`).first().click({ timeout: 5000 }).catch(() => {});
    await page.waitForTimeout(800);
  };
  const reload = async () => {
    await page.reload().catch(() => {});
    await page.locator('[data-testid="app.sidebar"]').first().waitFor({ timeout: 20000 }).catch(() => {});
    await page.waitForTimeout(1000);
  };
  // a card's words anywhere a card shows (a failed read counts as shown: a red to look at)
  const cardShown = (token: string) => page.evaluate((t) => [...document.querySelectorAll('[data-testid^="inbox."], [data-testid^="strip."], [data-testid^="dock."]')]
    .some((e) => (e.textContent ?? '').includes(t)), token).catch(() => 'unread');
  // the card holding `token` anywhere in the block: its words, else a walk through '+ N more' (each
  // opens the next item; a folded card's words aren't in the block, try-gate-f2a7's second run)
  const present = async (token: string) => {
    const seen = await cardShown(token);
    if (seen !== false) return true;
    const n = Number(/(\d+) waiting/.exec(flat(await text('[data-testid="inbox.block"]')))?.[1] ?? 0);
    for (let i = 0; i < n + 1; i++) {
      const m = page.locator('[data-testid="inbox.block.more"]').first();
      if (!(await m.count().catch(() => 0))) break;
      await m.click({ timeout: 1000 }).catch(() => {});
      await page.waitForTimeout(250);
      if ((await cardShown(token)) !== false) return true;
    }
    return false;
  };
  // the block's card holding `token`, open in place: {listed, open, id}
  const dock = async (token: string, ms: number) => {
    const rowL = page.locator('[data-testid^="inbox.block.row."]', { hasText: token }).first();
    const open = page.locator('[data-testid^="inbox.block.open."]').filter({ hasText: token }).first();
    const listed = await until(async () => {
      if ((await rowL.isVisible().catch(() => false)) || (await open.isVisible().catch(() => false))) return true;
      await page.locator('[data-testid="inbox.block.more"]').first().click({ timeout: 1000 }).catch(() => {});
      return null;
    }, ms, 1000);
    for (let i = 0; listed && i < 20 && !(await open.isVisible().catch(() => false)); i++) {
      if (await rowL.isVisible().catch(() => false)) await rowL.click({ timeout: 2000 }).catch(() => {});
      else await page.locator('[data-testid="inbox.block.more"]').first().click({ timeout: 2000 }).catch(() => {});
      await page.waitForTimeout(400);
    }
    const isOpen = await open.isVisible().catch(() => false);
    const id = isOpen ? await open.evaluate((el) => {
      const c = el.matches('[data-testid^="inbox.block.card."]') ? el : el.querySelector('[data-testid^="inbox.block.card."]');
      const m = /-(\d+)$/.exec(c?.getAttribute('data-testid') ?? '');
      return m ? Number(m[1]) : null;
    }).catch(() => null) : null;
    return { listed: !!listed, open: isOpen ? open : null, id };
  };
  // the rows of the thread on screen holding `words`: his asides and the hub's approval rows, each once
  const rowsWith = (words: string) => page.evaluate((w) => [...document.querySelectorAll('[data-testid="thread.list"] [data-testid^="thread."]')]
    .filter((e) => /^thread\.(aside|approval)\.\d+$/.test(e.getAttribute('data-testid') ?? '') && (e.textContent ?? '').includes(w))
    .map((e) => ({ id: e.getAttribute('data-testid') ?? '', text: (e.textContent ?? '').replace(/\s+/g, ' ').trim().slice(0, 200) })), words).catch(() => [{ id: 'unread', text: '' }]);
  const bodyHas = async (re: RegExp) => { const b = flat(await page.locator('body').innerText({ timeout: 3000 }).catch(() => '')); const m = re.exec(b); return m ? m[0] : null; };
  // the resolution the world's hub journaled for card `id` whose words hold `token` (the journal
  // whose card_opened has both): 'answered', 'withdrawn by main: …', …; null if none
  // a journal line (JSON): the fields closeOf reads
  type JournalEv = { type?: string; id?: string; resolution?: unknown; card?: { id?: string; text?: string } };
  const closeOf = (token: string, id: string) => {
    const hubs = process.env.AS_HE_DOES_BISE_HOME ? path.join(process.env.AS_HE_DOES_BISE_HOME, 'hubs') : null;
    if (!hubs) return null;
    let dirs: string[] = [];
    try { dirs = fs.readdirSync(hubs); } catch { return null; }
    for (const d of dirs) {
      let lines: string[];
      try { lines = fs.readFileSync(path.join(hubs, d, 'journal.jsonl'), 'utf8').split(String.fromCharCode(10)); } catch { continue; }
      const evs = lines.map((l): JournalEv | null => { try { return JSON.parse(l); } catch { return null; } }).filter((e) => e !== null);
      if (!evs.some((e) => e.type === 'card_opened' && e.card?.id === id && (e.card?.text ?? '').includes(token))) continue;
      const c = evs.find((e) => e.type === 'card_closed' && e.id === id);
      return c ? String(c.resolution) : null;
    }
    return null;
  };

  const T = String(Date.now() % 100000);
  const [other] = await others();
  const goOther = async () => { if (other) await openProject(other.id); else await bise(); };
  const otherName = other ? other.name : 'bise';

  // main's cards in harness, each '<question>' then its options on their own lines (Shift+Enter, as
  // rows37: sb notify is main's alone, 'reserved for main' from an agent, try-gate-f2a7's first run)
  const cards = async (list: { q: string; opts: string[] }[], tail: string) => {
    await field.click({ timeout: 3000 }).catch(() => {});
    await page.keyboard.press('Meta+a');
    await page.keyboard.press('Backspace');
    // one line, printf's \n: the composer continues a '1. …' list on Shift+Enter ('2. 2. zqlarge',
    // and '5. 1. zqclick-one' on the third card of a message: no option 1 to click, try-gate-f2a7's
    // second run)
    const NL = String.fromCharCode(92) + 'n';
    const one = (c: { q: string; opts: string[] }) => `sb notify "$(printf "%s${NL}" ${[c.q, ...c.opts].map((x) => `"${x}"`).join(' ')})"`;
    await page.keyboard.type(`[[bash: ${list.map(one).join(' && ')}]] ${tail}`, { delay: 5 });
    await page.keyboard.press('Enter');
  };
  // the seed: three cards (one per way), in harness
  await closeArtifact();
  await harness();
  await until(async () => (/· idle\b/.test(flat(await text('[data-testid="composer"]'))) ? true : null), 90000, 1000);
  await cards(['digit', 'words', 'click'].map((w) => ({ q: `zqA${w}${T} which way?`, opts: [`1. zq${w}-one`, `2. zq${w}-two`] })), 'seeded f2a7');
  // folded under '+ N more' a card's words aren't in the block: main's 'card #N opened' x3 instead
  const seeded = await until(async () => ((((await text('[data-testid="thread.list"]')) ?? '').match(/(card|notification) #\d+ opened for the user/g) ?? []).length >= 3 ? true : null), 120000, 1000);
  step("rowsF2A7 seed: main's three cards (digit, words, click) in the inbox", !!seeded, { other: other ?? null, block: flat(await text('[data-testid="inbox.block"]')).slice(0, 300) });

  const took: Record<string, unknown> = {};
  // answer-in-thread A (787e6ee82): an answer to another project's card takes the window to that
  // project's thread at his answer, the hub's row there ('answered from <where>' under his bubble);
  // no window aside in the thread he answered from (store/answered.ts)
  const harnessPid = () => page.evaluate(() => [...document.querySelectorAll('[data-testid^="app.side.project."]')]
    // its name alone (.nm, nm-frame b06021770): the row's whole text runs on into its pill ('harness3 10 unread')
    .map((e) => ({ id: (e.getAttribute('data-testid') ?? '').slice('app.side.project.'.length), t: (e.querySelector('.nm, .app-side-name, .name')?.textContent ?? e.textContent ?? '').trim() }))
    .find((p) => p.id && !p.id.includes('.') && /^\W*harness\b/.test(p.t))?.id ?? null).catch(() => null);
  const HID = await harnessPid();
  const inHarness = () => page.locator('[data-testid="thread.screen"]').first().getAttribute('data-project', { timeout: 1000 }).then((x) => x === HID, () => false);
  const hubRows = async (w: string) => (await rowsWith(w)).filter((x) => x.id.startsWith('thread.approval.'));
  const asideRows = async (w: string) => (await rowsWith(w)).filter((x) => x.id.startsWith('thread.aside.'));
  // inbox-scope (e76af4925): a project's block lists that project's items only; another project's
  // card is answered from bise's view (every project's), here by its digit
  await row('answer by a digit from bise', async () => {
    await bise();
    await clear();
    await page.mouse.move(1, 300);
    const d = await dock(`zqAdigit${T}`, 60000);
    if (d.open) { await d.open.click({ position: { x: 4, y: 4 } }).catch(() => {}); await page.keyboard.press('2'); }
    const moved = d.open ? await until(async () => ((await inHarness()) ? true : null), 20000, 500) : null;
    const rows = moved ? await until(async () => { const r = await hubRows('zqdigit-two'); return r.length ? r : null; }, 20000, 500) : null;
    const gone = !(await present(`zqAdigit${T}`));
    await page.mouse.move(1, 300);
    await shot('f2a7-digit-bise');
    took.digit = rows?.[0] ?? null;
    step("a digit ('2') on main's card from bise's view: the window goes to harness' thread at his answer, 'zqdigit-two' there once with 'answered from bise'; the card gone",
      !!moved && !!rows && rows.length === 1 && /answered from bise/.test(rows[0].text) && gone,
      { listed: d.listed, opened: !!d.open, card: d.id, moved: !!moved, rows, gone });
  });

  const SAID = `zqwords-said ${T} the small one please`;

  await row('answer by a click in the asker project', async () => {
    await harness();
    await clear();
    const d = await dock(`zqAclick${T}`, 60000);
    if (d.open) await d.open.locator('[data-testid$=".opt.1"]').first().click({ timeout: 3000 }).catch(() => {});
    const rows = d.open ? await until(async () => { const r = (await rowsWith('zqclick-one')).filter((x) => x.id.startsWith('thread.approval.')); return r.length ? r : null; }, 20000, 500) : null;
    const asides = await rowsWith('zqclick-one');
    const gone = !(await present(`zqAclick${T}`));
    await page.mouse.move(1, 300);
    await shot('f2a7-click-harness');
    took.click = rows?.[0] ?? null;
    step("a click on option 1 from harness' own thread: the hub's row 'you answered <agent>' + 'zqclick-one' there, once, no window aside; the card gone",
      !!rows && rows.length === 1 && /you answered \S+/.test(rows[0].text) && asides.filter((x) => x.id.startsWith('thread.aside.')).length === 0 && gone, { listed: d.listed, opened: !!d.open, card: d.id, rows, asides, gone });
  });



  await harness();
}

// (f1) ad753852's file chips (ambient-lead m_22068): a Finder drop (CDP's drag, the desktop checks'
// dropFiles) of a pdf, csv, xlsx, docx and pptx on harness' composer; the hub's 'you' entry; his
// bubble; a click / right-click; a file gone at send time; a csv dropped while answering a card.
// The files live in <fx>/file-chips: outside the app's home and every world project, so main's
// fence (ipc.ts lookable) refuses a preview or a reveal and nothing reaches his screen. The calls
// are seen from logpoints on the preload's quickLook / showInFinder (this build has no test mode,
// so no 'bise-desktop: quicklook' line).
/** A part of a hub message (Part::Text / Part::File), as the journal keeps it. */
type JournalPart = { kind?: string; name?: string; path?: string; missing?: boolean; text?: string };
/** A journal row that carries a word: its hub, its message's ends, parts and text. */
type JournalRow = { hub: string; type: string | null; from: string | null; to: string | null; parts: JournalPart[] | null; text: string };
async function fileChips(harness: () => Promise<unknown>) {
  const FD = path.join(FX, 'file-chips');
  fs.rmSync(FD, { recursive: true, force: true });
  fs.mkdirSync(FD, { recursive: true });
  const DOCS = ['q3.pdf', 'data.csv', 'plan.xlsx', 'brief.docx', 'deck.pptx'];
  const NL = String.fromCharCode(10);
  for (const d of DOCS) fs.writeFileSync(path.join(FD, d), `fake ${d}${NL}`);
  const at = (d: string) => path.join(FD, d);
  // the preload's window.bise (the app's home only, here)
  const appHome = await page.evaluate(() => { const w: Window & { bise?: { home?: string } } = window; return w.bise?.home ?? null; }).catch(() => null);
  const fenced = !!appHome && !FD.startsWith(`${appHome}/`);
  const drop = async (sel: string, files: string[]) => {
    const box = await page.locator(sel).first().boundingBox().catch(() => null);
    if (!box) return false;
    const x = box.x + box.width / 2, y = box.y + box.height / 2;
    const s = await page.context().newCDPSession(page);
    const data = { items: [], files, dragOperationsMask: 1 };
    for (const type of ['dragEnter', 'dragOver', 'drop'] as const) await s.send('Input.dispatchDragEvent', { type, x, y, data });
    await s.detach().catch(() => {});
    return true;
  };
  // the home hubs' journals (the world's, under the app's home): the msg rows that carry word
  const journal = (word: string) => {
    const rows: JournalRow[] = [];
    const hubs = appHome ? path.join(appHome, '.bise', 'hubs') : null;
    for (const h of hubs && fs.existsSync(hubs) ? fs.readdirSync(hubs) : []) {
      const f = path.join(hubs ?? '', h, 'journal.jsonl');
      if (!fs.existsSync(f)) continue;
      for (const l of fs.readFileSync(f, 'utf8').split(NL)) {
        if (!l.includes(word)) continue;
        try {
          // a journal line (JSON): a msg row, or the message itself
          type Msg = { type?: string; from?: string; to?: string; parts?: string | JournalPart[]; text?: unknown };
          const j: Msg & { msg?: Msg } = JSON.parse(l);
          const m = j.msg ?? j;
          const parts = typeof m.parts === 'string' ? JSON.parse(m.parts) : m.parts;
          rows.push({ hub: h, type: j.type ?? null, from: m.from ?? null, to: m.to ?? null, parts: Array.isArray(parts) ? parts : null, text: String(m.text ?? '').slice(0, 300) });
        } catch { /* not a json row */ }
      }
    }
    return rows;
  };
  const field = page.locator('[data-testid="composer.field"]:visible').first();
  const fileChip = page.locator('[data-testid^="composer.chip."][data-kind="file"]');
  const fresh = async (words: string) => {
    await field.click({ timeout: 5000 }).catch(() => {});
    await page.keyboard.press('Meta+a');
    await page.keyboard.press('Backspace');
    await page.keyboard.type(words, { delay: 5 });
  };

  // the drop: one file chip each, in the drop order
  await fresh('here are the files zqfiles ');
  const dropped = await drop('[data-testid="composer.field"]:visible', DOCS.map(at));
  const five = await until(async () => ((await fileChip.count()) === DOCS.length ? true : null), 8000, 300);
  // b46e1fbd: a chip in the text shows its number; its name is in the attachment list's row
  const listNames = () => page.locator('[data-testid="composer"] [data-testid$=".attach"] [role="listitem"][data-kind="file"] .nm').allInnerTexts().catch(() => []);
  const names = (await listNames()).map((t) => t.trim());
  await shot('file-chips-composer');
  step('(f1) a drop of a pdf, csv, xlsx, docx and pptx on the composer: one data-kind=file chip each, in order', dropped && !!five && JSON.stringify(names) === JSON.stringify(DOCS), { dropped, names, fenced, app_home: appHome });

  // deck.pptx goes before the send: the hub finds it gone, the bubble says '· missing'
  fs.rmSync(at('deck.pptx'), { force: true });
  await page.keyboard.press('Enter');
  const entry = await until(async () => journal('zqfiles').find((r) => r.parts?.some((p) => p.kind === 'file')) ?? null, 30000, 1000);
  const fparts = (entry?.parts ?? []).filter((p) => p.kind === 'file');
  step("(f1) the hub's 'you' entry carries the Part::File parts in order (his words first, deck.pptx missing)",
    !!entry && entry.parts?.[0]?.kind === 'text' && JSON.stringify(fparts.map((p) => p.path)) === JSON.stringify(DOCS.map(at))
      && fparts.every((p) => /^\[File #\d+\]$/.test(String(p.name))) && fparts.find((p) => p.path === at('deck.pptx'))?.missing === true
      && fparts.filter((p) => p.missing).length === 1,
    entry ? { ...entry, parts: entry.parts?.map((p) => ({ kind: p.kind, name: p.name, path: p.path ? path.basename(p.path) : undefined, missing: p.missing, text: p.text })) } : journal('zqfiles').slice(0, 2));
  const bubble = page.locator('[data-testid^="thread.you."][data-testid$=".parts"]').filter({ hasText: 'q3.pdf' }).last();
  const drawn = await bubble.waitFor({ timeout: 20000 }).then(() => true, () => false);
  const bchips = bubble.locator('[data-kind="file"]');
  const gone = bchips.filter({ hasText: 'deck.pptx' }).first();
  const bub = drawn ? {
    names: (await bchips.allInnerTexts().catch(() => [])).map((t) => t.replace(/[▯\s]+/g, ' ').trim()),
    gone_text: ((await gone.innerText().catch(() => '')) ?? '').trim(),
    gone_title: await gone.getAttribute('title').catch(() => null),
    q3_title: await bchips.filter({ hasText: 'q3.pdf' }).first().getAttribute('title').catch(() => null),
  } : null;
  await page.mouse.move(1, 300);
  await shot('file-chips-bubble');
  step("(f1) his bubble draws a chip per file, '· missing' on the one gone at send time",
    !!bub && bub.names.length === DOCS.length && /· missing/.test(bub.gone_text) && /deck\.pptx is gone from /.test(bub.gone_title ?? '') && /preview · right-click: show in Finder/.test(bub.q3_title ?? ''), bub);

  // a click -> quickLook(path), a right-click -> showInFinder(path), the gone one calls nothing
  const calls: string[] = [];
  const dbg = await page.context().newCDPSession(page).catch(() => null);
  const bps: { fn: string; id: string | null; error: string | null }[] = [];
  if (dbg && fenced) {
    const scripts: { scriptId: string; url: string; startLine: number; startColumn: number }[] = [];
    dbg.on('Debugger.scriptParsed', (e) => scripts.push(e));
    dbg.on('Runtime.consoleAPICalled', (e) => { const v = e.args?.[0]?.value; if (typeof v === 'string' && v.startsWith('AHD-F ')) calls.push(v.slice(6)); });
    await dbg.send('Runtime.enable').catch(() => {});
    await dbg.send('Debugger.enable').catch(() => {});
    await page.waitForTimeout(800);
    for (const s of scripts) {
      const src = (await dbg.send('Debugger.getScriptSource', { scriptId: s.scriptId }).catch(() => null))?.scriptSource ?? '';
      for (const fn of ['quickLook', 'showInFinder']) {
        const m = new RegExp(`${fn}:\\s*async\\s*\\(?(\\w+)\\)?\\s*=>\\s*`).exec(src);
        if (!m || bps.some((b) => b.fn === fn)) continue;
        const where = m.index + m[0].length;
        const head = src.slice(0, where);
        const line = head.split(NL).length - 1;
        const col = where - (head.lastIndexOf(NL) + 1);
        const r = await dbg.send('Debugger.setBreakpoint', {
          location: { scriptId: s.scriptId, lineNumber: line + (s.startLine ?? 0), columnNumber: col + (line === 0 ? (s.startColumn ?? 0) : 0) },
          condition: `console.log('AHD-F ${fn} ' + String(${m[1]})), false`,
        }).then((ok) => ({ id: ok.breakpointId, error: null }), (e: unknown) => ({ id: null, error: String(e).slice(0, 200) }));
        bps.push({ fn, id: r.id, error: r.error });
      }
    }
  }
  const ck: { fenced: boolean; logpoints: typeof bps; calls?: string[]; gone_calls?: number; right?: unknown } = { fenced, logpoints: bps };
  if (drawn && fenced && bps.filter((b) => b.id).length === 2) {
    await bchips.filter({ hasText: 'q3.pdf' }).first().click({ timeout: 5000 }).catch(() => {});
    await until(async () => (calls.some((c) => c.startsWith('quickLook ')) ? true : null), 5000, 200);
    // the click's answer settles first: main fences quickLook out, so it answers false and q3's
    // chip redraws as gone ('· missing'), which moves data.csv; a right-click sent at once missed
    // (native-lead, 2026-10-28: red 3 times with no delay, green with this settle)
    await bchips.filter({ hasText: 'q3.pdf' }).first().filter({ hasText: 'missing' }).waitFor({ timeout: 2000 }).catch(() => {});
    await page.waitForTimeout(300);
    // what the right-click meets (the row was red with no showInFinder and no word why): the
    // chip's box, the element at its centre, the contextmenu events the page saw, the click's error
    const csv = bchips.filter({ hasText: 'data.csv' }).first();
    await page.evaluate(() => {
      const w = window as unknown as { __ahdCtx?: unknown[] };
      w.__ahdCtx = [];
      document.addEventListener('contextmenu', (e) => {
        const t = e.target as Element | null;
        const at = t?.closest('[data-testid]');
        (w.__ahdCtx as unknown[]).push({ tag: t?.tagName ?? null, testid: at?.getAttribute('data-testid') ?? null, cls: (t as HTMLElement | null)?.className ?? null });
      }, { capture: true, once: false });
    }).catch(() => {});
    const box = await csv.boundingBox().catch(() => null);
    const hit = box === null ? null : await page.evaluate(([x, y]) => {
      const el = document.elementFromPoint(x, y);
      const at = el?.closest('[data-testid]');
      return { tag: el?.tagName ?? null, testid: at?.getAttribute('data-testid') ?? null, cls: (el as HTMLElement | null)?.className ?? null };
    }, [box.x + box.width / 2, box.y + box.height / 2]).catch((e: unknown) => String(e).slice(0, 200));
    const err = await csv.click({ button: 'right', timeout: 5000 }).then(() => null, (e: unknown) => String(e).slice(0, 400));
    await until(async () => (calls.some((c) => c.startsWith('showInFinder ')) ? true : null), 5000, 200);
    const seen = await page.evaluate(() => (window as unknown as { __ahdCtx?: unknown[] }).__ahdCtx ?? null).catch(() => null);
    ck.right = { box, hit, err, seen, bise_has: await page.evaluate(() => typeof (window as unknown as { bise?: { showInFinder?: unknown } }).bise?.showInFinder).catch(() => null) };
    const n0 = calls.length;
    await gone.click({ timeout: 5000 }).catch(() => {});
    await gone.click({ button: 'right', timeout: 5000 }).catch(() => {});
    await page.waitForTimeout(800);
    ck.calls = [...calls];
    ck.gone_calls = calls.length - n0;
  }
  for (const b of bps) if (b.id && dbg) await dbg.send('Debugger.removeBreakpoint', { breakpointId: b.id }).catch(() => {});
  if (dbg) { await dbg.send('Debugger.disable').catch(() => {}); await dbg.detach().catch(() => {}); }
  step('(f1) a click on a file chip calls quickLook(its path), a right-click showInFinder(its path), the gone one calls nothing (seen in the preload; main fences them out)',
    (ck.calls ?? []).includes(`quickLook ${at('q3.pdf')}`) && (ck.calls ?? []).includes(`showInFinder ${at('data.csv')}`) && ck.gone_calls === 0, ck);

  // a csv dropped while answering a card: harness' main asks, bise's inbox block docks it
  const csv = at('bench-zq.csv');
  fs.writeFileSync(csv, ['run,ms', 'cold,412', ''].join(NL));
  const Q = 'which bench zqbench?';
  await fresh(`[[bash: sb notify '${Q}`);
  for (const l of ['1. cold', "2. warm']]"]) { await page.keyboard.press('Shift+Enter'); await page.keyboard.type(l, { delay: 5 }); }
  await page.keyboard.press('Enter');
  await openBise();
  const row = page.locator('[data-testid^="inbox.block.row."]', { hasText: 'zqbench' }).first();
  const card = page.locator('[data-testid^="inbox.block.card."]').filter({ hasText: 'zqbench' }).first();
  // the block draws its first items only: an item under '+ N more' is listed too (ambient-lead m_22076),
  // so each poll that sees no zqbench opens the next ones
  const listed = await until(async () => {
    if ((await row.isVisible().catch(() => false)) || (await card.isVisible().catch(() => false)) || /zqbench/.test(((await text('[data-testid="inbox.block"]')) ?? ''))) return true;
    await page.locator('[data-testid="inbox.block.more"]').first().click({ timeout: 1000 }).catch(() => {});
    return null;
  }, 90000, 1000);
  for (let i = 0; listed && i < 40 && !(await card.isVisible().catch(() => false)); i++) {
    if (await row.isVisible().catch(() => false)) await row.click({ timeout: 2000 }).catch(() => {});
    else await page.locator('[data-testid="inbox.block.more"]').first().click({ timeout: 2000 }).catch(() => {});
    await page.waitForTimeout(500);
  }
  const docked = await card.isVisible().catch(() => false);
  const answering = docked && /answering/.test(((await page.locator('[data-testid="composer"]').first().innerText({ timeout: 1000 }).catch(() => '')) ?? ''));
  let ans: { listed: boolean; docked: boolean; answering: boolean; chip?: boolean; entry?: unknown; ok?: boolean } = { listed: !!listed, docked, answering };
  if (answering) {
    await fresh('cold, see ');
    await drop('[data-testid="composer.field"]:visible', [csv]);
    const chip = await until(async () => ((await fileChip.count()) === 1 && (await listNames()).join() === 'bench-zq.csv' ? true : null), 8000, 300);
    await shot('file-chip-card-answer');
    await page.keyboard.press('Enter');
    // an answer reaches main as the hub's own words ('the user answered card #N (…): "…"'), the file
    // in them as its [name](path) link where he dropped it (cand-2760, file-mention-wire), or as a file part
    const asPart = (r: JournalRow) => r.parts?.some((p) => p.kind === 'file' && p.path === csv);
    // the hub says 'notification #N' since cards became notifications in its words
    const asTag = (r: JournalRow) => /answered (card|notification)/.test(r.text) && r.text.includes(`cold, see [${path.basename(csv)}](${csv})`);
    const got = await until(async () => journal('bench-zq.csv').find((r) => asPart(r) || asTag(r)) ?? null, 30000, 1000);
    ans = { ...ans, chip: !!chip, entry: got ? { ...got, parts: got.parts?.map((p) => ({ kind: p.kind, name: p.name, path: p.path ? path.basename(p.path) : undefined, text: p.text })) ?? null } : journal('bench-zq.csv').slice(0, 2) };
    ans.ok = !!chip && !!got && (asTag(got) || (got.parts?.[0]?.kind === 'text' && /cold, see/.test(got.parts?.[0]?.text ?? '')));
  }
  step("(f1) a csv dropped while answering a card: a file chip, sent as a file part where it was", !!ans.ok, ans);
  await harness();
}

// the draft across a restart (ambient-lead m_19525): typed in harness' composer, never sent; after
// the restart the same composer must hold it again
if (mode === 'draft-type' || mode === 'draft-check') {
  await openProject('harness');
  const composer = page.locator('[data-testid="composer"] [contenteditable="true"], [data-testid="composer"] textarea').first();
  await composer.waitFor({ timeout: 15000 }).catch(() => {});
  const holds = async () => ((await composer.innerText({ timeout: 2000 }).catch(() => '')) || (await composer.inputValue({ timeout: 500 }).catch(() => ''))).trim();
  // 516a4e79: the terminal panel left open across a restart starts its shell, never 'no folder for
  // this project' (it waits for the core's projects rows). Opened in the pause pass, read after it.
  const term = page.locator(TERM_SEL).first();
  const termMark = path.join(out, 'term-open.json');
  if (mode === 'draft-type' && !tag.endsWith('atonce')) {
    if (!(await term.isVisible().catch(() => false))) await page.keyboard.press('Control+Backquote');
    const open = await term.waitFor({ timeout: 8000 }).then(() => true, () => false);
    const state = open ? await until(async () => { const s = await page.locator('[data-testid="terminal.screen"]').first().getAttribute('data-state').catch(() => null); return s && s !== 'opening' ? s : null; }, 10000, 300) : null;
    if (open) fs.writeFileSync(termMark, JSON.stringify({ state }));
    await shot('terminal-open-before-restart');
    step('the terminal panel is open before the restart (⌃`)', open, { state });
  }
  if (mode === 'draft-check' && fs.existsSync(termMark) && !tag.endsWith('atonce')) {
    // native-mac: the panel keeps its tabs across a restart (panel/model.ts parseKept); the shell's
    // tab may come back behind another one: its tab is clicked to show it, as he would
    // tab may come back behind another one, and the panel itself closed (it floated over the thread
    // in the 1180 window before the restart): ⌥⌘B shows the panel, then its tab is clicked, as he would
    const termTab = page.locator('[data-testid^="app.panel.tab.term:"]:not([data-testid$=".close"])').first();
    await page.locator('[data-testid="app.main"]').first().waitFor({ timeout: 15000 }).catch(() => {});
    if (!(await term.isVisible().catch(() => false)) && !(await page.locator('[data-testid="app.panel"]').first().isVisible().catch(() => false))) {
      await page.locator('[data-testid="app.main"]').first().click({ position: { x: 40, y: 140 } }).catch(() => {});
      await page.keyboard.press('Alt+Meta+b');
    }
    if (!(await term.isVisible().catch(() => false)) && await termTab.waitFor({ timeout: 15000 }).then(() => true, () => false)) await termTab.click({ timeout: 3000 }).catch(() => {});
    const shown = await term.waitFor({ timeout: 15000 }).then(() => true, () => false);
    const state = shown ? await until(async () => { const s = await page.locator('[data-testid="terminal.screen"]').first().getAttribute('data-state').catch(() => null); return s && s !== 'opening' ? s : null; }, 20000, 300) : null;
    const failed = ((await text('[data-testid="terminal.failed"]')) ?? '').slice(0, 200);
    await shot('terminal-after-restart');
    step("the terminal panel kept open across the restart starts its shell (no 'no folder')", shown && state === 'live' && !/no folder/.test(failed), { shown, state, failed: failed || null });
  }
  if (mode === 'draft-type') {
    const typed = await click('the composer', composer);
    // the last pass's draft is still there (kept across its restart) and a click lands mid-text:
    // empty it first, so each pass holds exactly its own words (run 40fc444d merged two drafts)
    if (typed) {
      await page.keyboard.press('Meta+a');
      await page.keyboard.press('Backspace');
      await page.keyboard.type(draft, { delay: 15 });
    }
    await shot('draft');
    const now = await holds();
    step('the draft is in the composer, not sent', now === draft, now.slice(0, 300));
  } else {
    const back = await until(async () => ((await holds()) === draft ? draft : null), 20000, 500);
    await shot('draft-back');
    step('the draft is back in the composer after the restart', !!back, back ?? (await holds()).slice(0, 300));
    const kept = fs.existsSync(path.join(out, 'side-width.txt')) ? Number(fs.readFileSync(path.join(out, 'side-width.txt'), 'utf8')) : null;
    if (kept) {
      const w = Number(await page.locator('[data-testid="app.split.side"]').first().getAttribute('aria-valuenow').catch(() => null));
      step('the sidebar width is kept across the restart', w === kept, { kept, now: w });
    }
  }
}
// probe (cand.sh AS_HE_DOES_ONLY=modes, a port's dev loop, never in a pass): the app stays up and
// each <out>/probe/<n>.js an agent writes runs as an async body with `page` and `text` in scope;
// its value (JSON) goes to <n>.out, an error to <n>.err. <out>/probe/done, or AS_HE_DOES_PROBE_S
// (1800) seconds, ends it.
if (mode === 'probe') {
  const dir = path.join(out, 'probe');
  fs.mkdirSync(dir, { recursive: true });
  const limit = Number(process.env.AS_HE_DOES_PROBE_S ?? 1800) * 1000;
  passLimit(limit + 60000);
  const AsyncFn = Object.getPrototypeOf(async () => {}).constructor as new (...a: string[]) => (...v: unknown[]) => Promise<unknown>;
  const t0 = Date.now();
  while (Date.now() - t0 < limit && !fs.existsSync(path.join(dir, 'done'))) {
    lastStep.at = Date.now();
    for (const f of fs.readdirSync(dir).filter((x) => x.endsWith('.js')).sort()) {
      const base = path.join(dir, f.slice(0, -3));
      if (fs.existsSync(`${base}.out`) || fs.existsSync(`${base}.err`)) continue;
      try {
        const v = await new AsyncFn('page', 'text', 'shot', fs.readFileSync(path.join(dir, f), 'utf8'))(page, text, shot);
        fs.writeFileSync(`${base}.out`, JSON.stringify(v ?? null, null, 1));
      } catch (e) { fs.writeFileSync(`${base}.err`, String((e as Error)?.stack ?? e)); }
    }
    await new Promise((r) => setTimeout(r, 500));
  }
}
ending = true;
await browser.close().catch(() => {});
