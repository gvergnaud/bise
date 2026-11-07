// @vitest-environment happy-dom
// The outer card's section head with side3d.css, read back with getComputedStyle: happy-dom has no
// layout, so 'all ›' staying on one line is what the stylesheet gives it: no wrap and no shrink (his
// m_46681: 'all' over '›' beside a long title). Mutation:
// side3d.css's .s3-ohead-all without white-space: nowrap or flex: none turns this red.
import { cleanup, render } from '@testing-library/react';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import type { Agent } from '@/proto';
import { side } from '@/proto/words_side.gen';
import { panel } from '@/proto/words_panel.gen';
import type { BarRow, ProjAgent } from '../barrows';
import { jobOf, lineTip, outerRows, OuterHead, worktreeTip } from './OuterCard';

// the head's 3D picture needs WebGL: a picture that never draws
vi.mock('./Bubbles', () => ({ ThingPic: () => <span /> }));

beforeAll(() => {
  const style = document.createElement('style');
  style.textContent = ['../../ds/tokens.css', 'side3d.css'].map((f) => readFileSync(join(__dirname, f), 'utf8')).join(String.fromCharCode(10));
  document.head.append(style);
});
afterEach(cleanup);

describe('the outer card\'s head', () => {
  it('\'all ›\' stays on one line beside a full head: it never wraps nor shrinks', () => {
    const v = render(<OuterHead id="agents" name="Agents" count="1145" loud={side.head_waits({ n: 12 })} onAll={() => undefined} />);
    const all = v.getByTestId('app.outer.agents.all');
    expect(all.textContent).toBe(side.all_short);
    expect(getComputedStyle(all).whiteSpace).toBe('nowrap');
    expect(getComputedStyle(all).flexShrink).toBe('0');
  });
});

describe('an artifact chip in a side row (his m_47004)', () => {
  it('never wider than its line, padding included: its right radius shows, its title takes the ellipsis', () => {
    const v = render(<span className="s3-row-l1"><span data-testid="chip" className="b-artchip s3-chip"><span className="b-artchip-title">appartement 36 Monge</span></span></span>);
    const chip = getComputedStyle(v.getByTestId('chip'));
    expect(chip.boxSizing).toBe('border-box');
    expect(chip.maxWidth).toBe('100%');
  });
});

describe('an agent line\'s tip (native-lead m_47965)', () => {
  it('main at rest says what it is: its name · team lead', () => {
    expect(lineTip({ name: 'bise', main: true }, '')).toBe(`bise · ${panel.role_lead}`);
  });

  it('while it works, its job\'s words, main too', () => {
    expect(lineTip({ name: 'bise', main: true }, 'merging the docs')).toBe('bise · merging the docs');
    expect(lineTip({ name: 'perf' }, 'speed')).toBe('perf · speed');
  });

  it('another agent with no words: its name', () => {
    expect(lineTip({ name: 'perf' }, '')).toBe('perf');
  });
});

describe('an agent line\'s job words', () => {
  const a = (over: Partial<Pick<Agent, 'main' | 'note' | 'title' | 'purpose'>>) => ({ main: false, title: '', purpose: '', ...over });

  it('main at rest never says its purpose: no job, so its tip is the team lead', () => {
    const main = a({ main: true, purpose: 'orchestrator: routes the requests' });
    expect(jobOf(main)).toBe('');
    expect(lineTip({ name: 'bise', main: true }, jobOf(main))).toBe(`bise · ${panel.role_lead}`);
  });

  it('main working: its note; another agent: its title, else its purpose', () => {
    expect(jobOf(a({ main: true, note: 'merging the docs', purpose: 'orchestrator' }))).toBe('merging the docs');
    expect(jobOf(a({ title: 'speed', purpose: 'make it fast' }))).toBe('speed');
    expect(jobOf(a({ purpose: 'make it fast' }))).toBe('make it fast');
  });
});

describe('an agent line\'s ψ (designer m_55493)', () => {
  const row = (agent: string, group?: string): BarRow => ({ key: `p/${agent}`, project: 'p', agent, kind: 'agent', glyph: '', name: agent, sub: '', title: agent, ...(group === undefined ? {} : { group }) }) as BarRow;
  const pa = (a: Partial<Agent> & { name: string }): ProjAgent => ({ project: 'p', projectName: 'p', a: a as Agent });

  it('its tip names the worktree; its branch only when it says something else', () => {
    expect(worktreeTip({ worktree: '/h/.bise/worktrees/hub/perf/harness', branch: 'sb/perf' }, 'harness')).toBe(side.worktree_at({ folder: 'perf' }));
    expect(worktreeTip({ worktree: '/h/wt/perf', branch: 'perf' }, 'harness')).toBe(side.worktree_at({ folder: 'perf' }));
    expect(worktreeTip({ worktree: '/h/wt/perf', branch: 'fix/speed' }, 'harness')).toBe(side.worktree_on({ folder: 'perf', branch: 'fix/speed' }));
    expect(worktreeTip({ worktree: null }, 'harness')).toBe(side.in_worktree);
    expect(worktreeTip(undefined, 'harness')).toBe(side.in_worktree);
  });

  it('a row in a worktree carries its tip, a row in the shared checkout none', () => {
    const rows = outerRows([row('perf'), row('docs')], [pa({ name: 'perf', mode: 'worktree', worktree: '/h/wt/perf/harness', branch: 'sb/perf' }), pa({ name: 'docs' })], () => 'harness');
    expect(rows.map((x) => x.wt)).toEqual([side.worktree_at({ folder: 'perf' }), null]);
  });

  it('it sits right after the name, the end\'s column right-aligned past it; the hover pin never covers it', () => {
    const css = readFileSync(join(__dirname, 'side3d.css'), 'utf8');
    expect(css).not.toMatch(/\.s3-orow-wt\):last-child/u);
    const box = document.createElement('div');
    box.innerHTML = '<span class="s3-orow-name"></span><span class="s3-orow-wt"></span><span class="s3-orow-gap"></span>';
    document.body.append(box);
    const [name, wt, gap] = [...box.children].map((e) => getComputedStyle(e));
    expect(name?.flexGrow).toBe('0');
    expect(wt?.flexShrink).toBe('0');
    expect(gap?.flexGrow).toBe('1');
    box.remove();
  });
});
