// generated from delete.mts by scripts/ts_emit.ts: never hand-edited (edit the .ts, then run it)
// POST telemetry.bise.dev/v1/delete (vercel.json rewrites it here), the app's "Delete my data": see
// _handlers.mts.
import { fromEnv, handleDelete } from "./_handlers.mjs";
export function POST(request) {
    return handleDelete(request, fromEnv());
}
