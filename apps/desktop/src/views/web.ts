// src/views · the web host (docs/mcp-apps.md C9, piece 11): one view, full page, in his browser
// (the TUI's `O`), on the hub's page `GET /v/<agent>/<call>` (mcp-apps-sandbox's): the same
// ViewsStore and session as the app, the frame filling <main id="bise-view" data-agent
// data-call>, its asks a `POST /api/view` with the page's token. No React: built alone by
// views.web.vite.config.ts into kit/views/web.js + web.css. Owner: mcp-apps-host.
import { views } from '@/proto/words_views.gen';
import { allowOf } from '@/shared/viewperms';
import { readTokens, styleVariables, themeNow } from './context';
import type { Download, Json, JsonObject } from './protocol';
import { viewKey, type PluginViewsCmd, type ViewAnswer, type ViewCmd, type ViewRef } from './port';
import { ViewsStore, type ViewState } from './store';
import '@/ds/tokens.css';
import './web.css';

/** The C5 method of a view command. */
const METHOD: Readonly<Record<ViewCmd['cmd'], string>> = {
  view_resource: 'view/resource',
  view_call: 'view/call',
  view_message: 'view/message',
  view_context: 'view/context',
};

/** A view command as POST /api/view's body: its C5 method, its params without the window's ids. Pure. */
export function bodyOf(cmd: ViewCmd): JsonObject {
  const params: JsonObject = {};
  for (const [k, v] of Object.entries(cmd)) if (k !== 'cmd' && k !== 'project') params[k] = v as Json;
  return { method: METHOD[cmd.cmd], params };
}

/** POST /api/view's answer as the store reads it: {result} or {error}; anything else an error. Pure. */
export function answerOf(ok: boolean, body: unknown): ViewAnswer {
  const b = body !== null && typeof body === 'object' ? (body as Record<string, unknown>) : {};
  const e = b['error'];
  if (e !== null && typeof e === 'object') {
    const x = e as Record<string, unknown>;
    return { error: { code: typeof x['code'] === 'number' ? x['code'] : -32603, message: typeof x['message'] === 'string' ? x['message'] : '', ...(x['data'] === undefined ? {} : { data: x['data'] as Json }) } };
  }
  if (!ok || !('result' in b)) return { error: { code: -32603, message: 'the hub did not answer' } };
  return { result: (b['result'] ?? null) as Json };
}

function save(files: readonly Download[]): void {
  for (const f of files) {
    const bytes = f.base64 === undefined ? new TextEncoder().encode(f.text ?? '') : Uint8Array.from(atob(f.base64), (c) => c.charCodeAt(0));
    const url = URL.createObjectURL(new Blob([bytes], { type: f.mime }));
    const a = document.createElement('a');
    a.href = url;
    a.download = f.name;
    a.click();
    setTimeout(() => {
      URL.revokeObjectURL(url);
    }, 10_000);
  }
}

/** The page's quiet line for a state (the app's StateLine words), or '' when the view shows. Pure. */
export function wordsOf(s: ViewState): string {
  switch (s.kind) {
    case 'none':
    case 'loading':
      return views.loading;
    case 'expired':
      return views.expired;
    case 'failed':
      return views.failed;
    case 'live':
      return s.phase === 'live' ? '' : views.loading;
  }
}

/** Starts the page's view: its store, its frame once the hub gave it, its words until it shows. */
export function start(main: HTMLElement, token: string): void {
  const agent = main.dataset['agent'] ?? '';
  const call = main.dataset['call'] ?? '';
  const ref: ViewRef = { project: '', agent, call };
  const post = async (cmd: ViewCmd | PluginViewsCmd): Promise<ViewAnswer> => {
    if (cmd.cmd === 'plugin_views') return { error: { code: -32601, message: 'not on the web host' } };
    try {
      const r = await fetch('/api/view', { method: 'POST', headers: { 'content-type': 'application/json', 'x-bise-token': token }, body: JSON.stringify(bodyOf(cmd)) });
      return answerOf(r.ok, await r.json().catch(() => null));
    } catch {
      return { error: { code: -32603, message: 'the hub did not answer' } };
    }
  };
  const store = new ViewsStore({
    ask: post,
    base: () => ({ theme: themeNow(), variables: styleVariables(readTokens()), locale: navigator.language, timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone }),
    openLink: (url) => {
      window.open(url, '_blank', 'noopener,noreferrer');
    },
    download: save,
    // the page is the view: always fullscreen, no panel
    panel: { has: () => true, open: () => undefined, close: () => undefined },
    watch: () => () => undefined,
    now: () => Date.now(),
    later: (ms, f) => {
      setTimeout(f, ms);
    },
  });
  const key = viewKey(ref);
  const line = document.createElement('div');
  line.className = 'vw-web-line';
  main.append(line);
  let frame: HTMLIFrameElement | null = null;
  const size = () => {
    store.size(key, 'tab', main.clientWidth, main.clientHeight);
  };
  const draw = () => {
    const s = store.state(ref);
    line.textContent = wordsOf(s);
    line.hidden = line.textContent === '';
    const live = store.liveOf(key);
    if (live !== undefined && frame === null) {
      frame = document.createElement('iframe');
      frame.className = 'vw-web-frame';
      frame.src = live.res.proxy;
      frame.setAttribute('sandbox', 'allow-scripts allow-same-origin allow-forms');
      frame.allow = allowOf(live.res.permissions);
      frame.title = s.kind === 'live' && s.tool !== '' ? s.tool : views.tab_view;
      main.append(frame);
      store.frame(key, frame);
      size();
    }
  };
  store.subscribe(draw);
  store.listen(window);
  new ResizeObserver(size).observe(main);
  store.ensure(ref);
  draw();
}

const main = typeof document === 'undefined' ? null : document.getElementById('bise-view');
const token = typeof document === 'undefined' ? '' : (document.querySelector<HTMLMetaElement>('meta[name="bise-token"]')?.content ?? '');
if (main !== null) start(main, token);
