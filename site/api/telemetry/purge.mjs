// generated from purge.mts by scripts/ts_emit.ts: never hand-edited (edit the .ts, then run it)
// GET /api/telemetry/purge, called daily by Vercel's cron (vercel.json "crons"): see _handlers.mts.
import { fromEnv, handlePurge } from "./_handlers.mjs";
export function GET() {
    return handlePurge(fromEnv());
}
