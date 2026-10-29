// @vitest-environment happy-dom
// The BUILT web host (kit/views/web.js, the file the hub's GET /v/<agent>/<call> page loads): it
// loads in a page with no error (it threw 'process is not defined' when React rode along, sandbox
// m_50455), carries no React, stays under its size budget, and asks the hub for its view at once.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';

const WEB_JS = join(__dirname, '../../../../kit/views/web.js');
/** web.js's budget in bytes: ~17 KB without React (it was 46 KB with it). */
const BUDGET = 24 * 1024;

afterEach(() => {
  vi.unstubAllGlobals();
  document.head.innerHTML = '';
  document.body.innerHTML = '';
});

describe('kit/views/web.js', () => {
  const code = readFileSync(WEB_JS, 'utf8');

  it('carries no React and stays under its budget', () => {
    expect(code).not.toMatch(/process\.env|react\.production|react\.development|__SECRET_INTERNALS/);
    expect(code.length).toBeLessThan(BUDGET);
  });

  it("loads in a page without an error and asks the hub for its view", () => {
    const asked: { url: string; body: unknown }[] = [];
    vi.stubGlobal('fetch', (url: string, init: { body: string }) => {
      asked.push({ url, body: JSON.parse(init.body) as unknown });
      return new Promise(() => undefined);
    });
    document.head.innerHTML = '<meta name="bise-token" content="t0k">';
    document.body.innerHTML = '<main id="bise-view" data-agent="main" data-call="1-1"></main>';
    // the page's own <script>: a global script, so an indirect eval
    expect(() => {
      (0, eval)(code);
    }).not.toThrow();
    expect(asked).toEqual([{ url: '/api/view', body: { method: 'view/resource', params: { agent: 'main', call: '1-1' } } }]);
    expect(document.querySelector('.vw-web-line')?.textContent).not.toBe('');
  });
});
