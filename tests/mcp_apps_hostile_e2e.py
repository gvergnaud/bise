"""MCP Apps, the hostile view through the real hub (docs/mcp-apps.md C6, C9,
risk 1): a throwaway hub, the fake provider, the fixture fake-app as a
user plugin; main calls fake_app.show_hostile; the hub's web host
(`GET /v/main/<call>`, the views host built for the browser) shows it in
headless Chromium, its proxy on `v-<hash>.localhost:<pages port>`, its
calls through `POST /api/view` to the hub's view methods. Every row of
the view (`[data-probe]`) must say `blocked`, and `body[data-done]` must
come: the sandbox (origin, proxy, CSP, frame-ancestors), the hub's rules
(visibility, mistral/*, a browser's ui/message refused) and the host's
origin check, all at once.

Chromium (Playwright's, from apps/desktop's @playwright/test) resolves
`*.localhost` to 127.0.0.1 with --host-resolver-rules, as a browser on a
Mac does by itself.

python3 -u tests/mcp_apps_hostile_e2e.py
"""
import json
import os
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402

FIX = os.path.join(e2e.ROOT, "tests/fixtures/mcp-apps")
DESK = os.path.join(e2e.ROOT, "apps", "desktop")
HOSTILE = "return await tools.fake_app.show_hostile({})"
# the probes hostile.html runs (a row each); fewer means one never ran
MIN_PROBES = 20

# Playwright: open the web host, find the view's blob frame, wait for its
# body[data-done], print its rows as JSON (one line)
BROWSE = r"""
import { createRequire } from 'node:module';
const require = createRequire(process.env.DESK + '/package.json');
const { chromium } = require('@playwright/test');
const b = await chromium.launch({ args: ['--host-resolver-rules=MAP *.localhost 127.0.0.1'] });
const out = { rows: [], frames: [], console: [] };
try {
  const pg = await b.newPage();
  pg.on('console', (m) => out.console.push(m.text().slice(0, 200)));
  pg.on('pageerror', (e) => out.console.push('pageerror: ' + String(e).slice(0, 300)));
  pg.on('requestfailed', (r) => out.console.push('requestfailed: ' + r.url() + ' ' + (r.failure()?.errorText ?? '')));
  pg.on('response', (r) => { if (r.status() >= 400) out.console.push('http ' + r.status() + ': ' + r.request().method() + ' ' + r.url()); });
  await pg.goto(process.env.URL);
  let fr = null;
  for (let i = 0; i < 300 && !fr; i++) {
    fr = pg.frames().find((f) => f.url().startsWith('blob:http://v-')) ?? null;
    if (!fr) await pg.waitForTimeout(100);
  }
  out.frames = pg.frames().map((f) => f.url());
  if (!fr) out.body = (await pg.content()).slice(0, 1500);
  if (fr) {
    await fr.waitForSelector('body[data-done]', { timeout: 60000 }).catch(() => {});
    out.done = await fr.$('body[data-done]').then((e) => e !== null);
    out.mode = await fr.evaluate(() => document.compatMode).catch((e) => String(e));
    out.rows = await fr.$$eval('[data-probe]', (rs) => rs.map((r) => ({ probe: r.dataset.probe, result: r.dataset.result, text: r.textContent })));
  }
} finally {
  await b.close();
}
console.log(JSON.stringify(out));
"""

FAILS = []


def check(cond, what):
    print(("ok   " if cond else "FAIL ") + what, flush=True)
    if not cond:
        FAILS.append(what)


def hostile_call(E, c):
    """main calls show_hostile; its call id (its views file's name)."""
    done = lambda: [l for l in c.lines("main") if "done: " in l]
    before = len(done())
    c.say("[[ts: %s]]" % HOSTILE)
    c.wait(lambda: len(done()) > before, 120, "main's answer after show_hostile")
    c.wait_idle("main")
    views = os.path.join(E.state, "agents", "main", "views")
    names = sorted(os.listdir(views)) if os.path.isdir(views) else []
    check(len(names) == 1, "one views file: %r" % names)
    return names[0][:-5] if names else None


def browse(url):
    env = dict(os.environ, DESK=DESK, URL=url)
    p = subprocess.run(["node", "--input-type=module", "-e", BROWSE], env=env, capture_output=True, text=True, timeout=180)
    last = [l for l in p.stdout.splitlines() if l.startswith("{")]
    if p.returncode != 0 or not last:
        check(False, "the browser ran: rc %d %s" % (p.returncode, p.stderr[-800:]))
        return {}
    return json.loads(last[-1])


def main():
    if not os.path.exists(os.path.join(DESK, "node_modules", "@playwright", "test")):
        sys.exit("apps/desktop has no node_modules: pnpm install there first")
    if not os.path.exists(os.path.join(e2e.ROOT, "kit", "views", "web.js")):
        sys.exit("no kit/views/web.js: the web host isn't built")
    E = e2e.Env()
    import scripted_ts
    E.env.update(scripted_ts.jsrt_env())
    user = os.path.join(E.tmp, "user-plugins")
    os.makedirs(user)
    shutil.copytree(os.path.join(FIX, "fake-app"), os.path.join(user, "fake-app"))
    E.env.update({
        "BEND_PLUGINS_HOME": user,
        "BEND_PLUGINS_DATA": os.path.join(E.tmp, "plugin-data"),
        "BEND_PLUGINS_STATE": os.path.join(E.tmp, "plugins.json"),
        "BEND_MCP_STATUS": os.path.join(E.tmp, "mcp-status"),
        "BEND_MCP_SECRETS": os.path.join(E.tmp, "secrets"),
        "FAKE_APP_LOG": os.path.join(E.tmp, "fake-app.log"),
    })
    try:
        c = E.start_hub()
        c.wait_idle("main")
        call = hostile_call(E, c)
        base = (c.init.get("pages_url") or "").rstrip("/")
        check(base.startswith("http://127.0.0.1:"), "the hub has a pages server: %r" % base)
        if call and base:
            out = browse("%s/v/main/%s" % (base, call))
            rows = out.get("rows", [])
            check(any(f.startswith("http://v-") and f.endswith("/mcp-app") for f in out.get("frames", [])), "the view's proxy on its own origin: %r" % out.get("frames"))
            # the hub's CSP meta goes after the view's doctype: else quirks mode
            check(out.get("mode") == "CSS1Compat", "the view's doctype keeps it in standards mode: %r" % out.get("mode"))
            check(out.get("done") is True, "the hostile view ran every probe (body[data-done])")
            check(len(rows) >= MIN_PROBES, "%d probes reported (at least %d)" % (len(rows), MIN_PROBES))
            leaked = [r["text"] for r in rows if r.get("result") != "blocked"]
            check(not leaked, "every probe blocked: %r" % leaked)
            for r in rows:
                print("     %s" % r.get("text"), flush=True)
            if FAILS:
                print("console: %s" % json.dumps(out.get("console", [])[:20]), flush=True)
                if out.get("body"):
                    print("page: %s" % out["body"], flush=True)
    finally:
        if FAILS:
            os.environ["SB_KEEP"] = "1"
        E.close()
    print("PASS mcp apps hostile" if not FAILS else "FAIL mcp apps hostile: %s" % FAILS, flush=True)
    sys.exit(1 if FAILS else 0)


if __name__ == "__main__":
    main()
