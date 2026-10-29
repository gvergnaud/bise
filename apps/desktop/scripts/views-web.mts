// The web host's built files (docs/mcp-apps.md C9): src/views/web.ts built by
// views.web.vite.config.ts into a temp folder, then written to ../../kit/views/ (no argument), or
// compared with the committed ones (--check: exit 1 when one differs; `pnpm build` runs it, so a
// change to src/views that the committed web.js misses is red). Owner: mcp-apps-host.
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync, existsSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from 'vite';

const here = join(dirname(fileURLToPath(import.meta.url)), '..');
const kit = join(here, '../../kit/views');
const check = process.argv.includes('--check');
const tmp = mkdtempSync(join(process.env['TMPDIR'] ?? tmpdir(), 'views-web-'));
try {
  await build({ configFile: join(here, 'views.web.vite.config.ts'), logLevel: 'warn', build: { outDir: tmp, emptyOutDir: true } });
  const files = readdirSync(tmp).sort();
  if (check) {
    const stale = files.filter((f) => !existsSync(join(kit, f)) || !readFileSync(join(kit, f)).equals(readFileSync(join(tmp, f))));
    if (stale.length > 0) {
      console.error(`kit/views is stale: ${stale.join(', ')}; run pnpm -C apps/desktop build:views-web`);
      process.exit(1);
    }
  } else {
    mkdirSync(kit, { recursive: true });
    for (const f of files) writeFileSync(join(kit, f), readFileSync(join(tmp, f)));
    console.log(`kit/views: ${files.join(', ')}`);
  }
} finally {
  rmSync(tmp, { recursive: true, force: true });
}
