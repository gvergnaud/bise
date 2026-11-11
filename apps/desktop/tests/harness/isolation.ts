// Every launch's own home, and the sentinel that fails a run that reaches outside it.
// Why (main m_9882): a harness run of the app inherited his REAL HOME, so the app's bise call
// started his real global hub (~/bise) through launchd at 22:33. So:
//   - testHome(): every launch gets a throwaway HOME (+ BISE_HOME inside it, TMPDIR, ~/bise),
//     made by the harness itself; harnessEnv() builds the app's whole environment from a short
//     allowlist plus that home: the caller's HOME, BISE_HOME, TMPDIR, SB_*, XDG_* never pass,
//     and a caller env that tries to set them (or a workspace outside the home) is refused;
//   - Sentinel: from launch to close it reads the process table (ps, with each process's own
//     environment, of which it keeps only HOME, BISE_HOME, BISE_TEST_HOME and SB_STATE_DIR and
//     never prints anything else). It judges only the run's own processes (main m_10044): under
//     the app's pid, or carrying the run's home in their env. One of them on his real ~/.bise
//     (outside the run's home and the test worlds ~/.bise/gate/w-*), or running sbd or launchctl
//     that doesn't point into the run's home, is a breach: the app is killed at once and the run
//     fails. His processes and other agents' are never judged, signalled or killed;
//     at close it kills what the run left (its tree, and what it started that carries its home: a
//     hub started through launchd) and removes the launchd jobs that ran them, then deletes the home.
import { execFile, execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readdirSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

export type TestHome = { root: string; home: string; biseHome: string; tmp: string };

/**
 * The file that marks a home the harness made or adopted: paths/hook.ts (loaded before the app's
 * main) starts Electron only on a HOME that holds it (architect m_59538: told by a marker, never by
 * its path). The same name as paths/hook.ts's MARKER (self/isolation.spec.ts pins both).
 */
export const TEST_HOME_MARKER = '.bise-test-home';

const mark = (home: string): void => {
  writeFileSync(path.join(home, TEST_HOME_MARKER), 'a test home made by apps/desktop/tests/harness (isolation.ts)\n');
};

/** A throwaway home under /tmp (short: bise's sockets live under BISE_HOME, 104 bytes max). */
export function testHome(): TestHome {
  const root = mkdtempSync(path.join('/tmp', 'bh-'));
  const home = path.join(root, 'home');
  const biseHome = path.join(home, '.bise');
  const tmp = path.join(root, 'tmp');
  for (const d of [home, biseHome, tmp, path.join(home, 'bise')]) mkdirSync(d, { recursive: true });
  mark(home);
  return { root, home, biseHome, tmp };
}

/** His real home, from the user database (never $HOME, which a caller can set). */
export const realHome = (): string => os.userInfo().homedir;

/**
 * Why `dir` can't be a launch's home, or null (pure: self/isolation.spec.ts). `dir` is a realpath.
 * Only an ambient-qa world's home (a folder under ~/.bise/gate/) or another launch's /tmp/bh-* home:
 * never his home, his ~/.bise or ~/bise, nor anything else.
 */
export function homeProblem(dir: string, real: string): string | null {
  const gate = path.join(real, '.bise', 'gate');
  if (inTestWorld(dir, real)) return null;
  const tmp = path.relative('/private/tmp', dir).split(path.sep)[0] ?? '';
  if (inside(dir, '/private/tmp') && tmp.startsWith('bh-')) return null;
  return `${dir} is neither an ambient-qa world's home (under ${gate}/w-*) nor a /tmp/bh-* home`;
}

/**
 * An existing home (qa-flows m_9931: an ambient-qa world's, its hubs already up) used as a launch's
 * test home: same env, same sentinel, but never deleted (the world owns it); the run's own tmp and
 * userdata go in a fresh /tmp/bh-* root, which is.
 */
export function adoptHome(dir: string): TestHome {
  let home: string;
  try { home = realpathSync(dir); } catch { throw new Error(`harness: home ${dir} doesn't exist`); }
  const why = homeProblem(home, realpathSync(realHome()));
  if (why) throw new Error(`harness: refused home: ${why}`);
  const root = mkdtempSync(path.join('/tmp', 'bh-'));
  const tmp = path.join(root, 'tmp');
  mkdirSync(tmp, { recursive: true });
  mark(home);
  return { root, home, biseHome: path.join(home, '.bise'), tmp };
}

/**
 * Electron's folders the app reports (name -> path) that are outside the run's own folders (its home
 * and its /tmp/bh-* root: userdata, tmp), as 'name path' lines. Pure; `dirs` and the paths are
 * realpaths. launch.ts refuses a launch with any (main m_59528: getPath('home') was his real home).
 */
export function pathsOutside(paths: Record<string, string>, dirs: string[]): string[] {
  return Object.entries(paths).filter(([, p]) => !dirs.some((d) => inside(p, d))).map(([n, p]) => `${n} ${p}`);
}

/**
 * What a run may never add to his real home, as names (main m_59528, architect m_59538): his home's
 * top-level entries and ~/Library/CloudStorage's (a new File Provider domain folder). Never listed
 * inside a domain: listing one makes his real appex enumerate, reaching his real service.
 */
export function realHomeNames(real: string): string[] {
  const ls = (d: string): string[] => { try { return readdirSync(d); } catch { return []; } };
  return [...ls(real), ...ls(path.join(real, 'Library', 'CloudStorage')).map((n) => path.join('Library', 'CloudStorage', n))];
}

/** The names in `after` that were not in `before` (realHomeNames), as breaches. Pure. */
export function newInRealHome(before: string[], after: string[], real: string): string[] {
  const was = new Set(before);
  return after.filter((n) => !was.has(n)).map((n) => `the run created ${path.join(real, n)} in his real home`);
}

const KEEP = ['PATH', 'LANG', 'LC_ALL', 'LC_CTYPE', 'USER', 'LOGNAME', 'SHELL', 'TERM', '__CF_USER_TEXT_ENCODING'];
const FORBIDDEN = /^(HOME|BISE_HOME|BISE_HOME_WORKSPACE|TMPDIR|XDG_.*|SB_.*|BISE_AGENT.*|BISE_ROLE|BEND_.*|CODEX_HOME|CLAUDE_CONFIG_DIR)$/;

const inside = (p: string, dir: string): boolean => {
  const r = path.relative(dir, path.resolve(p));
  return r === '' || (!r.startsWith('..') && !path.isAbsolute(r));
};

/**
 * The app's whole environment: the allowlist, the test home, then `harness` (the harness's own
 * BISE_HARNESS_* / BISE_DESKTOP_* values) and `caller` (LaunchOptions.env). Throws on a caller key
 * that would choose a home, or a workspace outside the test home. Pure.
 */
export function harnessEnv(h: TestHome, harness: Record<string, string>, caller: Record<string, string> = {}, base: NodeJS.ProcessEnv = process.env): Record<string, string> {
  for (const [k, v] of Object.entries(caller)) {
    if (FORBIDDEN.test(k)) throw new Error(`harness: a launch can't set ${k}: every launch has its own throwaway home (main m_9882)`);
    if (k === 'BISE_DESKTOP_WORKSPACE' && v !== '' && !inside(v, h.home)) throw new Error(`harness: BISE_DESKTOP_WORKSPACE=${v} is outside the test home ${h.home}`);
  }
  const env: Record<string, string> = {};
  for (const k of KEEP) { const v = base[k]; if (v !== undefined) env[k] = v; }
  return { ...env, HOME: h.home, BISE_HOME: h.biseHome, TMPDIR: h.tmp, ...harness, ...caller };
}

/** stateDir: SB_STATE_DIR, the hub state folder a test hub (a gate's e2e) points elsewhere than ~/.bise;
 * testHome: BISE_TEST_HOME, the jail the app sets for every bise it starts */
type Proc = { pid: number; ppid: number; command: string; home?: string; biseHome?: string; testHome?: string; stateDir?: string };

/** The process table with each process's HOME and BISE_HOME (ps -E); nothing else of the env is kept. */
function processes(): Promise<Proc[]> {
  return new Promise((resolve) => {
    execFile('/bin/ps', ['-axwwE', '-o', 'pid=,ppid=,command='], { maxBuffer: 64 * 1024 * 1024 }, (err, out) => {
      if (err) { resolve([]); return; }
      const procs: Proc[] = [];
      for (const line of out.split('\n')) {
        const m = /^\s*(\d+)\s+(\d+)\s+(.*)$/.exec(line);
        if (!m) continue;
        const rest = m[3] ?? '';
        const envAt = rest.search(/\s[A-Za-z_][A-Za-z0-9_]*=/);
        const command = envAt < 0 ? rest : rest.slice(0, envAt);
        const home = / HOME=(\S+)/.exec(rest)?.[1];
        const biseHome = / BISE_HOME=(\S+)/.exec(rest)?.[1];
        const stateDir = / SB_STATE_DIR=(\S+)/.exec(rest)?.[1];
        const testHome = / BISE_TEST_HOME=(\S+)/.exec(rest)?.[1];
        procs.push({ pid: Number(m[1]), ppid: Number(m[2]), command, ...(home ? { home } : {}), ...(biseHome ? { biseHome } : {}),
          ...(testHome ? { testHome } : {}), ...(stateDir ? { stateDir } : {}) });
      }
      resolve(procs);
    });
  });
}

const isHub = (c: string): boolean => /(^|\/)(bise|sb)\s+sbd\b/.test(c) || /(^|\/)sbd(\s|$)/.test(c);
const isLaunchctl = (c: string): boolean => /(^|\/)launchctl(\s|$)/.test(c);

/** An ambient-qa test world (~/.bise/gate/w-*, world.py): not his, whoever runs in it. Pure. */
export function inTestWorld(p: string, real: string): boolean {
  const gate = path.join(real, '.bise', 'gate');
  const first = path.relative(gate, path.resolve(p)).split(path.sep)[0] ?? '';
  return inside(p, gate) && first.startsWith('w-');
}

/** The process carries the run's own home in its env (HOME, BISE_HOME or BISE_TEST_HOME). Pure. */
export const carriesRun = (p: Proc, h: TestHome): boolean =>
  [p.home, p.biseHome, p.testHome].some((v) => v !== undefined && inside(v, h.home));

/**
 * What breaks the isolation in one table (pure: self/isolation.spec.ts). Only the run's own processes
 * are judged (main m_10044, lead m_10046): those under the app's pid (now, or `seen` there earlier)
 * and those carrying the run's home in their env. His processes (his TUI starting a hub, his app)
 * and other agents' (their worlds, gates) are never the run's, whatever they do. A run's process
 * breaks it when it names his real ~/.bise in its env (outside the run's home and the test worlds),
 * or runs sbd or launchctl that doesn't point into the run's home.
 */
export function breaches(procs: Proc[], baseline: Set<number>, appPid: number, h: TestHome, real: string, seen: Set<number> = new Set()): string[] {
  const parent = new Map(procs.map((p) => [p.pid, p.ppid]));
  const underApp = (pid: number): boolean => {
    if (seen.has(pid)) return true;
    for (let p = pid, i = 0; p > 1 && i < 64; i++) { if (p === appPid) return true; p = parent.get(p) ?? 0; }
    return false;
  };
  const out: string[] = [];
  const realBise = path.join(real, '.bise');
  const his = (v: string | undefined): boolean => v !== undefined && inside(v, realBise) && !inside(v, h.home) && !inTestWorld(v, real);
  for (const p of procs) {
    if (baseline.has(p.pid) || !(underApp(p.pid) || carriesRun(p, h))) continue;
    const line = `pid ${p.pid}: ${p.command.slice(0, 120)}`;
    if (his(p.biseHome) || (p.biseHome === undefined && p.home !== undefined && his(path.join(p.home, '.bise')))) {
      out.push(`a process of the run on his real bise home (${line})`);
      continue;
    }
    const hub = isHub(p.command), lctl = isLaunchctl(p.command);
    if (!hub && !lctl) continue;
    const pointsHome = p.command.includes(h.root) || p.command.includes(h.home) || carriesRun(p, h);
    if (!pointsHome) out.push(`the run ran ${hub ? 'sbd' : 'launchctl'} outside the test home (${line})`);
  }
  return out;
}

/** The pid a process names as its owner with `--parent <pid>` in its command line. Pure. */
export function declaredOwner(command: string): number | undefined {
  const m = /(?:^|\s)--parent(?:=|\s+)(\d+)(?=\s|$)/.exec(command);
  const pid = m ? Number(m[1]) : NaN;
  return pid > 1 ? pid : undefined;
}

/**
 * The process belongs to an adopted world (up before the run): a baseline ancestor of it carries
 * the run's home, or the owner it declares with `--parent <pid>` is such a world process or has
 * one above it. A helper started detached (`nohup ... &` from a sh that exits: the plugins bridge,
 * bend/runtime/plugins.bend) has ppid 1 from birth, so only its declared owner ties it to the
 * world whose REPL started it (bridge-exit, agents/new on 1a7160891). An owner gone, or outside
 * any world: not in one, so killed as left by the run. Pure (self/isolation.spec.ts).
 */
export function inWorld(pid: number, procs: Proc[], baseline: Set<number>, h: TestHome): boolean {
  const byPid = new Map(procs.map((p) => [p.pid, p]));
  const world = (q: number): boolean => {
    const a = byPid.get(q);
    return baseline.has(q) && a !== undefined && carriesRun(a, h);
  };
  // up from the process: its parent, or (reparented to launchd) the owner it declares
  const up = (p: Proc): number => (p.ppid > 1 ? p.ppid : declaredOwner(p.command) ?? 0);
  let q = byPid.get(pid);
  for (let i = 0; q && i < 64; i++) {
    const next = up(q);
    if (next <= 1) return false;
    if (world(next)) return true;
    q = byPid.get(next);
  }
  return false;
}

/** Every process under `root` in one table. Pure. */
export function descendants(procs: Proc[], root: number): number[] {
  const kids = new Map<number, number[]>();
  for (const p of procs) kids.set(p.ppid, [...(kids.get(p.ppid) ?? []), p.pid]);
  const out: number[] = [];
  const walk = (pid: number, depth: number): void => {
    if (depth > 64) return;
    for (const k of kids.get(pid) ?? []) { out.push(k); walk(k, depth + 1); }
  };
  if (root > 1) walk(root, 0);
  return out;
}

/**
 * Notes every process under the app's pid now, with the command it runs now (pure but for `seen`,
 * which it updates: self/isolation.spec.ts). A pid under the app is the run's whatever it ran
 * before, so its command is always the latest: a child caught between fork and exec shows its
 * parent's command (a hub's REPL in its pre_exec, `bise sbd --workspace ...`), and keeping that one
 * made the REPL, once exec'd, 'not the run's' at close: never killed with the hub, then counted as
 * left in the test home (stop-leak: inbox/answer-everywhere 'its hub stopped', 3 repl-live on
 * 6a79da476). A pid no longer under the app keeps the command it had there.
 */
export function noteSeen(seen: Map<number, string>, procs: Proc[], appPid: number): void {
  const cmd = new Map(procs.map((p) => [p.pid, p.command]));
  for (const pid of descendants(procs, appPid)) seen.set(pid, cmd.get(pid) ?? '');
}

/** Reads the process table every second from launch to close; the first breach calls onBreach. */
export class Sentinel {
  readonly breaches: string[] = [];
  private timer: NodeJS.Timeout | undefined;
  private baseline = new Set<number>();
  private appPid = -1;
  private busy = false;
  /** every pid ever seen under the app's pid, with its last command there (noteSeen): an orphan
   * (ppid 1 once the app is gone) is still the run's, a pid reused since by another command is not */
  private readonly seen = new Map<number, string>();

  // plain fields, no parameter properties: test:land loads this file under node's strip-only types
  private readonly h: TestHome;
  private readonly onBreach: (why: string) => void;

  constructor(h: TestHome, onBreach: (why: string) => void) {
    this.h = h;
    this.onBreach = onBreach;
  }

  /** his real home's top-level names and CloudStorage's at the start (realHomeNames) */
  private realBefore: string[] = [];

  async start(): Promise<void> {
    this.realBefore = realHomeNames(realHome());
    this.baseline = new Set((await processes()).map((p) => p.pid));
    this.timer = setInterval(() => { void this.scan(); }, 1000);
  }

  /** The app's pid, once launched: its tree is checked from then on. */
  follow(pid: number): void {
    this.appPid = pid;
  }

  private note(procs: Proc[]): void {
    noteSeen(this.seen, procs, this.appPid);
  }

  async scan(): Promise<void> {
    if (this.busy) return;
    this.busy = true;
    try {
      const procs = await processes();
      this.note(procs);
      const found = breaches(procs, this.baseline, this.appPid, this.h, realHome(), new Set(this.ours(procs)));
      const fresh = found.filter((b) => !this.breaches.includes(b));
      if (fresh.length && !this.breaches.length) this.onBreach(fresh.join('; '));
      this.breaches.push(...fresh);
    } finally {
      this.busy = false;
    }
  }

  /** The seen pids still running the command they ran under the app (never a reused pid). */
  private ours(procs: Proc[]): number[] {
    return procs.filter((p) => this.seen.get(p.pid) === p.command).map((p) => p.pid);
  }

  /**
   * The run's processes now: under the app's pid, or seen there and still running the same command
   * (its core, helper, their children; orphans have ppid 1 once the app is gone).
   */
  async tree(): Promise<number[]> {
    const procs = await processes();
    this.note(procs);
    return this.ours(procs);
  }

  /**
   * Last scan, then kills what the run left, and only that (main m_10044: never a process outside
   * the run, whatever it is): the run's tree (`tree`, read before the app was closed, checked again
   * against the table now) and the processes the run started that carry its home in their env (a
   * hub started through launchd), not those of an adopted world (up before the run, or under a
   * process that was), removing the launchd jobs that ran them. Returns what was left.
   */
  async stop(tree: number[] = []): Promise<string[]> {
    if (this.timer) clearInterval(this.timer);
    await this.scan();
    const now = await processes();
    this.note(now);
    const mine = new Set(this.ours(now));
    for (const pid of tree) if (mine.has(pid) || !now.some((p) => p.pid === pid)) mine.add(pid);
    for (const pid of mine) { try { process.kill(pid, 'SIGKILL'); } catch { /* gone */ } }
    const after = await processes();
    const left = after.filter((p) => p.pid !== process.pid && !this.baseline.has(p.pid) && carriesRun(p, this.h) && !inWorld(p.pid, after, this.baseline, this.h));
    const cleaned: string[] = [];
    if (left.length) {
      const pids = new Set(left.map((p) => p.pid));
      let jobs = '';
      try { jobs = execFileSync('/bin/launchctl', ['list']).toString(); } catch { /* none */ }
      for (const line of jobs.split('\n')) {
        const [pid, , label] = line.trim().split(/\s+/);
        if (label && pids.has(Number(pid))) {
          try { execFileSync('/bin/launchctl', ['remove', label]); cleaned.push(`launchd job ${label}`); } catch { /* gone */ }
        }
      }
      for (const p of left) {
        try { process.kill(p.pid, 'SIGKILL'); cleaned.push(`pid ${p.pid}`); } catch { /* gone */ }
      }
    }
    // nothing new in his real home's top level nor in ~/Library/CloudStorage (main m_59528)
    for (const b of newInRealHome(this.realBefore, realHomeNames(realHome()), realHome())) if (!this.breaches.includes(b)) this.breaches.push(b);
    rmSync(this.h.root, { recursive: true, force: true });
    return cleaned;
  }
}
