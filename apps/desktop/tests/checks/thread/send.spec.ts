// thread/send · bar ⛔3 (docs/desktop-bar.md) · his bug 5: "a sent message does nothing".
// Owner: qa-flows (first cut by qa-type on the old window). The NEW app on the REAL core
// (`bise ambient-core --home`, started by the app's main as for him) over a throwaway world
// (../world.ts), the TUI on the same hub as the reference. Real keys only.
// The rule (ambient-pm, m_9852 and m_9860), in its cold case: the send goes as soon as bise's
// composer shows after the app's launch, against the TUI's first send after its own launch:
//   - his words are drawn in the thread by the 3rd frame after ⏎ (frames, not ms: the draw waits
//     for nothing outside the page, and a wall-clock bound went red under load on 11.8's checks;
//     the ms stay in the timeline annotation), and the field is empty. Its gap (architect): a
//     long task on the page's own main thread (300 ms of JS before the next frame) is still one
//     frame, so this bound misses it; the annotation's ms show it, as-he-does and the bench
//     record are where to watch it;
//   - from ⏎ until main works, every moment shows a named state: his row's state ('sending…',
//     'connecting to bise…', the routing hold thread.route 'to <project> · N s to change', 'sent
//     to <name>'), or the answer's own entry; a gap of one event hop (<= 250 ms) is let through;
//     a plain 'sending…' longer than 1 s fails: a wait that long is named for what it is;
//   - main's working state (composer.stop) comes no later than the TUI's + 0.5 s, the time the
//     hold is shown not counted;
//   - the home hub's main really got the words (its transcript.log in the world), and the
//     scripted model's answer ("ack: … <his words> …") is drawn.
// The warm case (⏎ seconds after the launch) measured 81 ms to working: the cold one is the hard one.
// Offline (the core gone): thread/send-offline.
import { test, expect } from '../../harness';
import { hubOf, hubState, launchReal, realWorld, transcriptHas, tui, tuiReady, type World } from '../world';
import { expectShownSoft } from './shown';

const T = {
  field: '[data-testid="composer.field"]',
  thread: '[data-testid="thread.list"]',
  setupSkip: '[data-testid="setup.skip"]',
};

const mainStatus = async (w: World): Promise<string> => {
  const agents = (await hubState(hubOf(w, 'bise').sock))['agents'] as { name: string; status: string }[] | undefined;
  return agents?.find((a) => a.name === 'main')?.status ?? '';
};

/** The TUI's first send after its launch on bise's hub: ms from ⏎ to main working (the hub). */
async function tuiColdMs(w: World): Promise<number> {
  const t = tui(w, 'bise');
  try {
    await tuiReady(t);
    t.keys('-l', `tui cold ${Date.now() % 1_000_000} [[slow: 2]]`);
    const t0 = Date.now();
    t.keys('Enter');
    while (Date.now() - t0 < 15_000) {
      if ((await mainStatus(w)) === 'working') return Date.now() - t0;
      await new Promise((r) => setTimeout(r, 30));
    }
    return Infinity;
  } finally {
    t.close();
  }
}

type Sample = { t: number; mine: boolean; row: string; hold: boolean; working: boolean; answer: boolean };

test('thread/send: cold, his words show at once, every wait is named, main works as soon as in the TUI', async () => {
  test.setTimeout(240_000);
  const w = realWorld();
  await expect.poll(() => mainStatus(w), { timeout: 30_000, message: 'bise main is idle before the test' }).not.toBe('working');
  const tuiMs = await tuiColdMs(w);
  test.info().annotations.push({ type: 'tui', description: `the TUI's first send: main working ${tuiMs} ms after ⏎` });
  await expect.poll(() => mainStatus(w), { timeout: 30_000, message: "the TUI's turn is over" }).not.toBe('working');

  // [[slow: 2]]: the model takes 2 s, so main's working state lasts long enough to be seen
  const n = `${Date.now() % 1_000_000}`;
  const words = `hi from qa-flows ${n} [[slow: 2]]`;
  const s = await launchReal(w);
  try {
    const { page } = s;
    const field = page.locator(T.field).first();
    // a fresh home opens setup once (amb-home c44c926d): skip it the way he would, by a click
    await expect(page.locator(`${T.field}, ${T.setupSkip}`).first()).toBeVisible({ timeout: 10_000 }).catch(() => undefined);
    if (await page.locator(T.setupSkip).isVisible()) await page.locator(T.setupSkip).click();
    try {
      await expect(field, "bise's thread opens with its composer on the real core").toBeVisible({ timeout: 10_000 });
    } finally {
      await page.screenshot({ path: test.info().outputPath('opened.png') });
    }
    await field.click();
    await page.keyboard.type(words);
    // the clock starts at his ⏎ as the page gets it (its event time), not when the test asks: the
    // listener is in place before the key is pressed (a listener added by the sampler itself raced
    // the key, and a sampler that never saw it never ended)
    await page.evaluate(() => {
      const g = window as unknown as { __qaEnterAt?: number };
      delete g.__qaEnterAt;
      window.addEventListener('keydown', (e) => { if (e.key === 'Enter' && g.__qaEnterAt === undefined) g.__qaEnterAt = e.timeStamp; }, { capture: true });
    });
    // what is drawn, every frame from ⏎ until main works (or 15 s): read in the page, from the DOM
    const timeline = page.evaluate(({ thread, n: nonce }) => new Promise<Sample[]>((done) => {
      const out: Sample[] = [];
      const g = window as unknown as { __qaEnterAt?: number };
      const started = performance.now();
      const tick = (): void => {
        const t0 = g.__qaEnterAt;
        if (t0 === undefined) {
          if (performance.now() - started > 5000) { done(out); return; } // ⏎ never reached the page
          requestAnimationFrame(tick);
          return;
        }
        const list = document.querySelector(thread);
        const rows = Array.from(document.querySelectorAll('[data-testid^="thread.pending."]')).filter((r) => r.textContent.includes(nonce));
        const row = rows.map((r) => `${r.getAttribute('data-state') ?? ''}:${r.querySelector('.th-hold') ? 'hold' : (r.textContent.match(/sending…|connecting[^·]*…|sent to \S+|not sent/)?.[0] ?? '')}`).join(',');
        const s: Sample = {
          t: performance.now() - t0,
          mine: list?.textContent.includes(nonce) ?? false,
          row,
          hold: document.querySelector('[data-testid="thread.route"]') !== null,
          working: document.querySelector('[data-testid="composer.stop"]') !== null,
          answer: (list?.textContent ?? '').includes(`ack: hi from qa-flows ${nonce} [[slow: 2]]`),
        };
        out.push(s);
        if (s.working || s.answer || s.t > 15_000) done(out);
        else requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
    }), { thread: T.thread, n });
    await page.keyboard.press('Enter');
    const frames = await timeline;
    const drawnMs = frames.find((f) => f.mine)?.t ?? Infinity;
    // the frames sampled from ⏎ until his words are in the DOM: 0 = the first frame after ⏎
    const drawnAt = frames.findIndex((f) => f.mine);
    const drawnFrame = drawnAt < 0 ? Infinity : drawnAt;
    const workingMs = frames.find((f) => f.working)?.t ?? Infinity;
    let holdMs = 0, gap = 0, worstGap = 0, sending = 0, worstSending = 0;
    for (let i = 1; i < frames.length; i++) {
      const f = frames[i], prev = frames[i - 1];
      if (!f || !prev) continue;
      const dt = f.t - prev.t;
      if (f.hold) holdMs += dt;
      const named = f.hold || f.working || f.answer || f.row !== '';
      gap = f.mine && !named ? gap + dt : 0;
      worstGap = Math.max(worstGap, gap);
      sending = /sending…/.test(f.row) && !f.hold ? sending + dt : 0;
      worstSending = Math.max(worstSending, sending);
    }
    const states = [...new Set(frames.map((f) => (f.working ? 'working' : f.hold ? 'hold' : f.row || (f.mine ? '(entry)' : '(none)'))))];
    test.info().annotations.push({ type: 'timeline', description: `drawn ${Math.round(drawnMs)} ms · working ${Math.round(workingMs)} ms · hold shown ${Math.round(holdMs)} ms · longest unnamed ${Math.round(worstGap)} ms · longest plain sending ${Math.round(worstSending)} ms · states ${states.join(' > ')}` });
    await page.screenshot({ path: test.info().outputPath('sent.png') });
    expect.soft(drawnFrame, 'frames from ⏎ to his words drawn in the thread (his own row, no wait outside the page)').toBeLessThanOrEqual(2);
    await expect.poll(() => field.evaluate((e) => e.textContent), { message: 'the field is empty after the send' }).toBe('');
    expect.soft(worstGap, 'ms with his words drawn but no named state').toBeLessThanOrEqual(250);
    expect.soft(worstSending, "ms of a plain 'sending…' (a longer wait must say what it waits for)").toBeLessThanOrEqual(1000);
    expect.soft(workingMs - holdMs, "ms from ⏎ to main working, the shown hold not counted, vs the TUI's + 500").toBeLessThanOrEqual(tuiMs + 500);
    await expect.poll(() => transcriptHas(w, 'bise', 'main', `hi from qa-flows ${n}`), { timeout: 15_000, message: "the home hub's main got his words" }).toBe(true);
    await expect(page.locator(T.thread), "the model's answer is drawn in the thread").toContainText(`ack: ${words}`, { timeout: 20_000 });
    // in the DOM is not on screen (amb-win 35d08ae5): his row and the answer's are shown
    const rows = page.locator(T.thread).locator('[data-testid^="thread.entry."], [data-testid^="thread.pending."]');
    await expectShownSoft(rows.filter({ hasText: `hi from qa-flows ${n}` }).last(), 'his row');
    await expectShownSoft(rows.filter({ hasText: `ack: ${words}` }).last(), "the answer's row (the list follows the answer)");
    await page.screenshot({ path: test.info().outputPath('answered.png') });
  } finally {
    await s.close();
  }
});
