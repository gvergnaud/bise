// POST telemetry.bise.dev/v1/delete (vercel.json rewrites it here), the app's "Delete my data": see
// _handlers.mts.
import { fromEnv, handleDelete } from "./_handlers.mjs";

export function POST(request: Request): Promise<Response> {
  return handleDelete(request, fromEnv());
}
