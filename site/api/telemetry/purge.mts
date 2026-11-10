// GET /api/telemetry/purge, called daily by Vercel's cron (vercel.json "crons"): see _handlers.mts.
import { fromEnv, handlePurge } from "./_handlers.mjs";

export function GET(): Promise<Response> {
  return handlePurge(fromEnv());
}
