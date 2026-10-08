// Test harness: a throwaway Chrome profile with the extension loaded, the
// pages of test/pages/ on a local server, and a fake native host that
// hands the extension's port to the test (the test plays bise's broker,
// contract C4). Never the user's Chrome profile: --user-data-dir in $TMPDIR.
import { spawn } from "node:child_process";
import { mkdtempSync, writeFileSync, readFileSync, rmSync, mkdirSync, existsSync } from "node:fs";
import { createServer } from "node:http";
import net from "node:net";
import path from "node:path";

export const here = path.dirname(new URL(import.meta.url).pathname);
export const extDir = path.resolve(here, "..");
export const EXT_ID = "bogffepmbkbmbfejcadaipgphgkocgob";
export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const BROWSERS = {
  chrome: "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  edge: "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
  brave: "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
};
export const installed = (name) => existsSync(BROWSERS[name]);

/** The test pages on 127.0.0.1:<port>. */
export async function servePages() {
  const dir = path.join(here, "pages");
  const server = createServer((req, res) => {
    const u = new URL(req.url, "http://x");
    const file = path.join(dir, path.basename(u.pathname) || "index.html");
    if (!existsSync(file)) { res.writeHead(404); res.end("not found"); return; }
    res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    res.end(readFileSync(file));
  });
  await new Promise((r) => server.listen(0, "127.0.0.1", r));
  const base = `http://127.0.0.1:${server.address().port}/`;
  return { base, close: () => server.close() };
}

/** The fake bise side: accepts the fake host's connection, speaks C4. */
export async function fakeBroker() {
  let sock = null, buf = "", id = 0;
  const pending = new Map();
  const events = [];
  const hellos = [];
  const waiters = [];
  const onMsg = (m) => {
    if (m.hello) hellos.push(m.hello);
    else if (m.event) events.push({ ...m, at: Date.now() });
    else if (m.id !== undefined && pending.has(m.id)) { pending.get(m.id)(m); pending.delete(m.id); }
    for (const w of [...waiters]) if (w.test()) { waiters.splice(waiters.indexOf(w), 1); w.done(); }
  };
  const server = net.createServer((s) => {
    sock = s;
    s.on("data", (d) => {
      buf += d.toString();
      let i;
      while ((i = buf.indexOf("\n")) >= 0) { const line = buf.slice(0, i); buf = buf.slice(i + 1); if (line) onMsg(JSON.parse(line)); }
    });
    s.on("close", () => { if (sock === s) sock = null; });
  });
  await new Promise((r) => server.listen(0, "127.0.0.1", r));
  const send = (m) => { if (!sock) throw new Error("the extension is not connected"); sock.write(JSON.stringify(m) + "\n"); };
  return {
    port: server.address().port,
    hellos,
    events,
    connected: () => !!sock,
    send,
    /** One C4 request; resolves to {ok, result|error} plus ms. */
    request(agent, op, args = {}, timeout = 30_000, extra = {}) {
      const n = ++id;
      const t0 = Date.now();
      return new Promise((res, rej) => {
        const timer = setTimeout(() => { pending.delete(n); rej(new Error(`no answer to ${op} in ${timeout} ms`)); }, timeout);
        pending.set(n, (m) => { clearTimeout(timer); res({ ...m, ms: Date.now() - t0 }); });
        send({ id: n, agent, op, args, ...extra });
      });
    },
    /** Wait until test() is true (re-checked on every message). */
    until(test, timeout = 10_000) {
      if (test()) return Promise.resolve(true);
      return new Promise((res) => {
        const w = { test, done: () => { clearTimeout(timer); res(true); } };
        const timer = setTimeout(() => { waiters.splice(waiters.indexOf(w), 1); res(false); }, timeout);
        waiters.push(w);
      });
    },
    /** Cut the current connection (the host exits, like a broker restart). */
    drop: () => sock?.destroy(),
    close: () => { sock?.destroy(); server.close(); },
  };
}

/**
 * A throwaway headless browser with the extension, its native host = the fake host
 * connected to `brokerPort`. Returns a CDP client on the browser (pipe).
 */
export async function launch({ browser = "chrome", brokerPort, startUrl, windowSize = "1000,700" }) {
  // Headless (new mode: the full browser, tab groups and extensions included):
  // a test never opens a window in front of the user (the product's own rule).
  const tmp = process.env.TMPDIR || "/tmp";
  const profile = mkdtempSync(path.join(tmp, "cu-profile-"));
  // The native host, registered in the throwaway profile only.
  mkdirSync(path.join(profile, "NativeMessagingHosts"), { recursive: true });
  const hostSh = path.join(profile, "host.sh");
  writeFileSync(hostSh, `#!/bin/bash\nexec "${process.execPath}" "${path.join(here, "fake-host.mjs")}" ${brokerPort}\n`, { mode: 0o755 });
  writeFileSync(path.join(profile, "NativeMessagingHosts", "dev.bise.computer_use.json"), JSON.stringify({
    name: "dev.bise.computer_use", description: "bise computer use (test)", path: hostSh, type: "stdio",
    allowed_origins: [`chrome-extension://${EXT_ID}/`],
  }));
  const proc = spawn(BROWSERS[browser], [
    `--user-data-dir=${profile}`,
    "--remote-debugging-pipe",
    "--enable-unsafe-extension-debugging",
    "--no-first-run", "--no-default-browser-check", "--disable-sync", "--disable-features=Translate",
    "--headless=new",
    `--window-size=${windowSize}`,
    startUrl,
  ], { stdio: ["ignore", "ignore", "ignore", "pipe", "pipe"] });
  const toB = proc.stdio[3], fromB = proc.stdio[4];
  let id = 0, buf = "";
  const pending = new Map();
  const listeners = [];
  fromB.on("data", (d) => {
    buf += d.toString();
    let i;
    while ((i = buf.indexOf("\0")) >= 0) {
      const msg = JSON.parse(buf.slice(0, i)); buf = buf.slice(i + 1);
      if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
      else for (const l of listeners) l(msg);
    }
  });
  const send = (method, params = {}, sessionId) => new Promise((res, rej) => {
    const m = { id: ++id, method, params };
    if (sessionId) m.sessionId = sessionId;
    pending.set(m.id, (r) => (r.error ? rej(new Error(method + ": " + JSON.stringify(r.error))) : res(r.result)));
    toB.write(JSON.stringify(m) + "\0");
  });
  const kill = () => {
    try { proc.kill("SIGKILL"); } catch { /* gone */ }
    rmSync(profile, { recursive: true, force: true });
  };
  // Never leave this Chrome behind, also when the test dies.
  process.on("exit", kill);
  for (const sig of ["SIGINT", "SIGTERM", "SIGHUP"]) process.on(sig, () => { kill(); process.exit(130); });
  process.on("uncaughtException", (e) => { console.error(e); kill(); process.exit(1); });
  const quit = async () => {
    try { await Promise.race([send("Browser.close"), sleep(2000)]); } catch { /* gone */ }
    await sleep(500);
    kill();
  };
  return { send, on: (f) => listeners.push(f), proc, profile, quit };
}

/** Load the extension (pipe only) and give an evaluator on its worker. */
export async function loadExtension(b, dir = extDir) {
  const { id } = await b.send("Extensions.loadUnpacked", { path: dir });
  let sw;
  for (let i = 0; i < 50 && !sw; i++) {
    const { targetInfos } = await b.send("Target.getTargets");
    sw = targetInfos.find((t) => t.type === "service_worker" && t.url.includes(id));
    if (!sw) await sleep(100);
  }
  const { sessionId } = await b.send("Target.attachToTarget", { targetId: sw.targetId, flatten: true });
  const ev = async (expr) => {
    const r = await b.send("Runtime.evaluate", { expression: expr, awaitPromise: true, returnByValue: true }, sessionId);
    if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails).slice(0, 400));
    return r.result.value;
  };
  return { id, ev };
}

/** A CDP session on a page target (the "user" acting in a tab). */
export async function pageSession(b, match) {
  const { targetInfos } = await b.send("Target.getTargets");
  const t = targetInfos.find((x) => x.type === "page" && match(x));
  if (!t) throw new Error("no such page");
  const { sessionId } = await b.send("Target.attachToTarget", { targetId: t.targetId, flatten: true });
  return { targetId: t.targetId, call: (m, p) => b.send(m, p, sessionId), detach: () => b.send("Target.detachFromTarget", { sessionId }) };
}
