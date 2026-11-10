// GET bise.dev/telemetry (vercel.json rewrites it here): the private usage dashboard, see
// _handlers.mts.
import { fromEnv, handleDashboard } from "./_handlers.mjs";

export function GET(request: Request): Promise<Response> {
  return handleDashboard(request, fromEnv());
}
