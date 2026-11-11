// No main-process code reaches his real home around the harness's isolation (main m_59528, architect
// m_59538). The harness points Electron's folders under the test home (tests/harness/paths/hook.ts),
// and node's os.homedir() follows the test HOME; what neither covers is os.userInfo().homedir (the
// user database: HOME can't fake it) and a '/Users/...' literal. Each use is listed here with why it
// is safe; a new one is red until it is justified the same way.
import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';

const MAIN = __dirname;

/** file (relative to src/main) -> how many real-home reads it may hold, and why each is safe */
const ALLOWED: Record<string, { n: number; why: string }> = {
  'index.ts': {
    n: 4,
    why: "testHomeProblem's check of a test HOME against it, standInPaths only outside test mode, the terminal's home only outside test mode, the try swap's realHome (refuses his paths in test mode)",
  },
};

const REAL_HOME = /\buserInfo\(\)|['"`]\/Users\//;

const sources = (dir: string): string[] =>
  readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const p = join(dir, e.name);
    if (e.isDirectory()) return sources(p);
    return e.name.endsWith('.ts') && !e.name.endsWith('.test.ts') ? [p] : [];
  });

/** the code lines of a file that read his real home (comment lines skipped) */
export const realHomeReads = (text: string): string[] =>
  text.split('\n').filter((l) => !/^\s*(\/\/|\*|\/\*)/.test(l) && REAL_HOME.test(l));

describe('main never reads his real home but where it says why', () => {
  it('a new os.userInfo() or /Users/ literal is red, the comments are not', () => {
    expect(realHomeReads("const h = userInfo().homedir;\n// os.userInfo().homedir in a comment\n * '/Users/x'")).toEqual(['const h = userInfo().homedir;']);
    expect(realHomeReads("const p = '/Users/gab/w';")).toHaveLength(1);
    expect(realHomeReads('const h = homedir();')).toEqual([]);
  });

  it('src/main: each read is in ALLOWED, with its count', () => {
    const found: Record<string, string[]> = {};
    for (const f of sources(MAIN)) {
      const reads = realHomeReads(readFileSync(f, 'utf8'));
      if (reads.length) found[relative(MAIN, f)] = reads;
    }
    const over = Object.entries(found)
      .filter(([f, reads]) => reads.length !== (ALLOWED[f]?.n ?? 0))
      .map(([f, reads]) => `${f}: ${reads.length} (allowed ${ALLOWED[f]?.n ?? 0}): ${reads.map((r) => r.trim()).join(' | ')}`);
    expect(over, 'a real-home read in src/main: go through app.getPath / os.homedir(), or justify it in ALLOWED').toEqual([]);
    for (const f of Object.keys(ALLOWED)) expect(found[f], `${f} in ALLOWED reads nothing now: remove it`).toBeDefined();
  });
});
