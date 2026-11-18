// A code block's text, highlighted (tui-parity B15; the TUI's syntax.rs):
// prism's tokens as React elements, never innerHTML, colored from the ds
// roles (so dark and light follow his theme). An unknown language is drawn
// as plain code; a block with no language at all (a bare fence), gray
// (--md-syn-bare). codeLines is the one rule: his bubble draws it here, his
// composer as decorations (composer/mdcolor.ts), so both show a block alike
// (code-fences). Owner: amb-win.
import { memo, type CSSProperties } from 'react';
import { normalizeTokens, Prism, type PrismTheme } from 'prism-react-renderer';
import { staggerOf } from './fade';
import { ROLES, roleOf, type Hue, type Role } from '../../../../kit/view/code';

// prism-react-renderer bundles no shell grammar: a small one, so a bash call's box has the TUI's
// colours too (code.rs highlight_bash: comments, strings, variables, keywords, flags, operators)
if (!('bash' in Prism.languages)) {
  Prism.languages['bash'] = {
    comment: { pattern: /(^|\s)#.*/, lookbehind: true },
    string: { pattern: /"(?:\\.|[^"\\])*"|'[^']*'/, greedy: true },
    variable: /\$(?:\{[^}]*\}|\w+|[@*#?$!0-9])/,
    keyword: /\b(?:if|then|else|elif|fi|for|while|until|do|done|case|esac|in|function|return|export|local|set|cd|echo)\b/,
    property: /(^|\s)--?[\w-]+/,
    operator: /&&|\|\||[|;&<>]/,
    number: /\b\d+\b/,
  };
}

// A syntax role's color on the ds's tokens (src/ds/md-tokens.css, theme.rs syntax_*)
const HUES: Record<Hue, string> = {
  comment: 'var(--md-syn-comment)', keyword: 'var(--md-syn-keyword)', string: 'var(--md-syn-string)', number: 'var(--md-syn-number)',
  call: 'var(--md-syn-call)', type: 'var(--md-syn-type)', 'text-2': 'var(--c-text-2)', ok: 'var(--c-ok)', err: 'var(--c-err)',
};

/** A role's look as a style (kit/view/code.ts's ROLES, the one table the Code app's view reads too). Pure. */
function lookOf(r: Role): CSSProperties {
  return {
    ...(r.hue === undefined ? {} : { color: HUES[r.hue] }),
    ...(r.bold === true ? { fontWeight: '700' } : r.normal === true ? { fontWeight: 'normal' } : {}),
    ...(r.italic === true ? { fontStyle: 'italic' } : {}),
    ...(r.strike === true ? { textDecorationLine: 'line-through' } : {}),
  };
}

// The TUI's syntax.rs style(): prism's token kinds on bise's syntax roles (kit/view/code.ts ROLES):
// keywords bold, comments italic, punctuation dim (amb-kit, lead m_11457; code-view m_66499).
export const THEME: PrismTheme = {
  plain: { color: 'var(--md-syn-plain)', backgroundColor: 'transparent' },
  // a role's look sets color, weight, style and line only: plain strings, as prism's entry wants
  styles: ROLES.map((r) => ({ types: [...r.types], style: lookOf(r) as PrismTheme['styles'][number]['style'] })),
};

/** A bare fence's words (``` with no language): gray, the text at ~90 % (his ask, code-fences). */
export const BARE: CSSProperties = { color: 'var(--md-syn-bare)' };

/** A token's style: its types' roles, inner over outer, a type word as a type (roleOf). Pure. */
function styleOf(types: readonly string[], content: string): CSSProperties {
  return lookOf(roleOf(types, content));
}

/** A run of a code line: its text, its token types ('plain' for none) and its look. */
export interface CodeRun {
  readonly text: string;
  readonly types: readonly string[];
  readonly style: CSSProperties;
}

/**
 * A block's lines, each as runs with their look: the one highlighting rule of his bubble and his
 * composer. `lang` '' (a bare fence): every line gray; a language prism doesn't know: plain. Pure.
 */
export function codeLines(text: string, lang: string): CodeRun[][] {
  const code = text.replace(/\n$/, '');
  const bare = lang === '';
  if (bare || !known(lang)) {
    const style = bare ? BARE : {};
    return code.split('\n').map((l) => [{ text: l, types: ['plain'], style }]);
  }
  const grammar = Prism.languages[lang];
  if (grammar === undefined) return code.split('\n').map((l) => [{ text: l, types: ['plain'], style: {} }]);
  return normalizeTokens(Prism.tokenize(code, grammar)).map((line) =>
    line.filter((t) => t.content !== '' && !(t.empty === true)).map((t) => ({ text: t.content, types: t.types, style: styleOf(t.types, t.content) })),
  );
}

/** Prism knows this language (its bundled grammars). Pure. */
export function known(lang: string | undefined): lang is string {
  return lang !== undefined && lang in Prism.languages;
}

/**
 * A streamed code block's new lines (text-appearing v2 (5)): from the
 * code's offset `from`, each whole line fades in, staggered as words are;
 * the lines before it are never redrawn. None without `from`. Pure.
 */
export function freshLines(text: string, from: number | undefined): (CSSProperties | null)[] {
  const lines = text.replace(/\n$/, '').split('\n');
  if (from === undefined) return lines.map(() => null);
  let at = 0;
  const starts = lines.map((l) => {
    const s = at;
    at += l.length + 1;
    return s;
  });
  const step = staggerOf(starts.filter((s) => s >= from).length);
  let k = 0;
  return starts.map((s) => (s >= from ? { animationDelay: `${String(Math.round(step * k++))}ms` } : null));
}

/** A streamed block's line: 'th-ln-in' with its delay when it is new. */
function lineIn(fresh: CSSProperties | null | undefined): { className?: string; style?: CSSProperties } {
  return fresh == null ? {} : { className: 'th-ln-in', style: fresh };
}

/** A code block in a language prism doesn't know, while it streams: its lines, new ones fading in. */
export function PlainCode({ text, from }: { text: string; from: number }) {
  const fresh = freshLines(text, from);
  return (
    <code>
      {text
        .replace(/\n$/, '')
        .split('\n')
        .map((l, i) => (
          <span key={i} {...lineIn(fresh[i])}>
            {l}
            {'\n'}
          </span>
        ))}
    </code>
  );
}

export const Code = memo(function Code({ text, lang, from }: { text: string; lang: string; from?: number | undefined }) {
  const fresh = freshLines(text, from);
  const bare = lang === '';
  return (
    <code className={bare ? undefined : `language-${lang}`}>
      {codeLines(text, lang).map((line, i) => (
        // a block's lines have no other key than their place
        <span key={i} {...lineIn(fresh[i])}>
          {line.map((r, k) => (
            <span key={k} style={r.style}>
              {r.text}
            </span>
          ))}
          {'\n'}
        </span>
      ))}
    </code>
  );
});
