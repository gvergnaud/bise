// Postgres over Neon's HTTP SQL endpoint, by fetch, with no dependency (bise.dev has no build step
// and no package.json). The wire is what @neondatabase/serverless sends in its HTTP mode: POST
// https://api.<the host without its first label>/sql, the connection string in the
// Neon-Connection-String header, {query, params} as JSON; the answer is {fields, rows}, rows as
// arrays of text (Neon-Raw-Text-Output, Neon-Array-Mode). Every value comes back as text or null:
// the store parses what it reads. The tests run it against a fake of that endpoint on a throwaway
// Postgres; scripts/telemetry_smoke.mjs checks the real one by hand after a deploy.

/** A row: column name to its text, or null. */
export type Row = Record<string, string | null>;
/** One parameterized query ($1, $2... in the text). */
export type Sql = (query: string, params?: (string | number | null)[]) => Promise<Row[]>;

export class SqlError extends Error {}

/** The HTTP endpoint for a connection string's host (the driver's default fetchEndpoint). */
export function endpointOf(connection: string): string {
  const host = new URL(connection).hostname;
  return `https://${host.replace(/^[^.]+\./, "api.")}/sql`;
}

/** A Sql on the database of `connection` (Neon's DATABASE_URL); `endpoint` only for the tests' fake. */
export function neon(connection: string, endpoint: string = endpointOf(connection)): Sql {
  return async (query, params = []) => {
    const res = await fetch(endpoint, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "Neon-Connection-String": connection,
        "Neon-Raw-Text-Output": "true",
        "Neon-Array-Mode": "true",
      },
      body: JSON.stringify({ query, params }),
    });
    if (!res.ok) {
      const text = await res.text();
      let message = text;
      try {
        message = (JSON.parse(text) as { message?: string }).message ?? text;
      } catch { /* the text as it came */ }
      throw new SqlError(`the store said ${res.status}: ${message.slice(0, 300)}`);
    }
    const out = (await res.json()) as { fields: { name: string }[]; rows: (string | null)[][] };
    return out.rows.map((r) => Object.fromEntries(out.fields.map((f, i) => [f.name, r[i] ?? null])));
  };
}
