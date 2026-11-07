// thread/steady-bottom (his feedback main m_21487): a wheel over a long thread moves only the
// thread, never the window. The window's page never scrolls: html and body around the shell hide
// their overflow, and the shell clips (not hidden: a hidden box still scrolls from code, his
// m_45466), so only inner panes (the thread's own scroller) move. happy-dom lays nothing out: the
// check reads app.css's top-level rules, the ones the window gets.
// Mutation: .app-shell's 'overflow: clip' as 'visible' turns it red.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

/** app.css's top-level rules (an @ block skipped): each selector list with its declarations. */
function rules(): readonly { sel: string; body: string }[] {
  const css = readFileSync(join(__dirname, 'app.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const out: { sel: string; body: string }[] = [];
  let depth = 0;
  let head = '';
  let body = '';
  let skip = false;
  for (const ch of css) {
    if (ch === '{') {
      if (depth === 0) skip = head.trim().startsWith('@');
      depth += 1;
      if (depth === 1) body = '';
    } else if (ch === '}') {
      depth -= 1;
      if (depth === 0) {
        if (!skip) out.push({ sel: head.trim().replace(/\s+/g, ' '), body });
        head = '';
      }
    } else if (depth === 0) head += ch;
    else if (depth === 1) body += ch;
  }
  return out;
}

/** The last value `prop` gets in the rules whose selector list holds `sel`. */
function valueOf(sel: string, prop: string): string | undefined {
  const rx = new RegExp(`(?:^|;)\\s*${prop}\\s*:\\s*([^;]+)`);
  return rules()
    .filter((r) => r.sel.split(',').map((s) => s.trim()).includes(sel))
    .map((r) => rx.exec(r.body)?.[1]?.trim())
    .filter((v): v is string => v !== undefined)
    .at(-1);
}

describe('thread/steady-bottom: the window never scrolls, only its panes', () => {
  it('html and body around the shell hide their overflow, with no margin', () => {
    for (const sel of ['html:has(.app-shell)', 'body:has(.app-shell)']) {
      expect(valueOf(sel, 'overflow'), sel).toBe('hidden');
      expect(valueOf(sel, 'margin'), sel).toBe('0');
    }
  });

  it('the shell is the window high and clips, so no scroll from code moves the frame', () => {
    expect(valueOf('.app-shell', 'height')).toBe('100vh');
    expect(valueOf('.app-shell', 'overflow')).toBe('clip');
  });
});
