// The line relay both stubs share (helper-stub.mts, core-stub.mts): a child process of the app that
// speaks JSON lines like the real one and is scripted by a test.
//   start lines  written to stdout at once (the hello, a fixture world)
//   socket       every line a test writes to the unix socket goes to stdout (an event each)
//   stdin        every line the app writes (a command) is appended to the log file, so the test
//                reads what the app asked for: an effect outside the app
//   exit         on a line whose cmd is `quitCmd`, or when stdin closes
//   rpc          (the core stub) JSON-RPC like `ambient-core` (bise_proto::app_rpc): initialize
//                answered, the start lines and every event held until `initialized`, then each as its
//                notification; a request answered {} and logged as the command it carries
//                ({cmd, ...params}), so a test reads the same log either way
import { appendFileSync, rmSync } from 'node:fs';
import net from 'node:net';

const NL = String.fromCharCode(10);

/** A command the app sent (a JSON line, or a request's method and params): its cmd and fields. */
export type Cmd = {
  cmd?: string; card?: unknown;
  // what the core stub reads of a request (thread/subscribe, page, diff, log, completion, files...)
  project?: unknown; agent?: unknown; before?: number | null; limit?: number; commit?: unknown; id?: unknown;
  q?: unknown; rid?: unknown; find?: unknown; slot?: unknown; raw?: unknown;
  [k: string]: unknown;
};
/** A test's {"harness":"refuse"} line: the next request for that command (and card) fails so. */
type Refusal = { cmd?: string; card?: unknown; message?: string; reason?: unknown; code?: unknown };
/** A test's {"harness":"result"} line: a command's answer (and the events before it). */
type Answer = { cmd?: string; card?: unknown; result?: unknown; before?: unknown[] };
/** A JSON-RPC message the app wrote on stdin. */
type RpcIn = { method?: unknown; id?: unknown; params?: Cmd };
/** A line a stub sends: a JSON line already, or an object to stringify. */
export type Line = string | object;

/** Splits a stream of chunks into lines, calling `on` for each non-empty one. */
function lines(on: (line: string) => void) {
  let buf = '';
  return {
    push(chunk: string) {
      buf += chunk;
      let i;
      while ((i = buf.indexOf(NL)) >= 0) {
        const line = buf.slice(0, i).trim();
        buf = buf.slice(i + 1);
        if (line) on(line);
      }
    },
    end() { if (buf.trim()) on(buf.trim()); buf = ''; },
  };
}

/**
 * The JSON-RPC side of the core stub: `hubEvs`, the HubEv tags (hub/<tag>; every other event is
 * core/<tag>). Returns the event writer and the stdin reader.
 */
function rpcSide(write: (line: string) => void, hubEvs: readonly string[]) {
  let ready = false;
  const held: string[] = [];
  // FakeCore.refuse: the next request for that command (and card) gets this error, as the real
  // core answers a hub's refusal to that very request (architect m_23524)
  const refusals: Refusal[] = [];
  const note = (line: string) => {
    let ev: { ev?: unknown; [k: string]: unknown } | null;
    try { ev = JSON.parse(line) as typeof ev; } catch { write(line); return; /* a test's raw line, as is */ }
    if (!ev || typeof ev.ev !== 'string') { write(line); return; }
    const { ev: tag, ...params } = ev;
    if (typeof tag !== 'string') { write(line); return; }
    write(JSON.stringify({ jsonrpc: '2.0', method: `${hubEvs.includes(tag) ? 'hub' : 'core'}/${tag}`, params }));
  };
  return {
    out: (line: string) => { if (ready) note(line); else held.push(line); },
    refuse: (r: Refusal) => refusals.push(r),
    // a stdin message -> the command it carries (null: none), after answering it
    take(m: RpcIn): Cmd | null {
      // BISE_HARNESS_THUMBS_DIR (test-only, the harness's; not in bise_home::env::VARS like every
      // BISE_HARNESS_*): the cache main makes the artifact cards' pictures in (a core's
      // thumbs_dir); unset, none is made and every card keeps its kind's icon
      if (m.method === 'initialize') {
        const thumbs = process.env.BISE_HARNESS_THUMBS_DIR;
        write(JSON.stringify({ jsonrpc: '2.0', id: m.id, result: { proto: 1, methods: [], notifications: [], ...(thumbs ? { thumbs_dir: thumbs } : {}) } }));
        return null;
      }
      if (m.method === 'initialized') { ready = true; for (const l of held.splice(0)) note(l); return null; }
      if (typeof m.method !== 'string' || m.id === undefined) return null;
      const cmd: Cmd = { cmd: m.method.slice(m.method.indexOf('/') + 1), ...(m.params ?? {}) };
      const i = refusals.findIndex((r) => r.cmd === cmd.cmd && (r.card === undefined || r.card === cmd.card));
      const r = i >= 0 ? refusals.splice(i, 1)[0] : undefined;
      if (r) {
        // its code: the hub's own when the test gives one (a document's view: -32032 app off, -32041 gone)
        const code = typeof r.code === 'number' ? r.code : -32001;
        write(JSON.stringify({ jsonrpc: '2.0', id: m.id, error: { code, message: r.message, ...(r.reason === undefined ? {} : { data: { reason: r.reason } }) } }));
      } else {
        // the events the hub's step sent before it answered (its rows without the card it closed):
        // the real hub broadcasts a step's cards before the request's response (FakeCore.result)
        const a = answerOf(cmd);
        for (const ev of a?.before ?? []) note(JSON.stringify(ev));
        write(JSON.stringify({ jsonrpc: '2.0', id: m.id, result: a?.result ?? defaultResult(cmd) }));
      }
      return cmd;
    },
  };
}

/**
 * A request's result when the test set none: a send is answered as today's hub does, a message
 * made with its own id (bise_proto SendResult {msg_id, reason: 'made'}), so his row waits for its
 * echo as in the app; every other request gets {}. A test that wants an older hub's {} or another
 * reason sets it (FakeCore.result). Since send-reason (fb64c8ff1) the window drops a row on a
 * send answered {} (echo.ts answered), which emptied thread/first-run's thread (review-refactor m_30967).
 */
let nextMsgId = 9000;
const defaultResult = (cmd: Cmd) => (cmd.cmd === 'send' ? { msg_id: nextMsgId++, reason: 'made' } : {});

/**
 * A request's answer a test set (FakeCore.result), or undefined: then its result is defaultResult's. A card's
 * answer stays for every request on that card; one with no card (a send's msg_id) answers the
 * next request alone, so each send of a test gets its own id.
 */
let answersBy: Answer[] = [];
const answerOf = (cmd: Cmd): Answer | undefined => {
  const i = answersBy.findIndex((a) => a.cmd === cmd.cmd && a.card === cmd.card);
  const a = answersBy[i];
  if (i < 0 || !a) return undefined;
  if (a.card === undefined) answersBy = answersBy.filter((_, j) => j !== i);
  return a;
};

/**
 * answer(cmd): the lines to write back for a command the app sent (a fake core answering a
 * subscribe). rpc: { hubEvs } to speak JSON-RPC (the core stub). follow(line): the lines to send
 * right after each line that goes out (the core stub: its rows after a hub's cards).
 */
export function runStub({ start = [], sock, log, quitCmd, answer, rpc, follow }: {
  start?: readonly Line[];
  sock?: string | undefined;
  log?: string | undefined;
  quitCmd?: string;
  answer?: (cmd: Cmd) => readonly Line[];
  rpc?: { hubEvs: readonly string[] };
  follow?: (line: string) => readonly string[];
}) {
  const write = (line: string) => process.stdout.write(line + NL);
  const side = rpc ? rpcSide(write, rpc.hubEvs) : null;
  const sent = side ? side.out : write;
  // follow(line): the lines a real core sends right after that one (the core stub's rows on a cards event)
  const out = (line: string) => {
    sent(line);
    for (const l of follow?.(line) ?? []) sent(l);
  };
  for (const l of start) out(typeof l === 'string' ? l : JSON.stringify(l));
  // a test's {"harness":"hold","cmd":c}: the answers to `c` wait (a busy hub, thread-loading)
  // until {"harness":"release"} sends them, in order; BISE_HARNESS_CORE_HOLD arms it from the start
  // (FakeCore's `hold` option: the app's very first ask waits, boot-speed)
  let holding: unknown = process.env.BISE_HARNESS_CORE_HOLD ?? null;
  let held: string[] = [];
  const heldReqs: string[] = [];
  // a test's {"harness":"exit","code":n} line ends this process with that code (a core that
  // stops: the app restarts it, amb-win's app/keep); any other line goes out as is
  const relay = (line: string) => {
    if (line.startsWith('{"harness":"hold"')) {
      try { holding = (JSON.parse(line) as { cmd?: unknown }).cmd ?? null; } catch { /* a broken line: nothing */ }
      return;
    }
    if (line.startsWith('{"harness":"release"')) {
      holding = null;
      const lines = held;
      held = [];
      for (const l of lines) out(l);
      // the held requests, answered now in order (logged already when they came)
      for (const l of heldReqs.splice(0)) handle(l, true);
      return;
    }
    if (line.startsWith('{"harness":"refuse"') || line.startsWith('{"harness":"result"')) {
      try {
        const r = JSON.parse(line) as Refusal & Answer & { harness?: string };
        if (r.harness === 'refuse') side?.refuse(r);
        else answersBy = [...answersBy, r];
      } catch { /* a broken line: nothing */ }
      return;
    }
    if (line.startsWith('{"harness":"exit"')) {
      let code = 1;
      try { code = Number((JSON.parse(line) as { code?: unknown }).code ?? 1); } catch { /* the default */ }
      quit(code);
      return;
    }
    out(line);
  };
  let server: net.Server | undefined;
  if (sock) {
    rmSync(sock, { force: true });
    server = net.createServer((c) => {
      c.setEncoding('utf8');
      const s = lines(relay);
      c.on('data', (d: string) => { s.push(d); });
      c.on('end', () => { s.end(); });
    });
    server.listen(sock);
  }
  function quit(code = 0) {
    server?.close();
    if (sock) rmSync(sock, { force: true });
    process.exit(code);
  }
  // `logged`: a held request answered on release, in the log since it came
  function handle(line: string, logged = false) {
    // a JSON line: a command (a plain stub) or a JSON-RPC message (the core stub)
    let msg: (Cmd & RpcIn) | null;
    try { msg = JSON.parse(line) as typeof msg; } catch { if (log) appendFileSync(log, line + NL); return; /* not JSON: logged as is */ }
    // a JSON-RPC request for the held command waits whole until release (its answer is the one armed
    // then: FakeCore.result/refuse after the app asked); a test sees it asked at once (waitCmd)
    if (side && msg && holding !== null && typeof msg.method === 'string' && msg.id !== undefined && msg.method.slice(msg.method.indexOf('/') + 1) === holding) {
      heldReqs.push(line);
      if (log) appendFileSync(log, JSON.stringify({ cmd: holding, ...(msg.params ?? {}) }) + NL);
      return;
    }
    const cmd = side && msg ? side.take(msg) : msg;
    if (cmd === null) return;
    if (log && !logged) appendFileSync(log, (side ? JSON.stringify(cmd) : line) + NL);
    if (quitCmd && cmd.cmd === quitCmd) quit();
    if (answer) {
      for (const l of answer(cmd)) {
        const line = typeof l === 'string' ? l : JSON.stringify(l);
        if (holding !== null && cmd.cmd === holding) held.push(line);
        else out(line);
      }
    }
  }
  const s = lines(handle);
  process.stdin.setEncoding('utf8');
  process.stdin.on('data', (d: string) => { s.push(d); });
  process.stdin.on('end', () => { s.end(); quit(); });
}
