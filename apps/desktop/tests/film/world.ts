// The README film's world (readme-film): a made-up shop, `acme`, on the scripted core. Nothing of
// his: fake paths, fake agents, fake numbers. The story's events are built here, sent on cue by
// readme.spec.ts while the real app is recorded off screen.

import { thread } from '../../src/proto/words_thread.gen';

export const HOME = 'bise-home-00000000';
export const P = 'acme-5h0p2026';
const PATH = '/Users/sam/acme';
const MIN = 60_000;

export type Ev = Record<string, unknown>;

const prefs = (): Ev => ({
  ev: 'prefs',
  prefs: {
    onboarded: true, theme: 'dark', context: { shot: true, url: true, file: true, window_text: false }, excluded_apps: [],
    quiet: { call: true, share: true, fullscreen: true, focus: true, batch_min: 20, away_card: true },
    voice: { fn_hold: true, fn_twice: 'spotlight', language: 'auto' }, shots_keep_days: 7,
  },
});

export type Row = { name: string; status: 'working' | 'idle' | 'waiting' | 'done'; phase?: 'starting' | null; title?: string; purpose?: string; waits?: number; doing?: string | null; turn_ms?: number | null; branch?: string };

/** an agent row as the hub sends it (rows.ts Agent), only what the bar and the bims read */
export function agentRow(r: Row, now: number): Ev {
  const main = r.name === 'main';
  return {
    name: r.name, main, status: r.status, phase: r.phase ?? null, archived: false,
    title: r.title ?? '', purpose: r.purpose ?? '', since_ms: now - 2000, waits: r.waits ?? 0,
    dir: r.name, watching: [], waiting_on: (r.waits ?? 0) > 0 ? { who: 'you' } : null,
    // a working main has no turn_ms here: the window's title bar reads it as a time (app/title.ts
    // stateFactOf: now - turn_ms), which drew 'working · 20758 d' in take 3
    turn_ms: r.status === 'working' && !main ? (r.turn_ms ?? 4000) : null,
    ...(main ? { mode: 'shared', path: PATH, checkout_branch: 'main', place_id: 'shared', parent: null }
      : { parent: 'main', branch: r.branch ?? `sb/${r.name}`, worktree: `/Users/sam/.bise/worktrees/acme/${r.name}`, place_id: `wt:sb/${r.name}`, mode: 'worktree' }),
    model: 'anthropic/claude-opus', effort: 'high',
    doing: r.doing != null ? { kind: r.doing, since_ms: now - 1000 } : null,
  };
}

export function agents(rows: Row[], now: number): Ev {
  const places = rows.filter((r) => r.name !== 'main').map((r) => ({ id: `wt:sb/${r.name}`, branch: `sb/${r.name}`, agents: [r.name] }));
  return { ev: 'agents', project: P, agents: rows.map((r) => agentRow(r, now)), places };
}

export function projects(o: { agents: number; working: number; waits: number }): Ev {
  return {
    ev: 'projects',
    projects: [
      { project: HOME, name: 'bise', path: '/Users/sam/bise', git: false, home: true, order: 0, agents: 1, working: 0, waits: 0, running: true },
      { project: P, name: 'acme', path: PATH, git: true, home: false, order: 1, agents: o.agents, working: o.working, waits: o.waits, running: true },
    ],
  };
}

/** main's thread before the film: one small thing already done, so the thread isn't empty */
export function history(now: number): Ev[] {
  return [
    { pos: 1, at_ms: now - 40 * MIN, kind: 'you', text: 'free shipping from 60 € instead of 50', delivery: 'read', msg_id: 1 },
    { pos: 2, at_ms: now - 39 * MIN, kind: 'agent', text: 'done: free shipping starts at 60 € now. the cart test passes, it is on main.' },
  ];
}

// the tools' summaries in the app's own words (words_thread.gen.ts), never retyped
const SUM_READ = `${thread.did_read({ n: 2 })}, ${thread.did_run({ n: 1 })}`;

/** perf's thread: its brief's work so far, then what comes while he looks (perfLive) */
const tool = (pos: number, at: number, kind: string, text: string, more: Record<string, unknown> = {}) => ({ pos, id: pos, at_ms: at, text, kind, state: 'ok', ms: 400, ...more });
export function perfThread(now: number): Ev[] {
  return [
    { pos: 1, at_ms: now - 9000, kind: 'tools', text: SUM_READ, tools: { count: 3, summary: SUM_READ, items: [
      tool(1, now - 9000, 'read', 'reading src/pages/product.tsx'),
      tool(2, now - 8800, 'read', 'reading src/lib/images.ts'),
      tool(3, now - 8000, 'run', 'measuring the product page', { name: 'bash', code: JSON.stringify({ command: 'pnpm lighthouse /product/42', description: 'measuring the product page' }), out: 'LCP 2.9 s · TBT 410 ms' }),
    ] } },
    { pos: 2, at_ms: now - 6000, kind: 'agent', text: 'the page waits on a 1.8 MB hero image and on the reviews query. i resize the images and load the reviews after the first paint.' },
  ];
}
export function perfLive(now: number): Ev[] {
  return [
    { pos: 3, at_ms: now, kind: 'tools', text: thread.did_edit({ n: 2 }), tools: { count: 2, summary: thread.did_edit({ n: 2 }), items: [
      tool(3, now, 'edit', 'images through the CDN, 3 widths', { files: [{ path: 'src/lib/images.ts', add: 24, del: 3 }] }),
      tool(4, now + 300, 'edit', 'reviews after the first paint', { files: [{ path: 'src/pages/product.tsx', add: 9, del: 4 }] }),
    ] } },
  ];
}
export function perfLive2(now: number): Ev[] {
  return [
    { pos: 5, at_ms: now, kind: 'tools', text: thread.did_run({ n: 1 }), tools: { count: 1, summary: thread.did_run({ n: 1 }), items: [
      tool(5, now, 'run', 'measuring again', { name: 'bash', code: JSON.stringify({ command: 'pnpm lighthouse /product/42', description: 'measuring again' }), out: 'LCP 0.8 s · TBT 90 ms' }),
    ] } },
    { pos: 6, at_ms: now + 200, kind: 'agent', text: '0.8 s now, it was 2.9 s. i run the tests, then i land it.' },
  ];
}

export const ASK = 'make the product page load under a second, and add search with size filters';

export function world(now: number): Ev[] {
  return [
    prefs(),
    projects({ agents: 1, working: 0, waits: 0 }),
    { ev: 'welcome', project: P, proto: 1, workspace: PATH, name: 'acme', cmds: ['subscribe', 'send', 'slash'] },
    agents([{ name: 'main', status: 'idle' }], now),
    { ev: 'growth', project: P, growth: { jobs_done: 140, days_used: 21, artifacts_made: 60, since_ms: now - 21 * 24 * 60 * MIN } },
    { ev: 'agents', project: HOME, agents: [{ name: 'main', main: true, status: 'idle', archived: false, title: '', purpose: '', since_ms: now, waits: 0, watching: [], dir: 'main' }] },
    { ev: 'cards', project: P, cards: [] },
    { ev: 'thread', project: P, agent: 'main', before: null, more: false, entries: history(now) },
    { ev: 'thread', project: P, agent: 'perf', before: null, more: false, entries: perfThread(now + 12_000) },
    // perf's land, as the commit view asks for it (the stub answers `diff` with merged = the sha)
    { ev: 'diff', project: P, agent: 'main', base: 'main', merged: LAND_SHA, commit: LAND_SHA, files: LAND_FILES, note: 'the product page loads in 0.8 s' },
  ];
}

export const LAND_SHA = '4e1c9a2';

export const LAND_FILES = [
  {
    path: 'src/pages/product.tsx', status: 'modified', add: 9, del: 4,
    hunks: [{
      header: '@@ -12,9 +12,14 @@ export default function Product', head: 'export default function Product', lines: [
        { kind: 'ctx', old: 12, new: 12, text: 'export default function Product({ id }: Props) {' },
        { kind: 'del', old: 13, text: '  const product = useProduct(id);' },
        { kind: 'del', old: 14, text: '  const reviews = useReviews(id);' },
        { kind: 'add', new: 13, text: '  const product = useProduct(id, { prefetch: true });' },
        { kind: 'add', new: 14, text: '  const reviews = useDeferred(() => useReviews(id));' },
        { kind: 'ctx', old: 15, new: 15, text: '  return (' },
        { kind: 'del', old: 16, text: '    <Gallery images={product.images} />' },
        { kind: 'add', new: 16, text: '    <Gallery images={product.images} priority={1} />' },
      ],
    }],
  },
  {
    path: 'src/lib/images.ts', status: 'modified', add: 24, del: 3,
    hunks: [{
      header: '@@ -1,6 +1,27 @@', lines: [
        { kind: 'del', old: 1, text: "export const imageUrl = (src: string) => src;" },
        { kind: 'add', new: 1, text: "export const imageUrl = (src: string, w = 800) =>" },
        { kind: 'add', new: 2, text: '  `${CDN}/${src}?w=${w}&fm=avif&q=70`;' },
        { kind: 'ctx', old: 2, new: 3, text: '' },
        { kind: 'add', new: 4, text: '/** the widths the gallery asks for, smallest first */' },
        { kind: 'add', new: 5, text: 'export const WIDTHS = [400, 800, 1200] as const;' },
        { kind: 'add', new: 6, text: '' },
        { kind: 'add', new: 7, text: 'export const srcSet = (src: string) =>' },
        { kind: 'add', new: 8, text: "  WIDTHS.map((w) => `${imageUrl(src, w)} ${w}w`).join(', ');" },
      ],
    }],
  },
  { path: 'src/server/cache.ts', status: 'added', add: 8, del: 0, hunks: [{ header: '@@ -0,0 +1,8 @@', lines: [
    { kind: 'add', new: 1, text: "import { LRU } from './lru';" },
    { kind: 'add', new: 2, text: '' },
    { kind: 'add', new: 3, text: '/** a product read in the last minute is served from memory */' },
    { kind: 'add', new: 4, text: 'export const productCache = new LRU<string, Product>({ max: 500, ttl: 60_000 });' },
    { kind: 'add', new: 5, text: '' },
    { kind: 'add', new: 6, text: 'export const cachedProduct = (id: string) =>' },
    { kind: 'add', new: 7, text: '  productCache.get(id) ?? productCache.set(id, loadProduct(id));' },
  ] }] },
];
