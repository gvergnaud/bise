// The live check of bise.dev's telemetry endpoint, by hand after a deploy (never in a gate: it
// writes to the real store). It posts one packet of a fixed test install (yesterday, UTC), checks it
// is taken, posts it again (taken once), then deletes that install and checks the delete answered.
// It leaves nothing behind. Architect m_58375.
//
//   node scripts/telemetry_smoke.ts [base]     base: https://telemetry.bise.dev (default), or a preview's URL
//
// On a preview the rewrites need the telemetry host, so give the function paths' base instead:
//   node scripts/telemetry_smoke.ts https://<preview>.vercel.app/api/telemetry --direct
const args = process.argv.slice(2);
const direct = args.includes("--direct");
const base = (args.find((a) => !a.startsWith("--")) ?? "https://telemetry.bise.dev").replace(/\/$/, "");
const url = (route: "packets" | "delete") => (direct ? `${base}/${route}` : `${base}/v1/${route}`);

const TEST_ID = "00000000-0000-4000-8000-00000000beef";
const yesterday = new Date(Date.now() - 86_400_000).toISOString().slice(0, 10);
const packet = {
  schema: 1, day: yesterday, install_id: TEST_ID, cohort_week: "2026-W01", active_days_7: 0, active_days_28: 0,
  first_today_of_week: false, version: "smoke", channel: "dev", surface: "headless", os_major: 0, arch: "other",
  counts: {}, minutes: {}, hist: {}, milestones: {},
};

async function post(route: "packets" | "delete", body: unknown): Promise<{ status: number; body: string }> {
  const res = await fetch(url(route), { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) });
  return { status: res.status, body: await res.text() };
}

function expect(what: string, got: { status: number; body: string }, status: number, body?: string) {
  const ok = got.status === status && (body === undefined || got.body === body);
  console.log(`${ok ? "ok  " : "FAIL"} ${what}: ${got.status} ${got.body}`);
  if (!ok) process.exitCode = 1;
}

expect("a bad body is refused", await post("packets", { packets: [{ ...packet, brief: "hi" }] }), 400);
expect("the packet is stored", await post("packets", { packets: [packet] }), 200, '{"stored":1}');
expect("a resend counts once", await post("packets", { packets: [packet] }), 200, '{"stored":0}');
expect("Delete my data", await post("delete", { install_id: TEST_ID }), 200, '{"deleted":1}');
