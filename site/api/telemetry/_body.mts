// The body of a telemetry POST, read within bounds: JSON only, plain or gzip (the app sends plain
// today; docs/telemetry.md "The body"), at most 64 KB before and after inflating. It reads the body
// and these headers only: content-type, content-encoding, content-length. Never the address, the
// user agent or any other header (tests/site_telemetry.test.ts reads this folder's sources for it).

/** The biggest body taken: the app's own cap per body (rust/usage, encode + send). */
export const MAX_BODY = 64 * 1024;

export type Read = { ok: true; value: unknown } | { ok: false; status: number; error: string };

const no = (status: number, error: string): Read => ({ ok: false, status, error });

/** The stream's bytes, or null past `max`. */
async function bytesWithin(stream: ReadableStream<Uint8Array>, max: number): Promise<Uint8Array | null> {
  const reader = stream.getReader();
  const parts: Uint8Array[] = [];
  let size = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    size += value.byteLength;
    if (size > max) {
      await reader.cancel();
      return null;
    }
    parts.push(value);
  }
  const out = new Uint8Array(size);
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.byteLength;
  }
  return out;
}

/** The request's JSON body, or the status to answer. */
export async function readJson(request: Request): Promise<Read> {
  const type = (request.headers.get("content-type") ?? "").split(";")[0].trim().toLowerCase();
  if (type !== "application/json") return no(415, "send application/json");
  const encoding = (request.headers.get("content-encoding") ?? "identity").trim().toLowerCase();
  if (encoding !== "identity" && encoding !== "gzip") return no(415, "send plain or gzip");
  const length = Number(request.headers.get("content-length") ?? "0");
  if (length > MAX_BODY) return no(413, "the body is over 64 KB");
  if (!request.body) return no(400, "no body");
  const raw = await bytesWithin(request.body, MAX_BODY);
  if (!raw) return no(413, "the body is over 64 KB");
  let bytes = raw;
  if (encoding === "gzip") {
    try {
      const inflated = new Blob([raw.slice().buffer]).stream().pipeThrough(new DecompressionStream("gzip"));
      const out = await bytesWithin(inflated, MAX_BODY);
      if (!out) return no(413, "the body is over 64 KB inflated");
      bytes = out;
    } catch {
      return no(400, "not gzip");
    }
  }
  try {
    return { ok: true, value: JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)) };
  } catch {
    return no(400, "not JSON");
  }
}
