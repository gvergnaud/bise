// plugins/doc-tab · a document drawn by its app in the panel (docs/plugin-apps.md A5, A6b;
// designer's mocks, page plugin-apps-mocks §3). An artifact whose row names an opener (A3) opens
// in a document tab: a head (its name, ⋯) and one line under it, picked from view/open's answer
// and the row alone (app/panel/docline.ts):
//   facts      Sheets · v3                       (writable, the newest)
//   read-only  Sheets · v3 · read only           (document.writable false)
//   newer      Sheets · v2 · perf saved v3 · Reload  (the tab follows the newest and he can edit:
//              v3 was saved after its view/open answered v2; a version he picked would say
//              'there's a newer one · Open v3' instead, docline.test.ts)
//   off        Sheets is off · Turn on           (view/open -32032: its plugin is off)
//   gone       this file was removed             (the row's gone: no view/open at all)
//   failed     ✗ the view didn't load · Try again  (any other refusal)
//   loading    no line; 'loading the view…' centred in the body (view/open not answered yet)
// One place per state (designer m_61435): under gone, off and failed the body stays empty. The
// view itself never loads here (its origin answers nothing): the line is the host's, said before
// the view runs. Passes when, on FakeCore at 1400 light and dark, each case's tab shows its line
// with its words, view/open asked the artifact by id (none for gone), Turn on sends plugin_set for
// the opener's plugin and asks view/open again. Shots: doc-<case>-<theme>.png, for designer.
// Owner: apps-host.
import { FakeCore, launchDesktop, test, expect } from '../../harness';
import { views } from '../../../src/proto/words_views.gen';

const HOME = 'bise-home-00000000';
const P = 'harness-1a2b3c4d';
const NOW = Date.now();
const MIN = 60_000;
const ID = 'q3-budget';
const OPENER = { app: 'Sheets', plugin: 'sheets', tool: 'open_sheet', type: '*.xlsx', type_title: 'Excel workbook' };
const ORIGIN = 'http://v-0123456789abcdef0123456789abcdef.localhost:47123';

type Case = 'facts' | 'read-only' | 'newer' | 'off' | 'gone' | 'failed' | 'loading';
const CASES: readonly Case[] = ['facts', 'read-only', 'newer', 'off', 'gone', 'failed', 'loading'];
/** the lines that say why the document isn't there: the body under them stays empty */
const SAID: ReadonlySet<Case> = new Set(['off', 'gone', 'failed']);

/** view/open's answer for a live document: its session, handle, the document's facts, its opener. */
const opened = (version: number, writable: boolean) => ({
  kind: 'view',
  doc: '0123456789abcdef0123456789abcdef',
  handle: 'view-file://fedcba9876543210fedcba9876543210',
  document: { name: 'q3.xlsx', type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet', version, size: 18_432, writable },
  opener: { plugin: 'sheets', tool: 'open_sheet', app: 'Sheets' },
  origin: ORIGIN,
  proxy: `${ORIGIN}/mcp-app`,
  html: '<!doctype html><p>q3</p>',
  csp: {},
  permissions: [],
  declared: [],
  prefersBorder: false,
  toolInput: {},
  toolResult: { structuredContent: {} },
  tool: { name: 'open_sheet' },
});

const agent = (name: string) => ({ name, main: name === 'main', status: 'idle', archived: false, title: '', purpose: `${name}'s job`, since_ms: NOW - 14 * MIN, waits: 0, watching: [] });
const world = (theme: 'light' | 'dark', gone: boolean) => [
  { ev: 'prefs', prefs: { onboarded: true, theme } },
  {
    ev: 'projects',
    projects: [
      { project: HOME, name: 'bise', path: '/nowhere', git: false, home: true, order: 0, agents: 1, working: 0, waits: 0, running: true },
      { project: P, name: 'harness', path: '/nowhere/harness', git: true, home: false, order: 1, agents: 2, working: 0, waits: 0, running: true },
    ],
  },
  { ev: 'welcome', project: P, proto: 1, workspace: '/nowhere/harness', name: 'harness' },
  { ev: 'agents', project: P, agents: [agent('main'), agent('perf')] },
  { ev: 'agents', project: HOME, agents: [agent('main')] },
  {
    ev: 'artifacts',
    project: P,
    items: [
      { id: ID, kind: 'file', title: 'q3.xlsx', agent: 'perf', version: 3, at_ms: NOW - 5 * MIN, url: '/nowhere/w/q3.xlsx', new: false, opener: OPENER, openers: [], versions: [{ v: 3, at_ms: NOW - 5 * MIN, by: { kind: 'agent', name: 'perf' } }], ...(gone ? { gone: true } : {}) },
    ],
  },
];

/** Arms the core's answer to the tab's view/open for that case. */
async function arm(core: FakeCore, c: Case): Promise<void> {
  if (c === 'gone' || c === 'loading') return;
  if (c === 'facts') await core.result('view_open', undefined, opened(3, true));
  else if (c === 'read-only') await core.result('view_open', undefined, opened(3, false));
  else if (c === 'newer') await core.result('view_open', undefined, opened(2, true));
  else if (c === 'off') await core.refuse('view_open', undefined, 'plugin disabled', undefined, -32032);
  else await core.refuse('view_open', undefined, 'the opener failed');
}

/** The line's words for that case (the views catalog's). */
function wordsOf(c: Case): string {
  switch (c) {
    case 'facts':
      return `Sheets · ${views.doc_version({ n: 3 })}`;
    case 'read-only':
      return `Sheets · ${views.doc_version({ n: 3 })} · ${views.doc_read_only}`;
    case 'newer':
      return `Sheets · ${views.doc_version({ n: 2 })} · ${views.doc_newer_by({ who: 'perf', n: 3 })}`;
    case 'off':
      return views.doc_app_off({ app: 'Sheets' });
    case 'gone':
      return views.doc_gone;
    case 'failed':
      return views.doc_failed;
    case 'loading':
      return '';
  }
}

for (const theme of ['light', 'dark'] as const) {
  for (const c of CASES) {
    test(`plugins/doc-tab: an artifact with an opener opens in its document tab, line '${c}' (${theme})`, async () => {
      test.setTimeout(60_000);
      // the tab's view/open waits until its answer is armed ('newer': the hub answers v2 of a row
      // whose newest is v3)
      const core = new FakeCore(world(theme, c === 'gone') as unknown as ConstructorParameters<typeof FakeCore>[0], { hold: 'view_open' });
      const s = await launchDesktop({ core, route: `artifacts/${P}/${ID}`, size: '1400x900' });
      try {
        const { page } = s;
        await arm(core, c);
        // 'loading': its view/open stays held, never answered in this test
        if (c !== 'loading') await core.release();
        await expect(page.getByTestId('app.panel.doc')).toBeVisible({ timeout: 10_000 });
        await expect(page.getByTestId('app.panel.doc.name')).toHaveText('q3.xlsx');
        const line = page.getByTestId('app.panel.doc.line');
        const body = page.locator('.pn-doc-view .vw-quiet');
        // a views command (main/views.ts VIEW_CMDS), not one of the proto's AppCmd: read as a record
        const asked = (x: unknown): Record<string, unknown> => x as Record<string, unknown>;
        const opens = () => core.cmds().filter((x) => asked(x)['cmd'] === 'view_open').length;
        if (c === 'loading') {
          await expect(body).toBeVisible({ timeout: 10_000 });
          await expect(line).toHaveCount(0);
        } else {
          await expect(line).toHaveAttribute('data-line', c === 'read-only' ? 'facts' : c, { timeout: 10_000 });
          await expect(line).toContainText(wordsOf(c));
          if (SAID.has(c)) await expect(body).toHaveCount(0);
        }
        if (c === 'gone') expect(opens(), 'a removed file asks the hub nothing').toBe(0);
        else {
          const ask = asked(await core.waitCmd((x) => asked(x)['cmd'] === 'view_open'));
          expect(ask['target']).toMatchObject({ kind: 'artifact', id: ID });
        }
        await page.screenshot({ path: test.info().outputPath(`doc-${c}-${theme}.png`) });
        if (c === 'off') {
          await page.getByTestId('app.panel.doc.turn-on').click();
          const on = await core.waitCmd((x) => x.cmd === 'plugin_set');
          expect(on).toMatchObject({ project: P, name: 'sheets', on: true });
          await expect.poll(opens, { message: 'Turn on opens it again' }).toBe(2);
        }
        if (c === 'failed') await expect(page.getByTestId('app.panel.doc.retry')).toHaveText(views.doc_try_again);
        if (c === 'newer') await expect(page.getByTestId('app.panel.doc.reload')).toHaveText(views.doc_reload);
      } finally {
        await s.close();
      }
    });
  }
}
