// POST telemetry.bise.dev/v1/packets (vercel.json rewrites it here): see _handlers.mts.
import { fromEnv, handlePackets } from "./_handlers.mjs";

export function POST(request: Request): Promise<Response> {
  return handlePackets(request, fromEnv());
}
