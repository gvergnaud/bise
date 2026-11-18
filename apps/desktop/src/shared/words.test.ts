// The words he reads never name an sb command (Benjamin's feedback, Gabriel's « Ouais grave »):
// 'agents set them with sb every' reads like a typo to anyone who does not know `sb`. Every string
// and JSX text of the app's code (comments are not read; tests and the generated proto are not
// the app's words) is judged: a line he reads says what happens in plain words.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import ts from 'typescript';
import { describe, expect, it } from 'vitest';

const SRC = join(__dirname, '..');
const walk = (dir: string): string[] =>
  readdirSync(dir).flatMap((n) => {
    const p = join(dir, n);
    return statSync(p).isDirectory() ? walk(p) : [p];
  });
const rel = (p: string) => relative(SRC, p).split('\\').join('/');
const judged = (r: string) => /\.tsx?$/.test(r) && !/\.(test|spec)\.tsx?$/.test(r) && !r.startsWith('proto/') && !r.endsWith('.d.ts');

/** an sb command as a phrase: 'sb every', 'sb land --here', `sb page publish` */
const SB_CMD = /\bsb (every|land|send|ask|page|artifact|wait|wake|list|tasks|report|status|taste|people|history|show|inspect|spawn|drop|card|flow|resume|reset)\b/;

/** the strings of a source file (literals, templates, JSX text), with their line */
export function wordsOf(code: string, tsx = true): { line: number; text: string }[] {
  const f = ts.createSourceFile(tsx ? 'x.tsx' : 'x.ts', code, ts.ScriptTarget.Latest, true, tsx ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  const out: { line: number; text: string }[] = [];
  const visit = (n: ts.Node) => {
    if (ts.isStringLiteral(n) || ts.isNoSubstitutionTemplateLiteral(n) || ts.isJsxText(n) || ts.isTemplateHead(n) || ts.isTemplateMiddle(n) || ts.isTemplateTail(n)) {
      // an import's path is not words
      if (!(ts.isStringLiteral(n) && (ts.isImportDeclaration(n.parent) || ts.isExportDeclaration(n.parent)))) {
        out.push({ line: f.getLineAndCharacterOfPosition(n.getStart()).line + 1, text: n.text });
      }
    }
    ts.forEachChild(n, visit);
  };
  visit(f);
  return out;
}

describe('the words he reads name no sb command', () => {
  it('reads the strings, not the comments', () => {
    const code = `// sb every in a comment
/** sb land in a doc */
const a = 'agents set them with sb every';
const b = <div>run sb land --here</div>;
const c = \`stop it: \${x} sb wake --stop\`;
const d = 'sb is fine alone, so is sbx land';`;
    const hits = wordsOf(code).filter((w) => SB_CMD.test(w.text)).map((w) => w.line);
    expect(hits).toEqual([3, 4, 5]);
  });

  it('no string of the app names an sb command', () => {
    // a string's text is in its file's text, so only a file whose text has the phrase is parsed
    // (parsing all ~1,500 took 33-42 s at load 60-80, over this test's limit)
    const hits = walk(SRC)
      .filter((p) => judged(rel(p)))
      .map((p) => ({ p, code: readFileSync(p, 'utf8') }))
      .filter(({ code }) => SB_CMD.test(code))
      .flatMap(({ p, code }) => wordsOf(code, p.endsWith('.tsx')).filter((w) => SB_CMD.test(w.text)).map((w) => `${rel(p)}:${String(w.line)}: ${w.text.trim()}`));
    expect(hits, 'say what happens in plain words, not the sb command agents run').toEqual([]);
    // it still reads every file: 6.4 s at load 75, so its own limit stays
  }, 30_000);
});
