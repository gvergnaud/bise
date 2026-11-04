// A diff's code, colored by its file's language (diff-syntax, his m_53153): the bubble's own
// prism and THEME (thread/Code.tsx codeLines), so a diff's code looks like a code block's in light
// and dark. Each hunk is tokenized as two texts, its old side (context and − lines) and its new
// side (context and + lines), so a comment or a string over several lines stays one token; each
// line then takes its side's runs. Vibe's diff (@pierre/diffs) tokenizes the two sides the same
// way, whole files there, hunks here (the diff is all we have). Pure.
import type { DiffLine, DiffRun } from '@/ds';
import { codeLines, known } from '@/thread/Code';

// a file's extension → prism's language (prism-react-renderer's bundled grammars, Code.tsx's bash)
const LANGS: Record<string, string> = {
  ts: 'typescript', mts: 'typescript', cts: 'typescript', tsx: 'tsx',
  js: 'javascript', mjs: 'javascript', cjs: 'javascript', jsx: 'jsx',
  rs: 'rust', py: 'python', bend: 'python', go: 'go', swift: 'swift', kt: 'kotlin', kts: 'kotlin',
  c: 'c', h: 'c', cc: 'cpp', cpp: 'cpp', hpp: 'cpp', m: 'objectivec', mm: 'objectivec',
  css: 'css', json: 'json', jsonl: 'json', yaml: 'yaml', yml: 'yaml', md: 'markdown', sql: 'sql', graphql: 'graphql',
  html: 'markup', xml: 'markup', svg: 'markup', plist: 'markup', sh: 'bash', bash: 'bash', zsh: 'bash',
};

/** The prism language of a file by its path ('src/a.ts' → 'typescript'); undefined: drawn plain. Pure. */
export function langOfPath(path: string): string | undefined {
  const base = path.slice(path.lastIndexOf('/') + 1);
  const dot = base.lastIndexOf('.');
  const lang = dot <= 0 ? undefined : LANGS[base.slice(dot + 1).toLowerCase()];
  return known(lang) ? lang : undefined;
}

/**
 * Each line's colored runs, in the lines' order: a hunk head's null (it stays plain), a − line
 * its old side's, a + or context line its new side's. A language prism doesn't know: all null. Pure.
 */
export function diffRuns(lines: readonly DiffLine[], lang: string | undefined): (readonly DiffRun[] | null)[] {
  const out: (readonly DiffRun[] | null)[] = lines.map(() => null);
  if (!known(lang)) return out;
  let i = 0;
  while (i < lines.length) {
    if (lines[i]?.head === true) {
      i++;
      continue;
    }
    // a hunk: the lines up to the next head
    const oldAt: number[] = [];
    const newAt: number[] = [];
    for (; i < lines.length && lines[i]?.head !== true; i++) {
      const sign = lines[i]?.sign;
      if (sign !== '+') oldAt.push(i);
      if (sign !== '-') newAt.push(i);
    }
    side(out, lines, oldAt.filter((k) => lines[k]?.sign === '-'), oldAt, lang);
    side(out, lines, newAt, newAt, lang);
  }
  return out;
}

/** Tokenizes one side's lines `at` as one text and gives its runs to the lines `to`. */
function side(out: (readonly DiffRun[] | null)[], lines: readonly DiffLine[], to: readonly number[], at: readonly number[], lang: string): void {
  if (to.length === 0) return;
  const runs = codeLines(at.map((k) => lines[k]?.text ?? '').join('\n'), lang);
  const want = new Set(to);
  at.forEach((k, j) => {
    if (want.has(k)) out[k] = (runs[j] ?? []).map((r) => ({ text: r.text, style: r.style }));
  });
}
