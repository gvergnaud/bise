// The test side of the scripted stubs (stub-lib.mts): send events as the helper or the core, read
// the commands the app sent them. Events and commands are bise-proto's typed lines (src/proto;
// fixtures in '@/proto/fixtures.gen'), never hand-written shapes.
//   const helper = new FakeHelper();                 // bise-mac-helper (helper-stub.mts)
//   const core = new FakeCore();                     // bise ambient-core (core-stub.mts), fixture world
//   const s = await launchDesktop({ helper, core, route: 'settings' });
//   await helper.send({ ev: 'fn', act: 'talk_start', by: 'fn' });          // a fn press
//   await core.waitCmd((c) => c.cmd === 'send');                           // the app really sent it
import { chmodSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import net from 'node:net';
import path from 'node:path';
import type { AppCmd, CoreEv, HelperCmd, HelperEv, HubCmd, HubEv } from '../../../src/proto';

const NL = String.fromCharCode(10);
/** node runs the .ts stubs itself (until engines >= 22.18 makes it the default) */
const STRIP_TYPES = '--experimental-strip-types --disable-warning=ExperimentalWarning';
const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

class Stub<Ev, Cmd> {
  readonly dir: string;
  readonly sock: string;
  readonly log: string;
  /** what the app runs: a sh wrapper that execs node on the stub's .ts with type stripping on */
  protected readonly stub: string;

  constructor(source: string) {
    // a short path: a unix socket path must stay under 104 bytes
    this.dir = mkdtempSync(path.join('/tmp', 'bh-'));
    this.sock = path.join(this.dir, 's');
    this.log = path.join(this.dir, 'cmds.jsonl');
    // named like the stub (crash.spec finds the core by `pgrep -f core-stub`); exec keeps the pid
    this.stub = path.join(this.dir, path.basename(source, '.mts'));
    writeFileSync(this.stub, `#!/bin/sh${NL}NODE_OPTIONS="${STRIP_TYPES}" exec node '${source}' "$@"${NL}`);
    chmodSync(this.stub, 0o755);
  }

  /** Sends events as the stubbed process (each becomes one line on its stdout). */
  async send(...evs: Ev[]): Promise<void> {
    await this.raw(evs.map((e) => JSON.stringify(e)));
  }

  protected async raw(lines: string[]): Promise<void> {
    for (let i = 0; i < 50 && !existsSync(this.sock); i++) await sleep(100);
    await new Promise<void>((resolve, reject) => {
      const c = net.createConnection(this.sock, () => {
        c.end(lines.join(NL) + NL, () => { resolve(); });
      });
      c.on('error', reject);
    });
  }

  /** Every command the app sent so far: its whole lines only. The stub appends a line while a test
   * reads; a line read half written threw in JSON.parse and killed the test's own scripted hub
   * loop (history-perf's pages never answered under load, land-reds). */
  cmds(): Cmd[] {
    if (!existsSync(this.log)) return [];
    const text = readFileSync(this.log, 'utf8');
    const whole = text.slice(0, text.lastIndexOf(NL) + 1);
    return whole.split(NL).filter(Boolean).map((l) => JSON.parse(l) as Cmd);
  }

  /** Waits for a command matching `pred` (reads the log every 50 ms). */
  async waitCmd(pred: (c: Cmd) => boolean, timeoutMs = 5000): Promise<Cmd> {
    const end = Date.now() + timeoutMs;
    for (;;) {
      const hit = this.cmds().find(pred);
      if (hit) return hit;
      if (Date.now() > end) throw new Error(`no command matched in ${timeoutMs} ms; got ${JSON.stringify(this.cmds())}`);
      await sleep(50);
    }
  }

  dispose(): void {
    rmSync(this.dir, { recursive: true, force: true });
  }
}

/** The scripted bise-mac-helper: hello at start; without it a test run has no helper. */
export class FakeHelper extends Stub<HelperEv, HelperCmd> {
  constructor() {
    super(path.join(__dirname, 'helper-stub.mts'));
  }

  env(): Record<string, string> {
    return { BISE_DESKTOP_HELPER: this.stub, BISE_HARNESS_HELPER_SOCK: this.sock, BISE_HARNESS_HELPER_LOG: this.log };
  }
}

/**
 * The scripted bise ambient-core. Its start world: every fixture of rust/proto/fixtures (default),
 * the events given, or none. Without it a test run has no core (state 'down').
 */
export class FakeCore extends Stub<CoreEv | HubEv, AppCmd | HubCmd> {
  private readonly world: string | undefined;
  /** the answers to this command wait from the stub's start (`hold` before the app's first ask) */
  private readonly holdAtStart: string | undefined;

  /**
   * `opts.hold`: as `hold(cmd)`, armed from the start, before the app's first command (boot-speed:
   * main's first subscribe waits, as a booting hub's does, so the core's copy shows first).
   */
  constructor(start: 'fixtures' | 'none' | readonly (CoreEv | HubEv)[] = 'fixtures', opts: { hold?: string } = {}) {
    super(path.join(__dirname, 'core-stub.mts'));
    this.holdAtStart = opts.hold;
    if (start === 'none') this.world = 'none';
    else if (start !== 'fixtures') {
      this.world = path.join(this.dir, 'world.jsonl');
      writeFileSync(this.world, start.map((e) => JSON.stringify(e)).join(NL) + NL);
    }
  }

  /**
   * The core stops (exit `code`): the app's main starts it again after its backoff, and the new
   * one sends its start world again (amb-win's app/keep: a core restart loses nothing).
   */
  async exit(code = 1): Promise<void> {
    await this.raw([JSON.stringify({ harness: 'exit', code })]);
  }

  /**
   * The next request for `cmd` (and `card`) is refused with `message`, as the real core answers its
   * hub's refusal to that very request (architect m_23524: one request, one answer). `reason`: its
   * typed `data.reason` (a send the core gave up on: 'late' or 'gone', queue-drawer-fix).
   */
  async refuse(cmd: string, card: number | undefined, message: string, reason?: string): Promise<void> {
    await this.raw([JSON.stringify({ harness: 'refuse', cmd, ...(card === undefined ? {} : { card }), message, ...(reason === undefined ? {} : { reason }) })]);
  }

  /**
   * `cmd` for `card` is answered with `result` (card/answer's AnswerResult: the hub took it; no
   * card: a send's SendResult, its msg_id). `before`: the events its step sent first, as the real
   * hub does (its rows without the card it closed come before the response; none: the hub kept
   * the card open).
   */
  async result(cmd: string, card: number | undefined, result: unknown, before: readonly unknown[] = []): Promise<void> {
    await this.raw([JSON.stringify({ harness: 'result', cmd, ...(card === undefined ? {} : { card }), result, before })]);
  }

  /** The answers to `cmd` wait (a busy hub: thread-loading's subscribe) until `release()`. */
  async hold(cmd: string): Promise<void> {
    await this.raw([JSON.stringify({ harness: 'hold', cmd })]);
  }

  /** The held answers go, in order; `cmd` is answered at once again. */
  async release(): Promise<void> {
    await this.raw([JSON.stringify({ harness: 'release' })]);
  }

  env(): Record<string, string> {
    return {
      BISE_DESKTOP_BISE: this.stub,
      BISE_DESKTOP_WORKSPACE: '',
      BISE_HARNESS_CORE_SOCK: this.sock,
      BISE_HARNESS_CORE_LOG: this.log,
      ...(this.world ? { BISE_HARNESS_CORE_EVENTS: this.world } : {}),
      ...(this.holdAtStart === undefined ? {} : { BISE_HARNESS_CORE_HOLD: this.holdAtStart }),
    };
  }
}
