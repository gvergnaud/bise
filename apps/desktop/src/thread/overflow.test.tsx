// @vitest-environment happy-dom
// notice-overflow (his m_52838: '@tool-output-offload can't land: commit signing failed: … "/Users/…/
// PublicKeys/553053d9….pub"' ran past the column, under the right card): a word longer than the
// column (a path, a URL) breaks anywhere in every row's words. The column's rows carry
// overflow-wrap: anywhere, inherited by what they draw; 'anywhere' also lowers a flex item's
// min-content width, so a notice's flex row (mark + words) never grows past the column. A table
// keeps its words whole and scrolls in its own box. happy-dom has no layout and drops overflow-wrap
// from its computed style: the real rows, and thread.css's top-level rules cascaded onto them by
// wrapOf below (the element's own matching rule, else its parent's: the property is inherited).
// Mutation: thread.css's column rule without its overflow-wrap turns the notice and the inline
// code red; without the .th-table rule, the table's cell.
import { cleanup, render } from '@testing-library/react';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import type { Entry } from '@/proto';
import type { RowCtx } from './rows';
import { NoticeRow } from './rows-news';

afterEach(cleanup);

/** thread.css's top-level rules (an @media or @container block skipped), each selector with its overflow-wrap */
function wrapRules(): readonly { sel: string; wrap: string }[] {
  const css = readFileSync(join(__dirname, 'thread.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const out: { sel: string; wrap: string }[] = [];
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
        const wrap = /(?:^|;)\s*overflow-wrap\s*:\s*([^;]+)/.exec(body)?.[1]?.trim();
        if (!skip && wrap !== undefined) out.push({ sel: head.trim(), wrap });
        head = '';
      }
    } else if (depth === 0) head += ch;
    else if (depth === 1) body += ch;
  }
  return out;
}

/** The overflow-wrap an element gets: its own last matching rule, else its parent's (inherited); 'normal' at the root. */
function wrapOf(el: Element | null, rules = wrapRules()): string {
  if (el === null) return 'normal';
  const own = rules.filter((r) => {
    try {
      return el.matches(r.sel);
    } catch {
      return false;
    }
  });
  return own.at(-1)?.wrap ?? wrapOf(el.parentElement, rules);
}

const CTX: RowCtx = { tkey: 'acme/main', project: 'acme-1a2b3c4d', agent: 'main', bise: false, onLink: () => undefined, onArtifact: () => undefined };

/** a 200-char path with no space, as the signing error's public key */
const PATH = `/Users/someone/Library/Containers/com.example.SecretAgent/Data/PublicKeys/${'553053d9d8e6075500c2cb0bb0'.repeat(5)}.pub`.slice(0, 200);

const notice = (text: string): Entry => ({ pos: 7, at_ms: 1, kind: 'notice', text, notice: { level: 'err', text } });

/** a row of the thread's column, as ThreadScreen and EntryRow draw it */
function inColumn(child: React.ReactNode) {
  return render(
    <section className="th-screen">
      <div className="th-row">{child}</div>
    </section>,
  );
}

describe("a long word in the thread's column (notice-overflow)", () => {
  it("an error notice's 200-char path breaks anywhere, its words' box free to shrink", () => {
    expect(PATH).toHaveLength(200);
    const text = `@tool-output-offload can't land: commit signing failed: No private key found for public key "${PATH}"`;
    const v = inColumn(<NoticeRow e={notice(text)} ctx={CTX} />);
    const words = v.getByText(text);
    expect(words.closest('.th-notice')).not.toBeNull();
    expect(wrapOf(words)).toBe('anywhere');
  });

  it('inline code and plain words in any row break anywhere too', () => {
    const v = inColumn(
      <div className="b-text">
        <p>
          see <code data-testid="code">{PATH}</code>
        </p>
      </div>,
    );
    expect(wrapOf(v.getByTestId('code'))).toBe('anywhere');
  });

  it("a table keeps its words whole: it scrolls in its own box", () => {
    const v = inColumn(
      <div className="th-table">
        <table>
          <tbody>
            <tr>
              <td data-testid="cell">{PATH}</td>
            </tr>
          </tbody>
        </table>
      </div>,
    );
    expect(wrapOf(v.getByTestId('cell'))).toBe('normal');
  });
});
