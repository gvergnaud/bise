// @vitest-environment happy-dom
// A server's unfolded tools (his m_55586: blender's long names pushed their switches out of the
// card and over the next column). Every switch stays at its cell's end inside the grid; a long
// name gives way (an ellipsis, the whole name in its tip); the grid falls to one column when two
// don't fit. happy-dom has no layout: the real rows are rendered, plugins.css's top-level rules
// are cascaded onto them (the last matching rule wins), and their flex row is laid out here in the
// main column a 1400 and an 800 px window give (the frame's own left: under 1000 px the sidebar
// isn't docked), and at the main column's floor. A name's width is its characters at the mono's
// 0.6 em.
// Mutation: .plg-tool-name without min-width: 0 turns every case red (the longest name pushes its
// switch out of its cell); the grid's 240 px column min at 400 turns the column counts red.
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import type { Plugin } from '@/proto';
import { leftWidthOf } from '@/app/frame-state';
import { MAIN_MIN, SIDE } from '@/app/resize';
import { PluginPage } from './PluginPage';

afterEach(cleanup);

// blender's MCP server (blender.org Lab, v1.0.3): its 26 tools, the longest names of any plugin
const NAMES = [
  'execute_blender_code', 'execute_blender_code_for_cli', 'get_blendfile_summary_datablocks', 'get_blendfile_summary_datablocks_for_cli',
  'get_blendfile_summary_missing_files', 'get_blendfile_summary_missing_files_for_cli', 'get_blendfile_summary_of_linked_libraries',
  'get_blendfile_summary_of_linked_libraries_for_cli', 'get_blendfile_summary_path_info', 'get_blendfile_summary_path_info_for_cli',
  'get_blendfile_summary_usage_guess', 'get_blendfile_summary_usage_guess_for_cli', 'get_object_detail_summary', 'get_objects_summary',
  'get_python_api_docs', 'get_screenshot_of_area_as_image', 'get_screenshot_of_window_as_image', 'get_screenshot_of_window_as_json',
  'jump_to_tab_by_name', 'jump_to_tab_by_space_type', 'jump_to_view3d_object_by_name', 'jump_to_view3d_object_data_by_name',
  'render_thumbnail_to_path', 'render_viewport_to_path', 'search_api_docs', 'search_manual_docs',
];
const SERVER = 'blender#mcp:blender';
const BLENDER: Plugin = {
  id: 'blender', name: 'blender', kind: 'plugin', scope: 'user', title: 'blender', state: 'loaded', on: true,
  parts: [{ id: SERVER, kind: 'mcp', name: 'blender', on: true, tools: NAMES.map((name) => ({ id: `${SERVER}#tool:${name}`, kind: 'tool', name, on: true })) }],
};

type Rule = { sel: string; decls: ReadonlyMap<string, string> };

/** A CSS file's top-level rules (an @media or @container block skipped), each with its declarations. */
function rulesOf(file: string): readonly Rule[] {
  const css = readFileSync(join(__dirname, file), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const out: Rule[] = [];
  for (const m of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const sel = (m[1] ?? '').trim();
    if (sel.startsWith('@') || sel.startsWith(':root') || sel === '') continue;
    const decls = new Map((m[2] ?? '').split(';').map((d) => d.split(':')).filter((d) => d.length >= 2).map((d) => [(d[0] ?? '').trim(), d.slice(1).join(':').trim()] as const));
    out.push({ sel, decls });
  }
  return out;
}
const RULES = rulesOf('plugins.css');
const TOKENS = readFileSync(join(__dirname, '../ds/tokens.css'), 'utf8');

/** An element's declared value of a property: its last matching rule's. */
function decl(el: Element, prop: string): string | undefined {
  return RULES.filter((r) => r.decls.has(prop) && el.matches(r.sel)).at(-1)?.decls.get(prop);
}

/** A length in px: its px numbers and its tokens (var(--x), resolved in tokens.css) summed, so calc(a + b) reads too. */
function px(expr: string): number {
  let sum = 0;
  for (const m of expr.matchAll(/var\((--[\w-]+)\)|(-?\d+(?:\.\d+)?)px/g)) {
    if (m[2] !== undefined) sum += Number(m[2]);
    else {
      const v = new RegExp(`${m[1] ?? ''}:\\s*([^;]+);`).exec(TOKENS)?.[1];
      if (v === undefined) throw new Error(`no token ${m[1] ?? ''}`);
      sum += px(v);
    }
  }
  return sum;
}
/** A shorthand's values: split on its top-level spaces (a calc(a + b) stays whole). */
function words(v: string | undefined): string[] {
  const out = [''];
  let depth = 0;
  for (const ch of (v ?? '').trim()) {
    depth += ch === '(' ? 1 : ch === ')' ? -1 : 0;
    if (/\s/.test(ch) && depth === 0) {
      if (out.at(-1) !== '') out.push('');
    } else out[out.length - 1] = (out.at(-1) ?? '') + ch;
  }
  return out;
}

/** The main column at a window's width, the right panel shut: the frame's left (the rail, the sidebar docked from 1000 px) taken off. */
const mainAt = (win: number): number => win - leftWidthOf(win, { hidden: false, width: SIDE.def });

/** The tools grid's card in a main column: the shell column (--w-col) inside its pads. */
function cardIn(main: number): number {
  return Math.min(px('var(--w-col)'), main - 2 * px('var(--pad-column)'));
}

/** The grid's columns, each tool's cell, where its name ends and its switch's right edge. */
function layout(main: number) {
  const grid = screen.getByTestId(`plugins.part.${SERVER}.tools`);
  const pad = words(decl(grid, 'padding'));
  const width = cardIn(main) - px(pad[1] ?? '') - px(pad[3] ?? '');
  const colGap = px(words(decl(grid, 'gap'))[1] ?? '');
  const trackMin = Math.min(Number(/minmax\(min\((\d+)px, 100%\), 1fr\)/.exec(decl(grid, 'grid-template-columns') ?? '')?.[1]), width);
  const cols = Math.max(1, Math.floor((width + colGap) / (trackMin + colGap)));
  const cell = (width - (cols - 1) * colGap) / cols;
  const sw = px('var(--w-switch)');
  const tools = NAMES.map((name) => {
    const row = screen.getByTestId(`plugins.part.${SERVER}#tool:${name}`);
    const nameEl = row.querySelector('.plg-tool-name');
    if (nameEl === null) throw new Error(name);
    const gap = px(decl(row, 'gap') ?? '');
    const natural = name.length * 0.6 * 12;
    const gives = decl(nameEl, 'min-width') === '0' && Number(words(decl(nameEl, 'flex'))[1] ?? 0) >= 1 && decl(nameEl, 'overflow') === 'hidden';
    const shown = gives ? Math.min(natural, cell - gap - sw) : natural;
    return { name, nameEl, toggleLast: row.lastElementChild?.getAttribute('role') === 'switch', clipped: shown < natural, switchEnd: shown + gap + sw };
  });
  return { cols, cell, tools };
}

describe("a server's unfolded tools", () => {
  it.each([
    // two columns cut blender's longest names (49 characters, 353 px); one column shows them whole
    ['a 1400 px window', mainAt(1400), 2, true],
    ['an 800 px window (the sidebar not docked)', mainAt(800), 2, true],
    ['the main column at its floor (the panel open)', MAIN_MIN, 1, false],
  ])('in %s: %i px of main column, %i column(s), every switch at its cell\'s end, a long name cut (%s) with its tip', (_, main, cols, cut) => {
    render(<PluginPage p={BLENDER} ports={{ send: () => {}, back: () => {}, tryIt: () => {}, setUpComputer: () => {} }} />);
    fireEvent.click(screen.getByTestId(`plugins.part.${SERVER}.fold`));
    const g = layout(main);
    expect(g.cols).toBe(cols);
    for (const t of g.tools) {
      expect(t.toggleLast, t.name).toBe(true);
      expect(t.switchEnd, t.name).toBeLessThanOrEqual(g.cell + 0.5);
      expect(t.nameEl.getAttribute('data-tip')).toBe(`\`${t.name}\``);
      if (t.clipped) {
        expect(decl(t.nameEl, 'text-overflow'), t.name).toBe('ellipsis');
        expect(decl(t.nameEl, 'white-space'), t.name).toBe('nowrap');
      }
    }
    expect(g.tools.some((t) => t.clipped)).toBe(cut);
  });
});
