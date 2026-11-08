import type { SpawnOptions } from 'node:child_process';
import { EventEmitter, once } from 'node:events';
import { mkdtempSync, readFileSync, writeSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { PassThrough } from 'node:stream';
import { describe, expect, it, vi } from 'vitest';
import type { CoreExit, CoreState } from '../../shared/api';
import { Backoff, configOf, coreArgs } from './env';
import { initializeLine, INITIALIZED_LINE, LineSplitter, NL, notAnEventLog, readLine, refusedLog, requestOf } from './lines';
import { CoreProcess } from './process';

describe('lines', () => {
  it('gives a line only once its newline came, at any chunk boundary', () => {
    const s = new LineSplitter();
    expect(s.feed('{"ev":"a"')).toEqual([]);
    expect(s.feed('}' + NL + '{"ev"')).toEqual(['{"ev":"a"}']);
    expect(s.feed(':"b"}' + String.fromCharCode(13) + NL + NL)).toEqual(['{"ev":"b"}', '']);
    expect(s.pending).toBe('');
  });

  it('a notification is the event the store reads; an answer has its id; anything else is null', () => {
    expect(readLine('{"jsonrpc":"2.0","method":"hub/welcome","params":{"project":"p"}}')).toEqual({ kind: 'event', ev: { ev: 'welcome', project: 'p' } });
    expect(readLine('{"jsonrpc":"2.0","method":"core/phase","params":{"phase":"idle"}}')).toEqual({ kind: 'event', ev: { ev: 'phase', phase: 'idle' } });
    expect(readLine('{"jsonrpc":"2.0","method":"core/hush"}')).toEqual({ kind: 'event', ev: { ev: 'hush' } });
    expect(readLine('{"jsonrpc":"2.0","id":3,"result":{}}')).toEqual({ kind: 'answer', id: 3, result: {}, error: null });
    expect(readLine('{"jsonrpc":"2.0","id":4,"error":{"code":-32602,"message":"answer: no card id"}}')).toEqual({ kind: 'answer', id: 4, result: null, error: { code: -32602, message: 'answer: no card id' } });
    // the older lines, another family, a request, not JSON: not the protocol
    expect(readLine('{"ev":"welcome","project":"p"}')).toBeNull();
    expect(readLine('{"jsonrpc":"2.0","method":"other/x","params":{}}')).toBeNull();
    expect(readLine('{"jsonrpc":"2.0","id":1,"method":"core/x"}')).toBeNull();
    expect(readLine('[1]')).toBeNull();
    expect(readLine('not json')).toBeNull();
    expect(readLine('')).toBeNull();
  });

  it("a command is a request: hub/<cmd> with a project's hub's tag and its project, core/<cmd> otherwise", () => {
    expect(requestOf(5, { cmd: 'stop', project: 'p', agent: 'a' })).toEqual({ method: 'hub/stop', line: '{"jsonrpc":"2.0","id":5,"method":"hub/stop","params":{"project":"p","agent":"a"}}' + NL });
    expect(requestOf(6, { cmd: 'stop', agent: 'a' })?.method).toBe('core/stop');
    expect(requestOf(7, { cmd: 'skills', project: 'p' })?.method).toBe('core/skills');
    expect(requestOf(8, { cmd: 'computer', act: 'peek', key: 'h.d' })?.line).toBe('{"jsonrpc":"2.0","id":8,"method":"core/computer","params":{"act":"peek","key":"h.d"}}' + NL);
    expect(requestOf(1, { ev: 'x' })).toBeNull();
    expect(requestOf(1, 'stop')).toBeNull();
    expect(requestOf(1, null)).toBeNull();
    expect(JSON.parse(initializeLine('1.2'))).toEqual({ jsonrpc: '2.0', id: 0, method: 'initialize', params: { proto: 1, client: { name: 'bise-desktop', version: '1.2' } } });
    expect(JSON.parse(INITIALIZED_LINE)).toEqual({ jsonrpc: '2.0', method: 'initialized' });
    expect(refusedLog('core/answer', { code: -32602, message: 'answer: no card id' })).toBe('core: core/answer refused (-32602): answer: no card id');
    expect(refusedLog('core/provider_use', { code: -32602, message: 'key "sk-SECRET"' })).toBe('core: core/provider_use refused (-32602)');
  });

  it("logs a line that is not an event, never a computer use still's bytes (S29)", () => {
    expect(notAnEventLog('not an event')).toBe('core: not an event: not an event');
    expect(notAnEventLog('x'.repeat(300))).toBe(`core: not an event: ${'x'.repeat(200)}`);
    const broken = '{"jsonrpc":"2.0","method":"core/computer_peek","params":{"key":"h.perf","shot":{"data":"U0VDUkVU';
    expect(notAnEventLog(broken)).toBe(`core: not an event: a core/computer_peek line (${String(broken.length)} bytes, not logged)`);
    expect(notAnEventLog('{"ev":"computer_peek","shot":{"data":"U0VDUkVU')).not.toContain('U0VDUkVU');
    expect(notAnEventLog('"shot":{"data":"U0VDUkVU')).not.toContain('U0VDUkVU');
    expect(notAnEventLog('{"jsonrpc":"2.0","method":"core/provider_use","params":{"key":"sk-SECRET"')).not.toContain('SECRET');
  });

  it('never logs the usage counts\' view: it holds the install id (architect m_56355)', () => {
    const id = '0b6b3a4e-9f1c-4d2a-8e5f-2c7d9a1b3e4f';
    const broken = `{"jsonrpc":"2.0","method":"core/telemetry","params":{"share":true,"id":"${id}"`;
    expect(notAnEventLog(broken)).toBe(`core: not an event: a core/telemetry line (${String(broken.length)} bytes, not logged)`);
    expect(notAnEventLog(`{"ev":"telemetry","id":"${id}"`)).not.toContain(id);
  });
});

describe('env', () => {
  it('never runs PATH\'s bise: a path or nothing', () => {
    expect(configOf('bise', {})).toBeNull();
    expect(configOf(null, {})).toBeNull();
    expect(configOf('/opt/bise', { BISE_DESKTOP_WORKSPACE: '/w' })).toEqual({ bise: '/opt/bise', workspace: '/w' });
    expect(configOf('/opt/bise', {})).toEqual({ bise: '/opt/bise', workspace: '' });
  });

  it('runs ambient-core on the workspace, or --home', () => {
    expect(coreArgs('')).toEqual(['ambient-core', '--home']);
    expect(coreArgs('/w')).toEqual(['ambient-core', '--workspace', '/w']);
  });

  it('backs off 0.5 doubling to 30, back to 0.5 after a healthy run', () => {
    const b = new Backoff();
    expect([b.after(0), b.after(1), b.after(1)]).toEqual([0.5, 1, 2]);
    for (let i = 0; i < 10; i += 1) b.after(0);
    expect(b.after(0)).toBe(30);
    expect(b.after(60)).toBe(0.5);
  });
});

/**
 * The core is a fake child in this process, never a real one: a real `bise`
 * (a node script, a sh one) took seconds to start at load ~27 and 4 tests hit
 * vitest's 5 s (fast-loop m_54599). The fake keeps what CoreProcess sees of
 * node's child_process, as node does it (checked once against a real spawn):
 * 'spawn' on the next tick, stdout lines, stderr into the stdio[2] fd it was
 * given, 'exit' (code, signal) after the lines it wrote, kill() ends it with
 * that signal; a path with no program fails as ENOENT does: 'error' then
 * 'close', no 'spawn', no 'exit', no pid. A run of a real core process is the
 * end-to-end tests' (app ↔ core, docs/test-plan-min.md).
 */
interface FakeRun {
  readonly args: readonly string[];
  readonly cwd: string;
  /** a stderr line, into the log file: only while spawning (CoreProcess closes its fd right after) */
  readonly err: (line: string) => void;
  readonly out: (line: string) => void;
  readonly exit: (code: number | null, signal?: NodeJS.Signals) => void;
}
/** A fake `bise`: runs at spawn; what it returns gets each stdin line. */
type Program = (run: FakeRun) => ((line: string) => void) | undefined;

const fake = vi.hoisted(() => ({ programs: new Map<string, Program>(), runs: [] as EventEmitter[] }));

vi.mock('node:child_process', async (importOriginal) => ({
  ...(await importOriginal<typeof import('node:child_process')>()),
  spawn: (cmd: string, args: readonly string[], opts: SpawnOptions) => fakeSpawn(cmd, args, opts),
}));

let nextPid = 1000;

function fakeSpawn(cmd: string, args: readonly string[], opts: SpawnOptions): EventEmitter {
  const stdin = new PassThrough();
  const stdout = new PassThrough();
  let exited = false;
  const child = Object.assign(new EventEmitter(), {
    stdin,
    stdout,
    pid: undefined as number | undefined,
    kill: (signal: NodeJS.Signals = 'SIGTERM'): boolean => {
      run.exit(null, signal);
      return true;
    },
  });
  fake.runs.push(child);
  const program = fake.programs.get(cmd);
  if (program === undefined) {
    process.nextTick(() => {
      child.emit('error', Object.assign(new Error(`spawn ${cmd} ENOENT`), { code: 'ENOENT', errno: -2, syscall: `spawn ${cmd}`, path: cmd }));
      child.emit('close', -2, null);
    });
    return child;
  }
  child.pid = nextPid;
  nextPid += 1;
  const fd = Array.isArray(opts.stdio) ? opts.stdio[2] : undefined;
  let spawning = true;
  const run: FakeRun = {
    args,
    cwd: typeof opts.cwd === 'string' ? opts.cwd : process.cwd(),
    err: (line) => {
      if (!spawning) throw new Error('fake core: stderr is written while spawning only');
      if (typeof fd === 'number') writeSync(fd, line + NL);
    },
    out: (line) => {
      if (!exited) stdout.write(line + NL);
    },
    exit: (code, signal) => {
      if (exited) return;
      exited = true;
      // after the lines it wrote, as a real child's exit comes after its output
      setImmediate(() => {
        child.emit('exit', code, signal ?? null);
        stdout.end();
        child.emit('close', code, signal ?? null);
      });
    },
  };
  const onLine = program(run);
  spawning = false;
  const lines = new LineSplitter();
  stdin.setEncoding('utf8');
  stdin.on('data', (chunk: string) => {
    for (const l of lines.feed(chunk)) if (!exited) onLine?.(l);
  });
  stdin.on('end', () => {
    run.exit(0);
  });
  process.nextTick(() => {
    if (!exited) child.emit('spawn');
  });
  return child;
}

/** Registers a fake program under a path of its own, for the core's config. */
function program(p: Program): string {
  const path = `/fake/bise-${String(fake.programs.size)}`;
  fake.programs.set(path, p);
  return path;
}

/**
 * A fake `ambient-core`: speaks JSON-RPC as it does: answers initialize, then (after
 * initialized) a welcome and `after`'s lines; answers each request {} and echoes it;
 * refuses core/nope; exits 3 on core/quit.
 */
function rpcCore(after: readonly string[] = ['not an event']): string {
  return program((run) => {
    const out = (o: object): void => {
      run.out(JSON.stringify({ jsonrpc: '2.0', ...o }));
    };
    const note = (method: string, params: unknown): void => {
      out({ method, params });
    };
    run.err(`fake core ${run.args.join(' ')}`);
    return (l) => {
      const m = JSON.parse(l) as { id?: number; method: string; params?: unknown };
      if (m.method === 'initialize') out({ id: m.id, result: { proto: 1, methods: [], notifications: [] } });
      else if (m.method === 'initialized') {
        note('hub/welcome', { project: 'p', proto: 1, workspace: run.cwd, name: run.args.join(' ') });
        for (const a of after) run.out(a);
      } else if (m.method === 'core/quit') run.exit(3);
      else if (m.method === 'core/nope') out({ id: m.id, error: { code: -32601, message: 'unknown method' } });
      else {
        out({ id: m.id, result: {} });
        note('core/echo', { method: m.method, params: m.params });
      }
    };
  });
}

/**
 * The hooks' calls, and a wait on them: `until` resolves at the call that makes
 * its condition true. No clock: a wait that never ends is vitest's timeout.
 */
function hooked(): { readonly until: (cond: () => boolean) => Promise<void>; readonly into: <T>(list: T[]) => (x: T) => void } {
  const waits = new Set<() => void>();
  return {
    until: (cond) =>
      new Promise((resolve) => {
        const check = (): void => {
          if (!cond()) return;
          waits.delete(check);
          resolve();
        };
        waits.add(check);
        check();
      }),
    into: (list) => (x) => {
      list.push(x);
      for (const w of [...waits]) w();
    },
  };
}

describe('CoreProcess', () => {
  it('runs the core, passes events and commands as lines, restarts it after an exit, stops it', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'bise-core-test-'));
    const logFile = join(dir, 'core.log');
    const events: Record<string, unknown>[] = [];
    const states: CoreState[] = [];
    const logs: string[] = [];
    const h = hooked();
    const core = new CoreProcess(
      { config: { bise: rpcCore(), workspace: dir }, logFile, env: { PATH: process.env['PATH'], SB_AGENT: 'leak' }, scale: 10 },
      { onEvent: h.into(events), onState: h.into(states), log: h.into(logs) },
    );
    expect(core.send({ cmd: 'stop', project: 'p', agent: 'a' })).toBe(false);
    core.start();
    await h.until(() => events.length === 1);
    expect(events[0]).toMatchObject({ ev: 'welcome', name: `ambient-core --workspace ${dir}`, workspace: dir });
    expect(states).toEqual(['starting', 'up']);
    await h.until(() => logs.some((l) => l.startsWith('core: not an event: not an event')));

    expect(core.send({ cmd: 'stop', project: 'p', agent: 'a' })).toBe(true);
    expect(core.send({ nope: 1 })).toBe(false);
    await h.until(() => events.length === 2);
    expect(events[1]).toEqual({ ev: 'echo', method: 'hub/stop', params: { project: 'p', agent: 'a' } });
    // a refused request is logged with its method
    expect(core.send({ cmd: 'nope' })).toBe(true);
    await h.until(() => logs.some((l) => l === 'core: core/nope refused (-32601): unknown method'));

    // it exits: down, then started again after the backoff (0.5 * 10 ms here)
    core.send({ cmd: 'quit' }); // core/quit: the fake exits
    await h.until(() => events.filter((e) => e['ev'] === 'welcome').length === 2);
    expect(states).toEqual(['starting', 'up', 'down', 'starting', 'up']);

    core.stop();
    expect(core.state).toBe('down');
    expect(core.send({ cmd: 'stop', project: 'p', agent: 'a' })).toBe(false);
    // each run's stderr went to the log file
    expect(readFileSync(logFile, 'utf8').split(NL).filter((l) => l.startsWith('fake core'))).toEqual([`fake core ambient-core --workspace ${dir}`, `fake core ambient-core --workspace ${dir}`]);
  });

  it("passes a computer use still to the window and never logs it, whole or broken (S29, architect m_14385)", async () => {
    const dir = mkdtempSync(join(tmpdir(), 'bise-core-test-'));
    const data = 'U0VDUkVULVNDUkVFTi1CWVRFUw==';
    const peek = `{"jsonrpc":"2.0","method":"core/computer_peek","params":{"key":"h.perf","shot":{"data":"${data}","mime":"image/jpeg","width":640,"height":400,"at_ms":9}}}`;
    const events: Record<string, unknown>[] = [];
    const logs: string[] = [];
    const h = hooked();
    const core = new CoreProcess(
      { config: { bise: rpcCore([peek, peek.slice(0, 80), 'done']), workspace: dir }, env: { PATH: process.env['PATH'] }, scale: 10 },
      { onEvent: h.into(events), onState: () => undefined, log: h.into(logs) },
    );
    core.start();
    await h.until(() => logs.some((l) => l === 'core: not an event: done'));
    core.stop();
    expect(events.find((e) => e['ev'] === 'computer_peek')).toEqual({ ev: 'computer_peek', ...(JSON.parse(peek) as { params: object }).params });
    expect(logs.some((l) => l.includes('a core/computer_peek line'))).toBe(true);
    expect(logs.filter((l) => l.includes('U0VDUkVU') || l.includes('"shot"'))).toEqual([]);
  });

  it("hands a request's result back with the command as sent, and a refused one's error apart (A1, architect m_23524)", async () => {
    const dir = mkdtempSync(join(tmpdir(), 'bise-core-test-'));
    const answers: [Readonly<Record<string, unknown>>, unknown][] = [];
    const refused: [Readonly<Record<string, unknown>>, string][] = [];
    const logs: string[] = [];
    const h = hooked();
    const core = new CoreProcess(
      { config: { bise: rpcCore(), workspace: dir }, env: { PATH: process.env['PATH'] }, scale: 10 },
      { onEvent: () => undefined, onState: h.into<CoreState>([]), log: h.into(logs), onAnswer: (c, r) => {
          h.into(answers)([c, r]);
        },
        onRefused: (c, e) => {
          h.into(refused)([c, e.message]);
        },
      },
    );
    core.start();
    await h.until(() => core.state === 'up');
    const send = { cmd: 'send', project: 'p', agent: 'a', text: 'ok', cid: 4 };
    core.send(send);
    core.send({ cmd: 'nope' });
    await h.until(() => answers.length === 1 && refused.length === 1 && logs.some((l) => l.includes('core/nope refused')));
    core.stop();
    expect(answers).toEqual([[send, {}]]);
    expect(refused.map(([c]) => c)).toEqual([{ cmd: 'nope' }]);
  });

  it('a bise that cannot start is down and retried, never a crash', async () => {
    const states: CoreState[] = [];
    const h = hooked();
    const core = new CoreProcess(
      { config: { bise: '/nonexistent/bise', workspace: '' }, scale: 10 },
      { onEvent: () => undefined, onState: h.into(states), log: () => undefined },
    );
    core.start();
    await h.until(() => states.filter((s) => s === 'down').length >= 2);
    core.stop();
    expect(states.includes('up')).toBe(false);
  });

  it('says how a run ended: a panic is a crash with its message and the log; only this run\'s stderr counts', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'bise-core-test-'));
    const logFile = join(dir, 'core.log');
    writeFileSync(logFile, `an older run's last words${NL}`);
    const bise = program((run) => {
      run.err('ambient-core: hello');
      run.err("thread 'main' panicked at tui/src/ambient/core.rs:120:9:");
      run.err('no hub for sk-ant-SECRETSECRET1234');
      run.err('note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace');
      run.exit(101);
      return undefined;
    });
    const exits: CoreExit[] = [];
    const h = hooked();
    const core = new CoreProcess(
      { config: { bise, workspace: '' }, logFile, scale: 1000 },
      { onEvent: () => undefined, onState: () => undefined, log: () => undefined, onExit: h.into(exits) },
    );
    core.start();
    await h.until(() => exits.length === 1);
    core.stop();
    expect(exits[0]).toMatchObject({ how: 'code', code: 101, crash: true, log: logFile });
    expect(exits[0]?.last).toBe('no hub for …');
  });

  it('a core killed by SIGSEGV is a crash; one that exits 0 is clean; stop() says nothing', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'bise-core-test-'));
    const logFile = join(dir, 'core.log');
    const segv = program((run) => {
      run.err('about to fall');
      run.exit(null, 'SIGSEGV');
      return undefined;
    });
    const exits: CoreExit[] = [];
    const h = hooked();
    const core = new CoreProcess(
      { config: { bise: segv, workspace: '' }, logFile, scale: 1000 },
      { onEvent: () => undefined, onState: () => undefined, log: () => undefined, onExit: h.into(exits) },
    );
    core.start();
    await h.until(() => exits.length === 1);
    core.stop();
    expect(exits[0]).toMatchObject({ how: 'signal', signal: 'SIGSEGV', crash: true, last: 'about to fall', log: logFile });

    const done = program((run) => {
      run.err('done');
      run.exit(0);
      return undefined;
    });
    const clean: CoreExit[] = [];
    const c2 = new CoreProcess(
      { config: { bise: done, workspace: '' }, logFile, scale: 1000 },
      { onEvent: () => undefined, onState: () => undefined, log: () => undefined, onExit: h.into(clean) },
    );
    c2.start();
    await h.until(() => clean.length === 1);
    c2.stop();
    expect(clean[0]).toMatchObject({ how: 'clean', code: 0, crash: false, last: 'done', log: null });
    // a run stopped at once still ends (its own exit 0, or the SIGTERM): that end says nothing
    c2.start();
    const last = fake.runs.at(-1);
    c2.stop();
    if (last !== undefined) await once(last, 'exit');
    expect(clean).toHaveLength(1);
  });
});

