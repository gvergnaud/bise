// bise.dev's telemetry endpoint and dashboard (site/api/telemetry, plan usage-metrics-plan steps 7-8),
// on fake packets only. Run: node --experimental-strip-types --test tests/site_telemetry.test.ts
// (gate.sh quick runs it in the kit lane when site/ changes). It reads the emitted .mjs, what Vercel
// runs (ts_emit's --check keeps them equal to the .mts).
//
// - the schema the server takes is a byte copy of the crate's (rust/usage/schema.json), and the
//   crate's real bodies (rust/usage/tests/fixtures) pass it; each kind of bad body is refused;
// - the body is read within bounds (JSON only, plain or gzip, 64 KB);
// - the sources read no address, user agent or other header, and log nothing;
// - the routes end to end on a throwaway Postgres (homebrew's, initdb in a temp dir) behind a fake of
//   Neon's HTTP SQL endpoint: stored once per install and day, deleted on "Delete my data", purged
//   past 13 months, the dashboard behind its password, every number of fewer than 10 installs
//   hidden, no install id in it. No postgres binary: that part is skipped with a line.
//
// BLESS=1 rewrites site/api/telemetry/_schema.mts from the crate's snapshot.
// SHOT=<file.html> also writes the dashboard on a month of fake data (for its screenshot).
import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, writeFileSync, readdirSync, mkdtempSync, rmSync } from "node:fs";
import { spawnSync, execFileSync } from "node:child_process";
import { createServer, type Server } from "node:http";
import { gzipSync } from "node:zlib";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dir = join(root, "site/api/telemetry");
const crate = join(root, "rust/usage");

// ---- the schema copy -----------------------------------------------------------------------------

const BANNER = "// a byte copy of rust/usage/schema.json, the crate's exported schema: never hand-edited\n" +
  "// (BLESS=1 node --experimental-strip-types --test tests/site_telemetry.test.ts rewrites it)\n";
const schemaModule = (json: string) => `${BANNER}export const SCHEMA = ${json.trimEnd()};\n`;

const schemaPath = join(dir, "_schema.mts");
const schemaWant = () => schemaModule(readFileSync(join(crate, "schema.json"), "utf8"));
if (process.env.BLESS) writeFileSync(schemaPath, schemaWant());

test("the server's schema is the crate's, byte for byte", () => {
  const want = schemaWant();
  const path = schemaPath;
  assert.equal(readFileSync(path, "utf8"), want, "site/api/telemetry/_schema.mts is stale: BLESS=1 this test, then node scripts/ts_emit.ts site/tsconfig.json");
});

const V = await import(join(dir, "_validate.mjs"));
const B = await import(join(dir, "_body.mjs"));
const H = await import(join(dir, "_handlers.mjs"));
const R = await import(join(dir, "_render.mjs"));
const S = await import(join(dir, "_sql.mjs"));

// ---- the validator -------------------------------------------------------------------------------

const fixture = JSON.parse(readFileSync(join(crate, "tests/fixtures/packets_body.json"), "utf8"));
const FIXTURE_TODAY = "2026-11-06";

test("the crate's real bodies pass", () => {
  const r = V.checkPackets(fixture, FIXTURE_TODAY);
  assert.ok(r.ok, r.error);
  assert.equal(r.value.length, 2);
  const del = JSON.parse(readFileSync(join(crate, "tests/fixtures/delete_body.json"), "utf8"));
  assert.deepEqual(V.checkDelete(del), { ok: true, value: del.install_id });
});

test("each kind of bad body is refused", () => {
  const cases: [string, (b: any) => void, string?][] = [
    ["a top field", (b) => (b.host = "mac")],
    ["a free field", (b) => (b.packets[0].brief = "ok")],
    ["a field missing", (b) => delete b.packets[0].day],
    ["an unknown count", (b) => (b.packets[0].counts.acme = 1)],
    ["an inherited key", (b) => (b.packets[0].counts = JSON.parse('{"__proto__": 1}'))],
    ["a count past 999", (b) => (b.packets[0].counts.land = 1000)],
    ["a count of 0", (b) => (b.packets[0].counts.land = 0)],
    ["a fraction", (b) => (b.packets[0].counts.land = 1.5)],
    ["a count as text", (b) => (b.packets[0].counts.land = "3")],
    ["minutes off the step", (b) => (b.packets[0].minutes.app_open = 7)],
    ["minutes past the cap", (b) => (b.packets[0].minutes.app_open = 1445)],
    ["a value off its list", (b) => (b.packets[0].channel = "nightly")],
    ["a path in the version", (b) => (b.packets[0].version = "/Users/camille")],
    ["a mail in the version", (b) => (b.packets[0].version = "a@b")],
    ["a long version", (b) => (b.packets[0].version = "1".repeat(25))],
    ["an upper-case uuid", (b) => (b.packets[0].install_id = b.packets[0].install_id.toUpperCase())],
    ["a uuid not v4", (b) => (b.packets[0].install_id = "3c3c3c3c-3c3c-1c3c-bc3c-3c3c3c3c3c3c")],
    ["no calendar day", (b) => (b.packets[0].day = "2026-02-30")],
    ["a week 54", (b) => (b.packets[0].cohort_week = "2026-W54")],
    ["a hist of 5 buckets", (b) => (b.packets[0].hist.peak_agents = [0, 0, 0, 0, 1])],
    ["an unknown milestone", (b) => (b.packets[0].milestones.first_love = "under_1h")],
    ["a milestone's delay off its list", (b) => (b.packets[0].milestones.first_agent = "soon")],
    ["schema 2", (b) => (b.packets[0].schema = 2)],
    ["today, not closed", (b) => (b.packets[0].day = FIXTURE_TODAY)],
    ["a day in the future", (b) => (b.packets[0].day = "2027-01-01")],
    ["a day 41 days old", (b) => (b.packets[0].day = V.addDays(FIXTURE_TODAY, -41))],
    ["31 packets", (b) => (b.packets = Array(31).fill(b.packets[1]))],
    ["no packets", (b) => (b.packets = [])],
  ];
  for (const [what, edit] of cases) {
    const b = structuredClone(fixture);
    edit(b);
    assert.equal(V.checkPackets(b, FIXTURE_TODAY).ok, false, `${what} passed`);
  }
  assert.equal(V.checkPackets(fixture, V.addDays("2026-11-05", 40)).ok, true, "40 days old is taken");
  for (const bad of [{}, { install_id: "x" }, { install_id: fixture.packets[0].install_id, more: 1 }, []]) {
    assert.equal(V.checkDelete(bad).ok, false, JSON.stringify(bad));
  }
});

test("days and months", () => {
  assert.equal(V.addDays("2026-03-01", -1), "2026-02-28");
  assert.equal(V.monthsBefore("2026-03-31", 1), "2026-02-28");
  assert.equal(V.monthsBefore("2027-11-10", 13), "2026-10-10");
  assert.equal(V.isDay("2028-02-29"), true);
  assert.equal(V.isDay("2026-13-01"), false);
});

// ---- reading the body ----------------------------------------------------------------------------

const post = (body: BodyInit | null, headers: Record<string, string> = { "content-type": "application/json" }) =>
  new Request("https://telemetry.bise.dev/v1/packets", { method: "POST", body, headers });

test("the body: JSON, plain or gzip, at most 64 KB", async () => {
  const text = JSON.stringify(fixture);
  assert.deepEqual(await B.readJson(post(text)), { ok: true, value: fixture });
  const gz = await B.readJson(post(gzipSync(text), { "content-type": "application/json", "content-encoding": "gzip" }));
  assert.deepEqual(gz, { ok: true, value: fixture });
  assert.equal((await B.readJson(post(text, { "content-type": "text/plain" }))).status, 415);
  assert.equal((await B.readJson(post(text, { "content-type": "application/json", "content-encoding": "br" }))).status, 415);
  assert.equal((await B.readJson(post("x".repeat(B.MAX_BODY + 1)))).status, 413);
  const bomb = gzipSync(" ".repeat(B.MAX_BODY * 4));
  assert.equal((await B.readJson(post(bomb, { "content-type": "application/json", "content-encoding": "gzip" }))).status, 413);
  assert.equal((await B.readJson(post("not json"))).status, 400);
  assert.equal((await B.readJson(post("{}", { "content-type": "application/json", "content-encoding": "gzip" }))).status, 400);
});

// ---- no address, no log --------------------------------------------------------------------------

test("the sources read no address, no user agent, no other header, and log nothing", () => {
  const allowed = new Set(["content-type", "content-encoding", "content-length", "authorization"]);
  const sources = readdirSync(dir).filter((f) => f.endsWith(".mts"));
  assert.ok(sources.length >= 8);
  for (const f of sources) {
    const src = readFileSync(join(dir, f), "utf8");
    for (const m of src.matchAll(/headers\.get\(\s*"([^"]+)"/g)) assert.ok(allowed.has(m[1]), `${f} reads the header ${m[1]}`);
    assert.doesNotMatch(src, /headers\.(entries|keys|values|forEach)\b|headers\.get\(\s*[^"\s]/, `${f} reads headers wholesale`);
    assert.doesNotMatch(src, /console\.|forwarded|x-real-ip|user-agent|geolocation|ipAddress|request\.ip\b/i, `${f} logs or reads an address`);
  }
});

// ---- the dashboard's masking ---------------------------------------------------------------------

test("a number of 1 to 9 installs shows < 10", () => {
  assert.equal(R.shown({ value: 0, installs: 0 }), "0");
  assert.match(R.shown({ value: 412, installs: 9 }), /&lt; 10/);
  assert.equal(R.shown({ value: 1234, installs: 10 }), "1,234");
});

// ---- end to end on a throwaway Postgres ----------------------------------------------------------

/** A fake of Neon's HTTP SQL endpoint: {query, params} in, {fields, rows} of text out, run by psql. */
function fakeNeon(psql: (sql: string) => { ok: boolean; out: string; err: string }): Server {
  const literal = (v: unknown) => (v === null ? "NULL" : typeof v === "number" ? String(v) : `'${String(v).replace(/'/g, "''")}'`);
  return createServer((req, res) => {
    let raw = "";
    req.on("data", (c) => (raw += c));
    req.on("end", () => {
      assert.ok(req.headers["neon-connection-string"], "the connection string goes in its header");
      const { query, params } = JSON.parse(raw) as { query: string; params: unknown[] };
      const r = psql(query.replace(/\$(\d+)/g, (_, n) => literal(params[Number(n) - 1])));
      if (!r.ok) {
        res.writeHead(400, { "content-type": "application/json" }).end(JSON.stringify({ message: r.err.trim() }));
        return;
      }
      const [head, ...rows] = parseCsv(r.out);
      const fields = (head ?? []).map((name) => ({ name, dataTypeID: 25 }));
      res.writeHead(200, { "content-type": "application/json" }).end(JSON.stringify({ fields, rows: rows.map((row) => row.map((c) => (c === "\\N" ? null : c))) }));
    });
  });
}

function parseCsv(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [], cell = "", quoted = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quoted) {
      if (c === '"' && text[i + 1] === '"') { cell += '"'; i++; }
      else if (c === '"') quoted = false;
      else cell += c;
    } else if (c === '"') quoted = true;
    else if (c === ",") { row.push(cell); cell = ""; }
    else if (c === "\n") { row.push(cell); rows.push(row); row = []; cell = ""; }
    else cell += c;
  }
  if (cell || row.length) { row.push(cell); rows.push(row); }
  return rows;
}

const hasPostgres = spawnSync("initdb", ["--version"]).status === 0;
let pgDir = "", pgPort = 0, neonServer: Server | null = null, sqlOf: () => any = () => null;

before(async () => {
  if (!hasPostgres) return;
  pgDir = mkdtempSync(join(tmpdir(), "bise-telemetry-pg-"));
  execFileSync("initdb", ["-D", join(pgDir, "data"), "-U", "postgres", "-A", "trust", "--no-sync", "-E", "UTF8"], { stdio: "ignore" });
  pgPort = 20000 + (process.pid % 20000);
  execFileSync("pg_ctl", ["-D", join(pgDir, "data"), "-l", join(pgDir, "log"), "-w", "-o", `-p ${pgPort} -c listen_addresses=127.0.0.1 -k '' -c fsync=off`, "start"], { stdio: "ignore" });
  const psql = (sql: string) => {
    const r = spawnSync("psql", ["-X", "-q", "--csv", "-P", "null=\\N", "-v", "ON_ERROR_STOP=1", "-h", "127.0.0.1", "-p", String(pgPort), "-U", "postgres", "-d", "postgres", "-c", sql], { encoding: "utf8" });
    return { ok: r.status === 0, out: r.stdout, err: r.stderr };
  };
  neonServer = fakeNeon(psql);
  await new Promise<void>((ok) => neonServer!.listen(0, "127.0.0.1", ok));
  const port = (neonServer.address() as { port: number }).port;
  // one Sql per test world: ensureTable runs again on a fresh one, as on a cold start
  sqlOf = () => S.neon("postgresql://u:p@ep-fake-123.eu-central-1.aws.neon.tech/db", `http://127.0.0.1:${port}/sql`);
});

after(() => {
  neonServer?.close();
  if (pgDir) {
    spawnSync("pg_ctl", ["-D", join(pgDir, "data"), "-m", "immediate", "stop"], { stdio: "ignore" });
    rmSync(pgDir, { recursive: true, force: true });
  }
});

/** A small deterministic random. */
const rng = (seed: number) => () => ((seed = (seed * 1103515245 + 12345) % 2 ** 31) / 2 ** 31);

const uuid = (n: number) => {
  const h = n.toString(16).padStart(12, "0");
  return `00000000-0000-4000-8000-${h}`;
};

/** A valid packet of install `n` on `day`, from fake numbers. */
function packet(n: number, day: string, version: string, r: () => number, active = true) {
  const pick = (max: number) => Math.floor(r() * max);
  const counts: Record<string, number> = {};
  const put = (k: string, v: number) => v > 0 && (counts[k] = Math.min(999, v));
  if (active) put("user_message", 1 + pick(40));
  put("card_answered", active ? pick(6) : 0);
  put("agent_created_by_user", active ? pick(4) : 0);
  put("agent_created_by_main", active ? pick(6) : 0);
  put("agent_created_by_agent", pick(3));
  put("turn_done", pick(200));
  put("crash_tui", r() < 0.03 ? 1 : 0);
  put("crash_app_renderer", r() < 0.02 ? 1 : 0);
  put("setup_started", r() < 0.05 ? 1 : 0);
  put("setup_done", r() < 0.04 ? 1 : 0);
  const five = (m: number) => Math.round(m / 5) * 5;
  const minutes: Record<string, number> = {};
  const open = five(60 + pick(480));
  const putm = (k: string, v: number) => v > 0 && (minutes[k] = v);
  putm("agent_work", five(pick(900)));
  putm("app_open", open);
  putm("app_focused", five(open * (0.3 + r() * 0.4)));
  putm("app_engaged", five(open * (0.1 + r() * 0.2)));
  putm("tui_focused", r() < 0.3 ? five(pick(120)) : 0);
  return {
    schema: 1, day, install_id: uuid(n), cohort_week: "2026-W40", active_days_7: 1 + pick(7), active_days_28: 1 + pick(28),
    first_today_of_week: r() < 0.2, version, channel: version.includes("beta") ? "beta" : "stable",
    surface: ["desktop", "desktop", "both", "terminal"][n % 4], os_major: 26, arch: "arm64",
    counts, minutes, hist: { peak_agents: [0, 0, 1, 0, 0, 0] }, milestones: {},
  };
}

const TODAY = "2026-11-10";
const world = (over: Partial<{ sql: unknown; today: string; password: string | null }> = {}) =>
  ({ sql: sqlOf(), today: TODAY, password: "s3cret-long-enough", ...over });
const send = (path: string, body: unknown) =>
  new Request(`https://telemetry.bise.dev${path}`, { method: "POST", body: JSON.stringify(body), headers: { "content-type": "application/json", "x-forwarded-for": "203.0.113.7", "user-agent": "camille-mac" } });
const basic = (pw: string) => ({ authorization: `Basic ${btoa(`gabriel:${pw}`)}` });

async function sendAll(w: object, packets: object[]) {
  for (let i = 0; i < packets.length; i += 30) {
    const res = await H.handlePackets(send("/v1/packets", { packets: packets.slice(i, i + 30) }), w);
    assert.equal(res.status, 200, await res.clone().text());
  }
}

test("routes end to end on a throwaway Postgres", { skip: hasPostgres ? false : "no postgres binary (brew install postgresql): the store part is not run" }, async () => {
  const w = world();
  const sql = w.sql as (q: string, p?: unknown[]) => Promise<Record<string, string | null>[]>;
  const r = rng(7);
  // 12 installs on 2026.11.9 active yesterday, 3 on an old version, 1 inactive
  const yesterday = V.addDays(TODAY, -1);
  const packets = [
    ...Array.from({ length: 12 }, (_, i) => packet(i + 1, yesterday, "2026.11.9", r)),
    ...Array.from({ length: 3 }, (_, i) => packet(100 + i, yesterday, "2026.10.2", r)),
    packet(200, yesterday, "2026.11.9", r, false),
  ];
  await sendAll(w, packets);
  // a resend after a timeout counts once
  const again = await H.handlePackets(send("/v1/packets", { packets: packets.slice(0, 2) }), w);
  assert.deepEqual(await again.json(), { stored: 0 });
  const [{ n }] = await sql("select count(*) as n from packets");
  assert.equal(n, "16");
  // nothing of the request but the packet is kept: the columns are the schema's
  const cols = (await sql("select column_name from information_schema.columns where table_name = 'packets'")).map((c) => c.column_name).sort();
  assert.deepEqual(cols, Object.keys(fixture.packets[0]).sort());
  const dump = JSON.stringify(await sql("select * from packets"));
  assert.ok(!dump.includes("203.0.113.7") && !dump.includes("camille"), "the address or the user agent was kept");

  // a bad body is refused whole, a store-less world answers 503 (the app keeps its days)
  const bad = await H.handlePackets(send("/v1/packets", { packets: [packet(300, yesterday, "2026.11.9", r), { ...packet(301, yesterday, "x", r), brief: "hi" }] }), w);
  assert.equal(bad.status, 400);
  assert.equal((await sql("select count(*) as n from packets where install_id = $1::uuid", [uuid(300)]))[0].n, "0");
  assert.equal((await H.handlePackets(send("/v1/packets", fixture), world({ sql: null }))).status, 503);

  // the dashboard: closed without its password, open with it; < 10 hides the 3 old installs
  const anon = await H.handleDashboard(new Request("https://bise.dev/telemetry"), w);
  assert.equal(anon.status, 401);
  assert.match(anon.headers.get("www-authenticate") ?? "", /^Basic /);
  assert.equal((await H.handleDashboard(new Request("https://bise.dev/telemetry", { headers: basic("wrong") }), w)).status, 401);
  assert.equal((await H.handleDashboard(new Request("https://bise.dev/telemetry", { headers: basic("x") }), world({ password: null }))).status, 503);
  const page = await H.handleDashboard(new Request("https://bise.dev/telemetry", { headers: basic("s3cret-long-enough") }), w);
  assert.equal(page.status, 200);
  assert.match(page.headers.get("content-security-policy") ?? "", /default-src 'none'/);
  const html = await page.text();
  assert.ok(!/[0-9a-f]{8}-[0-9a-f]{4}-4/.test(html), "an install id is in the dashboard");
  assert.match(html, /<td><span class="id">2026\.11\.9<\/span><\/td><td>stable<\/td><td class="n">13<\/td>/, "the 13 installs of 2026.11.9 show");
  assert.doesNotMatch(html, /2026\.10\.2/, "a version of 3 installs is folded into others");
  assert.match(html, /<td>1 other<\/td><td><\/td><td class="n"><span class="few"/);
  assert.match(html, /active yesterday<\/div><div class="value">15</, "15 of the 16 installs were active yesterday");

  // "Delete my data": every row of that id, 200 even when none
  const del = await H.handleDelete(send("/v1/delete", { install_id: uuid(1) }), w);
  assert.deepEqual(await del.json(), { deleted: 1 });
  assert.deepEqual(await (await H.handleDelete(send("/v1/delete", { install_id: uuid(1) }), w)).json(), { deleted: 0 });

  // the purge: past 13 months goes, at ingest and by the cron
  await sql(`insert into packets select install_id, '2025-10-09'::date, schema, cohort_week, active_days_7, active_days_28, first_today_of_week, version, channel, surface, os_major, arch, counts, minutes, hist, milestones from packets where install_id = $1::uuid`, [uuid(2)]);
  await sql(`insert into packets select install_id, '2025-10-10'::date, schema, cohort_week, active_days_7, active_days_28, first_today_of_week, version, channel, surface, os_major, arch, counts, minutes, hist, milestones from packets where install_id = $1::uuid and day = $2::date`, [uuid(2), yesterday]);
  assert.deepEqual(await (await H.handlePurge(w)).json(), { purged: 1 }, "2025-10-09 is past 13 months on 2026-11-10, 2025-10-10 is not");
  assert.deepEqual(await (await H.handlePurge(world({ today: "2026-11-11" }))).json(), { purged: 1 });

  if (process.env.SHOT) {
    await sql("truncate packets");
    const rr = rng(42);
    const month: object[] = [];
    for (let n = 1; n <= 160; n++) {
      const joined = Math.floor(rr() * 25);
      const version = n % 9 === 0 ? "2026.11.10-beta.1" : n % 4 === 0 ? "2026.11.5" : n === 150 || n === 151 ? "2026.10.2" : "2026.11.9";
      for (let d = 30; d >= 1; d--) {
        if (30 - d < joined || rr() < 0.25) continue;
        month.push(packet(n, V.addDays(TODAY, -d), version, rr, rr() < 0.8));
      }
    }
    await sendAll(w, month);
    const shot = await H.handleDashboard(new Request("https://bise.dev/telemetry", { headers: basic("s3cret-long-enough") }), w);
    writeFileSync(process.env.SHOT, await shot.text());
  }
});
