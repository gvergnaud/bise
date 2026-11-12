// plugins/doc-fake-opener · a document opens, reads and saves through its app's view in the
// desktop's document tab (docs/plugin-apps.md A4, A5; apps-host's brief, apps-viewkit's done
// criterion m_61846). On FakeCore, an artifact q3.fake whose row names fake-opener: the tab asks
// view/open, the core answers a live session (doc, handle, the view's html), and a pages server here
// serves that view's origin as the hub does (rust/switchboard/src/pages/views.rs: GET /mcp-app?s=
// with the real sandbox proxy read from the Rust source, the view's folder and bise's view kit at
// /f/<session>/, the CSP view_csp.rs writes for a view with its own files). fake-opener's own view
// (tests/fixtures/plugin-apps/fake-opener, on @bise/view) runs in it unchanged.
// Passes when: the view shows the file's name and its lines from resources/read on the handle; the
// tab's notes reach it (dev.bise/notes): '2 notes · 1 detached' (one on line:1, one on a line the
// file hasn't), the tab lists the one it couldn't place under its head, and on the hub's
// artifact_notes the tab asks view/notes again ('3 notes · 1 detached'); his save goes as
// ai.mistral.vibe/resources/write {uri: handle, ifMatch: the read's etag, text, seq 1} and the
// head says 'Fake · v3 · saved <time>'; the row then naming v4 saved by him reads as his, never a
// newer version over his own work. Shot: doc-fake-opener.png. Owner: apps-host.
import { readFileSync } from 'node:fs';
import { createServer, type Server } from 'node:http';
import type { AddressInfo } from 'node:net';
import path from 'node:path';
import { FakeCore, launchDesktop, test, expect } from '../../harness';
import { views } from '../../../src/proto/words_views.gen';

const ROOT = path.resolve(__dirname, '..', '..', '..', '..', '..');
const VIEW_DIR = path.join(ROOT, 'tests/fixtures/plugin-apps/fake-opener/ai.mistral.vibe/views/fake');
const KIT_DIR = path.join(ROOT, 'kit/view');

/** The sandbox proxy the pages server writes (views.rs PROXY), read from the Rust source. */
function proxyPage(csp: string, base: string): string {
  const src = readFileSync(path.join(ROOT, 'rust/switchboard/src/pages/views.rs'), 'utf8');
  const m = /const PROXY: &str = r#"([\s\S]*?)"#;/.exec(src);
  if (m?.[1] === undefined) throw new Error('no PROXY in views.rs');
  // views.rs script_json: a JSON string with '<' escaped
  const js = (s: string) => JSON.stringify(s).replace(/</g, '\\u003c');
  return m[1].replace('__CSP__', js(csp)).replace('__ALLOW__', js('')).replace('__BASE__', js(base));
}

const HOME = 'bise-home-00000000';
const P = 'harness-1a2b3c4d';
const NOW = Date.now();
const MIN = 60_000;
const ID = 'q3';
const SID = '0123456789abcdef0123456789abcdef';
const HANDLE = 'view-file://fedcba9876543210fedcba9876543210';
const HASH = '0123456789abcdef';
const TEXT = 'alpha\nbeta\ngamma';
const OPENER = { app: 'Fake', plugin: 'fake-opener', tool: 'fake', type: '*.fake', type_title: 'Fake file' };
// view_csp.rs for a view with its own files and nothing declared (own_files: 'self' on top)
const CSP = "default-src 'none'; script-src 'unsafe-inline' 'self'; style-src 'unsafe-inline' 'self'; img-src data: blob: 'self'; media-src data: blob:; font-src data: 'self'; connect-src 'none'; frame-src blob:; base-uri 'self'";
const TYPES: Record<string, string> = { '.css': 'text/css', '.js': 'text/javascript', '.html': 'text/html; charset=utf-8' };

/** The view's origin on the pages server: its proxy, its folder and the kit under its session. */
async function viewServer(): Promise<{ server: Server; origin: string }> {
  let origin = '';
  const server = createServer((req, res) => {
    const url = new URL(req.url ?? '/', 'http://x');
    const send = (status: number, type: string, body: string | Buffer) => {
      res.writeHead(status, { 'content-type': type, 'content-security-policy': CSP });
      res.end(body);
    };
    const f = new RegExp(`^/f/${SID}/(~kit/view/)?([a-z.-]+\\.(?:js|css))$`).exec(url.pathname);
    if (url.pathname === '/mcp-app' && url.searchParams.get('s') === SID) send(200, 'text/html; charset=utf-8', proxyPage(CSP, `${origin}/f/${SID}/`));
    else if (f?.[2] !== undefined) {
      let body: Buffer | null = null;
      try {
        body = readFileSync(path.join(f[1] === undefined ? VIEW_DIR : KIT_DIR, f[2]));
      } catch {
        body = null;
      }
      if (body === null) send(404, 'text/plain', 'no such file');
      else send(200, TYPES[path.extname(f[2])] ?? 'text/plain', body);
    } else send(404, 'text/plain', 'no such file');
  });
  await new Promise<void>((ok) => server.listen(0, '127.0.0.1', ok));
  origin = `http://v-${HASH}.localhost:${String((server.address() as AddressInfo).port)}`;
  return { server, origin };
}

const NOTE1 = { id: 'n1', anchor: { kind: 'view', key: 'line:1' }, text: 'why alpha?', status: 'sent', version: 3 };
const NOTE2 = { id: 'n2', anchor: { kind: 'view', key: 'line:9' }, text: 'and this?', quote: 'omega', status: 'sent', version: 2 };

const opened = (origin: string) => ({
  kind: 'view',
  doc: SID,
  handle: HANDLE,
  document: { name: 'q3.fake', type: 'Fake file', version: 3, size: TEXT.length, writable: true },
  opener: { plugin: 'fake-opener', tool: 'fake', app: 'Fake' },
  origin,
  proxy: `${origin}/mcp-app?s=${SID}`,
  html: readFileSync(path.join(VIEW_DIR, 'index.html'), 'utf8'),
  csp: { connectDomains: [], resourceDomains: [], frameDomains: [], baseUriDomains: [] },
  permissions: [],
  declared: [],
  prefersBorder: false,
  toolInput: { file: { name: 'q3.fake', resourceUri: HANDLE } },
  toolResult: { structuredContent: {} },
  tool: { name: 'fake' },
});

const row = (version: number, by?: unknown) => ({
  id: ID,
  kind: 'file',
  title: 'q3.fake',
  agent: 'perf',
  version,
  at_ms: NOW - 5 * MIN,
  url: '/nowhere/w/q3.fake',
  new: false,
  opener: OPENER,
  openers: [],
  ...(by === undefined ? {} : { versions: [{ v: version, at_ms: NOW, by }] }),
});

const agent = (name: string) => ({ name, main: name === 'main', status: 'idle', archived: false, title: '', purpose: `${name}'s job`, since_ms: NOW - 14 * MIN, waits: 0, watching: [] });
const world = [
  { ev: 'prefs', prefs: { onboarded: true, theme: 'light' } },
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
  { ev: 'artifacts', project: P, items: [row(3)] },
];

test('plugins/doc-fake-opener: a document opens, reads and saves through its app in its tab, its notes shown', async () => {
  test.setTimeout(90_000);
  const v = await viewServer();
  const core = new FakeCore(world as unknown as ConstructorParameters<typeof FakeCore>[0], { hold: 'view_open' });
  const s = await launchDesktop({ core, route: `artifacts/${P}/${ID}`, size: '1400x900' });
  try {
    const { page } = s;
    // the hub's answers, in the order the tab asks: view/open, its notes (they reach the view before
    // its read: it counts them again once its lines are drawn, apps-viewkit c7a335c54), the view's read
    await core.result('view_open', undefined, opened(v.origin));
    await core.result('view_notes', undefined, { notes: [NOTE1, NOTE2], detached: [] });
    await core.result('view_call', undefined, { contents: [{ uri: HANDLE, text: TEXT, _meta: { etag: 'e3', writable: true, size: TEXT.length } }] });
    await core.release();

    await expect(page.getByTestId('app.panel.doc')).toBeVisible({ timeout: 10_000 });
    const frame = page.frameLocator('[data-testid^="views.frame."]').frameLocator('iframe');
    // read: its name from the host context, its lines from resources/read on the handle
    await expect(frame.getByTestId('name')).toHaveText('q3.fake', { timeout: 15_000 });
    await expect(frame.locator('#lines li')).toHaveText(['alpha', 'beta', 'gamma']);
    await expect(frame.getByTestId('state')).toHaveText('saved');
    const asked = (x: unknown): Record<string, unknown> => x as Record<string, unknown>;
    const read = asked(await core.waitCmd((x) => asked(x)['cmd'] === 'view_call' && asked(x)['method'] === 'resources/read'));
    expect(read).toMatchObject({ project: P, doc: SID, params: { uri: HANDLE } });
    // notes: the view places one, the other has no line in this version: the tab lists it
    await expect(frame.getByTestId('notes')).toHaveText('2 notes · 1 detached');
    await expect(page.getByTestId('app.panel.doc.detached')).toContainText(views.doc_detached({ n: 1 }));
    await expect(page.getByTestId('app.panel.doc.detached')).toContainText('and this?');
    await expect(page.getByTestId('app.panel.doc.line')).toHaveText('Fake · v3');
    // a note written meanwhile (the hub's artifact_notes): the tab asks view/notes again
    await core.result('view_notes', undefined, { notes: [NOTE1, NOTE2, { id: 'n3', anchor: { kind: 'view', key: 'line:2' }, text: 'beta?', status: 'draft', version: 3 }], detached: [] });
    await core.send({ ev: 'artifact_notes', project: P, artifact: ID } as unknown as Parameters<FakeCore['send']>[0]);
    await expect(frame.getByTestId('notes')).toHaveText('3 notes · 1 detached');

    // save: his edit goes with the read's etag, the head says saved
    await core.result('view_call', undefined, { outcome: 'saved', etag: 'e4' });
    await frame.getByTestId('text').evaluate((el, t) => {
      (el as HTMLTextAreaElement).value = t;
    }, `${TEXT}\ndelta`);
    // the click is dispatched: Playwright's mouse never reaches an out-of-process frame here (md-view)
    await frame.getByTestId('save').dispatchEvent('click');
    await expect(frame.getByTestId('state')).toHaveText('saved');
    await expect(frame.locator('#lines li')).toHaveCount(4);
    const write = asked(await core.waitCmd((x) => asked(x)['cmd'] === 'view_call' && asked(x)['method'] === 'ai.mistral.vibe/resources/write'));
    expect(write).toMatchObject({ doc: SID, params: { uri: HANDLE, ifMatch: 'e3', text: `${TEXT}\ndelta`, seq: 1 } });
    await expect(page.getByTestId('app.panel.doc.line')).toContainText(`Fake · ${views.doc_version({ n: 3 })} · `);
    await expect(page.getByTestId('app.panel.doc.line')).toHaveAttribute('data-line', 'facts');
    // the hub's row then names v4, saved by him: his own work, no 'newer' line
    await core.send({ ev: 'artifacts', project: P, items: [row(4, { kind: 'you' })] } as unknown as Parameters<FakeCore['send']>[0]);
    await expect(page.getByTestId('app.panel.doc.line')).toContainText(`Fake · ${views.doc_version({ n: 4 })} · `);
    await expect(page.getByTestId('app.panel.doc.line')).toHaveAttribute('data-line', 'facts');
    await page.screenshot({ path: test.info().outputPath('doc-fake-opener.png') });
  } finally {
    await s.close();
    v.server.close();
  }
});
