// generated from dashboard.mts by scripts/ts_emit.ts: never hand-edited (edit the .ts, then run it)
// GET bise.dev/telemetry (vercel.json rewrites it here): the private usage dashboard, see
// _handlers.mts.
import { fromEnv, handleDashboard } from "./_handlers.mjs";
export function GET(request) {
    return handleDashboard(request, fromEnv());
}
