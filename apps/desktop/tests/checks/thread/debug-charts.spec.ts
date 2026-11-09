// thread/debug-charts · debug-viz (Gabriel: « des graphiques sur cette page pour comprendre la
// proportion de la contribution en token de chaque role ... pouvoir cliquer sur le graph pour filtrer
// la liste »). On FakeCore, a request of 240 messages with the hub's parts (fake data), at 1400 and
// 1100, light and dark. Passes when the charts show the roles biggest first, a click on 'tool result'
// then 'bash' filters the list to their messages (a chip, the count), a bin adds a size chip, ⌫
// clears, and a click on the series' peak opens that message.
// Shots: debug-charts-<width>-<theme>.png, for designer (page debug-viz v5). Owner: debug-viz.
import { FakeCore, launchDesktop, test, expect } from '../../harness';
import { debug } from '../../../src/proto/words_debug.gen';

const ACME = 'acme-1a2b3c4d';
type Part = { role: string; kind: string; names?: string[]; tokens: number };

// a fixed pseudo-random walk: the same request every run
let seed = 7;
const rnd = (): number => {
  seed = (seed * 1103515245 + 12345) % 2147483648;
  return seed / 2147483648;
};
const PROGRAMS = ['rg', 'sed', 'cargo', 'git', 'sb', 'cat'];

function rowOf(n: number, parts: Part[], role: string, head: string) {
  const tokens = parts.reduce((a, p) => a + p.tokens, 0);
  return { n, role, head, blocks: [{ body: { kind: 'plain', text: head } }], bytes: tokens * 4, tokens, parts };
}

function request() {
  const rows = [
    rowOf(0, [
      { role: 'system', kind: 'tools', names: ['bash'], tokens: 2100 },
      { role: 'system', kind: 'tools', names: ['run_typescript'], tokens: 6400 },
      { role: 'system', kind: 'tools', names: ['edit'], tokens: 900 },
      { role: 'system', kind: 'params', tokens: 40 },
    ], 'request', 'anthropic · claude-opus-5-5 · 3 tools'),
    rowOf(1, [{ role: 'system', kind: 'prompt', tokens: 18900 }], 'system', 'system prompt, 412 lines'),
    rowOf(2, [{ role: 'user', kind: 'summary', tokens: 3400 }], 'user', 'Summary of the earlier conversation:'),
  ];
  for (let n = 3; n < 240; n++) {
    if (n % 2 === 1) {
      const prog = PROGRAMS[n % PROGRAMS.length] ?? 'rg';
      const t = n === 211 ? 9100 : Math.floor(rnd() ** 3 * 2600) + 30;
      const tool = n % 6 === 5 ? 'run_typescript' : 'bash';
      const parts: Part[] = [{ role: 'tool_result', kind: 'tool', names: tool === 'bash' ? ['bash', prog] : ['run_typescript', 'slack'], tokens: t }];
      if (n % 9 === 0) parts.push({ role: 'user', kind: 'agent_message', names: ['main'], tokens: 300 });
      if (n % 7 === 0) parts.push({ role: 'user', kind: 'bise_state', tokens: 420 });
      // images as their provider counts them (main m_41815): screenshots in results, his own
      if (n % 23 === 0) parts.push({ role: 'tool_result', kind: 'image', names: ['computer'], tokens: 1534 });
      if (n % 47 === 0) parts.push({ role: 'user', kind: 'image', tokens: 667 });
      rows.push(rowOf(n, parts, 'user', tool === 'bash' ? `ok: ${prog} ran` : '{"messages":[…]}'));
    } else {
      const prog = PROGRAMS[(n + 1) % PROGRAMS.length] ?? 'rg';
      rows.push(rowOf(n, [
        { role: 'assistant', kind: 'text', tokens: 20 + Math.floor(rnd() * 60) },
        { role: 'tool_call', kind: 'tool', names: n % 6 === 4 ? ['run_typescript', 'slack'] : ['bash', prog], tokens: 40 + Math.floor(rnd() * 120) },
      ], 'assistant', n % 6 === 4 ? 'tools.slack.search' : `${prog} -n "CompRow" rust`));
    }
  }
  const slot = { slot: 3, req: 99, kind: 'turn', model: 'anthropic/claude-opus-5-5', at_ms: 1791100171000 };
  return { ev: 'completion', project: ACME, agent: 'perf', slot, family: 'anthropic', bytes: 728000, rows, slots: [slot] };
}

for (const width of [1400, 1100] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`thread/debug-charts: the charts filter the list (${String(width)}, ${theme})`, async () => {
      test.setTimeout(90_000);
      const core = new FakeCore();
      const s = await launchDesktop({ core, route: `debug/${ACME}/perf/completion` });
      try {
        const { page } = s;
        await s.app.evaluate(({ BrowserWindow }, w) => {
          BrowserWindow.getAllWindows().find((b) => b.webContents.getURL().includes('surface=window'))?.setContentSize(w, 900);
        }, width);
        await expect.poll(() => page.evaluate(() => window.innerWidth), { timeout: 5000 }).toBe(width);
        await core.send({ ev: 'prefs', prefs: { onboarded: true, theme } } as never);
        await core.send(request() as never);
        // the panel folded, as a fresh window has it (panel.test.tsx 'is folded by default'); at 1400
        // the side's outer card goes compact (its bar button, the coins): the pane is over 880, the
        // charts a column; at 1100 (no room for the card) under 880, the charts above the list
        // behind three tabs
        await expect(page.getByTestId('app.panel.bar')).not.toHaveAttribute('data-open');
        if (width === 1400) await page.getByTestId('app.bar.sidecard').click();
        const charts = page.getByTestId('comp.charts');
        await expect(charts).toBeVisible({ timeout: 15_000 });
        const keys = await charts.locator('.lg-bar').evaluateAll((bs) => bs.map((b) => b.getAttribute('data-testid')));
        expect(keys[0]).toBe('comp.chart.bar.tool_result');
        // the images, a group of their own in each role, filter like the rest
        await page.getByTestId('comp.chart.bar.tool_result').click();
        await expect(page.getByTestId('comp.chart.bar.k:image')).toBeVisible();
        await page.getByTestId('comp.chart.bar.k:image').click();
        await expect(page.getByTestId('comp.filter.count')).toContainText('of 240 messages');
        if (width === 1400) await page.screenshot({ path: test.info().outputPath(`debug-charts-images-${theme}.png`) });
        await page.getByTestId('debug.completion').press('Backspace');

        // two clicks down: tool result, then bash; the list keeps their messages
        await page.getByTestId('comp.chart.bar.tool_result').click();
        await expect(page.getByTestId('comp.filter.group')).toContainText(debug.role_tool_result);
        await page.getByTestId('comp.chart.bar.n:bash').click();
        await expect(page.getByTestId('comp.chart.crumbs')).toHaveText(`${debug.crumb_all} › ${debug.role_tool_result} › bash`);
        const count = await page.getByTestId('comp.list').locator('[data-testid^="comp.row."][data-n]').count();
        expect(count).toBeGreaterThan(0);
        expect(count).toBeLessThan(240);
        await expect(page.getByTestId('comp.filter.count')).toContainText(`${String(count)} of 240 messages`);
        if (width === 1100) await page.getByTestId('comp.charts.tab.series').click();
        {
          await page.getByTestId('comp.chart.peak').click();
          await expect(page.getByTestId('comp.row.211.body')).toBeVisible();
        }
        await page.screenshot({ path: test.info().outputPath(`debug-charts-${String(width)}-${theme}.png`) });
        if (width === 1100) {
          await page.getByTestId('comp.charts.tab.tokens').click();
          await page.screenshot({ path: test.info().outputPath(`debug-charts-${String(width)}-${theme}-tokens.png`) });
        }

        // a bin adds its size; ⌫ clears both
        if (width === 1100) await page.getByTestId('comp.charts.tab.sizes').click();
        await charts.locator('.lg-bin:not([disabled])').first().click();
        await expect(page.getByTestId('comp.filter.size')).toBeVisible();
        await page.getByTestId('debug.completion').press('Backspace');
        await expect(page.getByTestId('comp.filter')).toHaveCount(0);
      } finally {
        await s.close();
      }
    });
  }
}
