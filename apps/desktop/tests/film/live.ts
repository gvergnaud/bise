// The README's first shot (readme-film, Gabriel m_51458): a live-looking project on the scripted
// core, for the real window on his screen (onscreen.mts). A made-up shop, `acme`, the films' one
// world (signup-perf, cookie-banner, pricing-page, empty-state, onboarding, changelog, login-test),
// and a few other made-up projects in the sidebar. Nothing of his.
import { HOME, P, agentRow, type Ev, type Row } from './world';

const MIN = 60_000;
const H = 60 * MIN;
const PATH = '/Users/sam/acme';

const others = [
  { project: 'docs-7d0c5a11', name: 'acme-docs', busy: [0, 2, 0, 3, 1, 0, 0, 0, 0, 4, 1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0], last: 3 * H, news: { kind: 'artifact', id: 'api-ref', title: 'the API reference' } },
  { project: 'mobile-2b8e44f0', name: 'mobile-app', busy: [0, 0, 0, 5, 2, 0, 0, 1, 0, 0, 3, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0], last: 20 * H, news: { kind: 'landed', agent: 'push', sha: '5d1f0b2', target: 'main' } },
  { project: 'data-9a1c3e77', name: 'data-pipeline', busy: [0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0], last: 3 * 24 * H },
  { project: 'design-4f6d2b90', name: 'design-system', busy: [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0], last: 24 * H },
  { project: 'infra-0e5b7c3d', name: 'infra', busy: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], last: 4 * 24 * H },
];

export function liveProjects(o: { working: number; agents: number; now: number }): Ev {
  return {
    ev: 'projects',
    projects: [
      { project: HOME, name: 'bise', path: '/Users/sam/bise', git: false, home: true, order: 0, agents: 1, working: 0, waits: 0, running: true },
      { project: P, name: 'acme', path: PATH, git: true, home: false, order: 1, agents: o.agents, working: o.working, waits: 0, running: true, pulse: pulseOf(P, o.now) },
      ...others.map((x, i) => ({ project: x.project, name: x.name, path: `/Users/sam/${x.name}`, git: true, home: false, order: 2 + i, agents: 1, working: 0, waits: 0, running: true, pulse: pulseOf(x.project, o.now) })),
    ],
  };
}

const from = (now: number): number => now - 7 * 24 * H;
const ACME_BUSY = [0, 1, 0, 2, 3, 0, 1, 4, 0, 2, 5, 1, 0, 3, 6, 2, 1, 4, 7, 3, 5];
/** a project's row pulse (rows.ts Pulse): 21 buckets of 8 h, its last activity, its one news */
function pulseOf(project: string, now: number): Record<string, unknown> {
  if (project === P) return { from_ms: from(now), busy_ms: ACME_BUSY.map((h) => h * H / 2), last_ms: now };
  const x = others.find((o) => o.project === project);
  if (x === undefined) return { from_ms: from(now), busy_ms: [] };
  const news = x.news === undefined ? {} : { news: { at_ms: now - x.last, what: x.news } };
  return { from_ms: from(now), busy_ms: x.busy.map((h) => h * H / 2), last_ms: now - x.last, ...news };
}
export function pulses(now: number): Ev[] {
  return [P, ...others.map((x) => x.project)].map((project) => ({ ev: 'pulse', project, pulse: pulseOf(project, now) }));
}

/** acme's agents, as they are when the shot starts */
export const START: Row[] = [
  { name: 'main', status: 'idle' },
  { name: 'signup-perf', status: 'working', title: 'profiling the sign-up on mobile', purpose: 'sign-up under a second on mobile', doing: 'build', turn_ms: 9 * MIN },
  { name: 'empty-state', status: 'working', title: 'drawing the empty orders page', purpose: 'an empty state for the orders page', doing: 'edit', turn_ms: 4 * MIN },
  { name: 'login-test', status: 'working', title: 'fixing the flaky login test', purpose: 'the login test passes 50 times in a row', doing: 'tool', turn_ms: 6 * MIN },
  { name: 'changelog', status: 'waiting', title: 'waits for the next land', purpose: 'keep the changelog page current' },
  { name: 'onboarding', status: 'done', title: 'the checklist page is on main', purpose: 'a first-run checklist' },
  { name: 'cookie-banner', status: 'done', title: 'the banner sits above the footer', purpose: 'the cookie banner never covers checkout' },
];

export function liveAgents(rows: Row[], now: number): Ev {
  const places = rows.filter((r) => r.name !== 'main').map((r) => ({ id: `wt:sb/${r.name}`, branch: `sb/${r.name}`, agents: [r.name] }));
  return { ev: 'agents', project: P, agents: rows.map((r) => ({ ...agentRow(r, now), since_ms: now - (r.turn_ms ?? 2 * MIN) })), places };
}

const you = (pos: number, at: number, text: string): Ev => ({ pos, at_ms: at, kind: 'you', text, delivery: 'read', msg_id: pos });
const lead = (pos: number, at: number, text: string): Ev => ({ pos, at_ms: at, kind: 'agent', text });
const hand = (pos: number, at: number, agent: string, brief: string): Ev => {
  const text = `✚ main → new agent @${agent} : ${brief}`;
  return { pos, at_ms: at, kind: 'notice', text, notice: { level: 'info', text } };
};
const between = (pos: number, at: number, a: string, b: string, text: string): Ev => ({ pos, at_ms: at, kind: 'from_agent', text, from: a, to: b, msg: 100 + pos });

/** main's thread before the shot: a morning of work in acme */
export function liveHistory(now: number): Ev[] {
  const t = (m: number) => now - m * MIN;
  return [
    you(1, t(95), 'sign-up feels slow on mobile, and the cookie banner covers the checkout button'),
    lead(2, t(94), 'two tasks, in parallel: signup-perf profiles the sign-up on mobile, cookie-banner moves the banner out of the way. i come back when one lands.'),
    hand(3, t(94), 'signup-perf', 'sign-up under a second on mobile'),
    hand(4, t(94), 'cookie-banner', 'the cookie banner never covers checkout'),
    between(5, t(80), 'cookie-banner', 'signup-perf', 'i change src/layout/Footer.tsx for the banner: do you touch it?'),
    between(6, t(79), 'signup-perf', 'cookie-banner', 'no, it is yours. i stay in src/signup/.'),
    between(7, t(60), 'cookie-banner', 'main', 'landed: the banner sits above the footer.'),
    lead(8, t(59), 'cookie-banner landed: the banner sits above the footer, the checkout button stays clear on every phone size. signup-perf is at 1.4 s, it was 4.1 s; it goes on.'),
    you(9, t(40), 'also an empty state for the orders page, and the login test fails one run in ten'),
    lead(10, t(39), 'empty-state takes the orders page with the design system\'s illustration; login-test hunts the flaky run.'),
    hand(11, t(39), 'empty-state', 'an empty state for the orders page'),
    hand(12, t(39), 'login-test', 'the login test passes 50 times in a row'),
    between(13, t(25), 'login-test', 'signup-perf', 'does your change touch the session cookie?'),
    between(14, t(24), 'signup-perf', 'login-test', 'no, only the bundle and the images.'),
    between(15, t(20), 'onboarding', 'changelog', 'the checklist page is on main, for the changelog.'),
    lead(16, t(12), 'login-test found it: the test clicked before the form was ready. it runs it 50 times before it lands. nothing new for you.'),
  ];
}

export const LIVE_ASK = 'the pricing page: add a yearly plan, 2 months free';

export function liveWorld(now: number): Ev[] {
  return [
    { ev: 'prefs', prefs: { onboarded: true, theme: 'dark', context: { shot: true, url: true, file: true, window_text: false }, excluded_apps: [], quiet: { call: true, share: true, fullscreen: true, focus: true, batch_min: 20, away_card: true }, voice: { fn_hold: true, fn_twice: 'spotlight', language: 'auto' }, shots_keep_days: 7 } },
    liveProjects({ agents: START.length, working: 3, now }),
    ...pulses(now),
    { ev: 'welcome', project: P, proto: 1, workspace: PATH, name: 'acme', cmds: ['subscribe', 'send', 'slash'] },
    liveAgents(START, now),
    { ev: 'growth', project: P, growth: { jobs_done: 140, days_used: 21, artifacts_made: 60, since_ms: now - 21 * 24 * H } },
    { ev: 'agents', project: HOME, agents: [{ name: 'main', main: true, status: 'idle', archived: false, title: '', purpose: '', since_ms: now, waits: 0, watching: [], dir: 'main' }] },
    { ev: 'cards', project: P, cards: [] },
    { ev: 'thread', project: P, agent: 'main', before: null, more: false, entries: liveHistory(now) },
  ];
}
