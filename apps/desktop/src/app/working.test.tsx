// @vitest-environment happy-dom
// The law of 'at work' (his m_44714: main's ASCII animation, never a turning arc): every surface's
// working mark is the breath '· ~ ∿ ≈ ∿ ~' (working/Signs.tsx, drawn as .wk-sign cells), and no
// stylesheet of the window turns a mark. One row per surface; a new surface adds its row here.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { cleanup, render } from '@testing-library/react';
import type { ReactElement } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import type { Agent } from '@/proto';
import { Dot } from '@/ds';
import { ev } from '../../tests/feature/inputs';
import { BREATH } from '@/working/gust';
import { StateMark } from '@/agents/AgentsScreen';
import { WorkingMark } from './Marks';
import { AgentSign } from './Marks';
import { Store, StoreProvider } from '@/store';
import { ComposerTab } from '@/thread/ComposerTab';

afterEach(cleanup);

const AGENT = (ev('agents').agents[0] ?? {}) as Agent;
const working: Agent = { ...AGENT, name: 'perf', main: false, status: 'working', waits: 0, archived: false, waiting_on: null };

/** surface -> its working mark, drawn by the surface's own component */
const SURFACES: readonly (readonly [string, () => ReactElement])[] = [
  ['panel overview: an agent row at work', () => <AgentSign mark="works" testId="t" />],
  ['agents and changes pages: a row at work', () => <StateMark a={working} />],
  ['prs page and changes: a dirty or running dot', () => <Dot state="wk" />],
  ['composer strip (r3-composer): the target at work', () => <StoreProvider store={new Store(null)}><ComposerTab words={{ label: 'to perf' }} at={working} testId="t" /></StoreProvider>],
];

describe('every working mark is the breath, never an arc', () => {
  it.each(SURFACES)('%s', (_name, draw) => {
    const { container } = render(draw());
    const cells = container.querySelectorAll('.wk-sign');
    expect(cells.length).toBe(1);
    expect(BREATH).toContain(cells[0]?.textContent);
    expect(container.querySelector('svg, .app-s-ring')).toBeNull();
  });

  it('the mark says what it is: its tip and VoiceOver name', () => {
    const { getByRole } = render(<WorkingMark tip="perf is working" testId="w" />);
    const m = getByRole('img');
    expect(m.getAttribute('aria-label')).toBe('perf is working');
    expect(m.getAttribute('data-mark')).toBe('working');
  });

  it("no window stylesheet turns a mark (the arc's keyframes are gone)", () => {
    // read from disk (vitest serves a ?raw stylesheet empty); the ds's b-spin is a loading button's, not a mark
    const src = join(__dirname, '..');
    const walk = (dir: string): string[] => readdirSync(dir).flatMap((n) => (statSync(join(dir, n)).isDirectory() ? walk(join(dir, n)) : [join(dir, n)]));
    const sheets = walk(src).filter((p) => p.endsWith('.css') && !p.endsWith(join('ds', 'css', 'motion.css')));
    expect(sheets.map((p) => relative(src, p))).toContain(join('app', 'marks.css'));
    const turning = sheets.filter((p) => /rotate\((1turn|360deg)\)/.test(readFileSync(p, 'utf8'))).map((p) => relative(src, p));
    expect(turning).toEqual([]);
  });
});
