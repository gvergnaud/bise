// generated from packets.mts by scripts/ts_emit.ts: never hand-edited (edit the .ts, then run it)
// POST telemetry.bise.dev/v1/packets (vercel.json rewrites it here): see _handlers.mts.
import { fromEnv, handlePackets } from "./_handlers.mjs";
export function POST(request) {
    return handlePackets(request, fromEnv());
}
