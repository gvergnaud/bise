// @vitest-environment happy-dom
// at-menu-icons (designer's C): the @ list's rows under the heads Agents, Plugins, Artifacts, Files
// (a group only with rows), each row with its kind's line icon; the heads are no rows, so the keys
// (the editor's index over the rows) never land on one; a pick's chip keeps the row's icon.
import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { agentRows, artifactRows, fileRows, pluginRows, move, type Row } from './pick';
import { headTrim, Picker, pickerGroups } from './Picker';
import { triggerAt } from './tokens';
import { chipDom } from './pm/schema';
import { composer } from '../proto/words.gen';

afterEach(cleanup);

const at = (q: string) => {
  const t = triggerAt(q, true);
  if (t === null) throw new Error('no trigger');
  return t;
};

/** The « @comp » list of the designer's page: an agent, a plugin, artifacts, files. */
function compRows(): Row[] {
  const now = Date.UTC(2026, 10, 5);
  const t = at('@comp');
  const made = [
    { id: 'the-bot-doc', title: 'The Bot · comp doc', kind: 'doc', agent: 'the-bot-doc', url: 'file:///a/the-bot.md', atMs: now - 3_600_000 },
    { id: 'piano', title: 'comp: la carte', kind: 'image', agent: 'piano', url: 'file:///a/carte.png', atMs: now - 7_200_000 },
    { id: 'composer-picker', title: 'composer-picker', kind: 'page', agent: 'ui-review', url: 'http://127.0.0.1/p/x', atMs: now - 9_000_000 },
    { id: 'site-shots', title: 'site-shots comp', kind: 'folder', agent: 'site-shots', url: '', atMs: now - 9_500_000 },
  ];
  return [
    ...agentRows(t, [{ name: 'computer-qa', desc: 'checking computer use' }]),
    ...pluginRows(t, [{ name: 'computer', desc: 'an agent uses your browser and Mac apps' }]),
    ...artifactRows(t, made, now),
    ...fileRows([
      { path: 'site/docs/computer-use.html', dir: false },
      { path: 'plugins/computer', dir: true },
    ]),
  ];
}

describe('the @ list in groups', () => {
  it('puts the rows under Agents, Plugins, Artifacts, Files, in that order', () => {
    const g = pickerGroups(compRows(), 'composer.pick');
    expect(g.map((x) => [x.head, x.rows.length])).toEqual([
      [composer.at_group_agents, 1],
      [composer.at_group_plugins, 1],
      [composer.at_group_artifacts, 4],
      [composer.at_group_files, 2],
    ]);
    expect(g[0]?.head).toBe('Agents');
  });

  it('shows a group only when it has rows; the $ and / lists have no head', () => {
    const rows = compRows().filter((r) => r.tag !== 'plugin' && r.tag !== 'artifact');
    expect(pickerGroups(rows, 'p').map((x) => x.id)).toEqual(['agents', 'files']);
    const cmd: Row = { id: 'cmd./help', label: '/help', desc: '', tag: 'cmd', off: false, pick: { kind: 'run', name: '/help' } };
    expect(pickerGroups([cmd], 'p').map((x) => x.head)).toEqual([undefined]);
  });

  it('gives each kind its line icon', () => {
    const { container } = render(<Picker rows={compRows()} index={0} testId="composer.pick" onPick={() => undefined} />);
    const icon = (id: string) => {
      const g = document.querySelector(`[data-testid="composer.pick.${id}"] .g svg`);
      if (g === null) return null;
      return g.getAttribute('data-icon') ?? `kind:${g.getAttribute('data-kind') ?? ''}`;
    };
    expect(container).toBeDefined();
    expect(icon('agent.computer-qa')).toBe('thread');
    expect(icon('plugin.computer')).toBe('plugins');
    expect(icon('artifact.the-bot-doc')).toBe('kind:doc');
    expect(icon('artifact.piano')).toBe('kind:image');
    expect(icon('artifact.composer-picker')).toBe('kind:page');
    expect(icon('artifact.site-shots')).toBe('artifacts');
    expect(icon('file.site/docs/computer-use.html')).toBe('file');
    expect(icon('file.plugins/computer')).toBe('folder');
  });

  it('draws the heads as no rows: ↓ from the agent lands on the plugin, never a head', () => {
    const rows = compRows();
    const i = move(0, 1, rows.length);
    render(<Picker rows={rows} index={i} testId="composer.pick" onPick={() => undefined} />);
    const heads = [...document.querySelectorAll('.b-picker-ghead')].map((h) => h.textContent);
    expect(heads).toEqual(['Agents', 'Plugins', 'Artifacts', 'Files']);
    expect(document.querySelectorAll('[role="menuitemradio"]')).toHaveLength(rows.length);
    expect(document.querySelector('.b-picker-row.on')?.getAttribute('data-testid')).toBe('composer.pick.plugin.computer');
  });
});

describe("a pick's chip keeps its icon", () => {
  it("an artifact's kind icon, a folder's stack, a plugin's plug, then its words", () => {
    const doc = chipDom({ kind: 'artifact', n: 1, id: 'a', title: 'The Bot', url: 'file:///a/b.md', what: 'doc' });
    expect(doc.querySelector('svg.icon')?.getAttribute('data-kind')).toBe('doc');
    expect(doc.textContent).toBe('The Bot');
    const folder = chipDom({ kind: 'artifact', n: 2, id: 'f', title: 'site-shots', url: '', what: 'folder' });
    expect(folder.querySelectorAll('svg.icon path')).toHaveLength(2);
    const plug = chipDom({ kind: 'plugin', n: 3, name: 'computer' });
    expect(plug.querySelector('svg.icon')).not.toBeNull();
    expect(plug.textContent).toBe('computer');
  });
});

describe('a head never ends the list (designer m_53730)', () => {
  // '@' alone: rows 32 px, the Files head at 300 (28 px), its first row at 328
  const files = { top: 300, rowTop: 328, rowHeight: 32 };
  it('stops above a head whose first row the cut leaves out', () => {
    expect(headTrim(328, [files])).toBe(28);
    expect(headTrim(340, [files])).toBe(40);
    expect(headTrim(310, [files])).toBe(10);
  });
  it('keeps the height when the cut shows half the first row or more, or ends before the head', () => {
    expect(headTrim(344, [files])).toBe(0);
    expect(headTrim(300, [files])).toBe(0);
    expect(headTrim(250, [files])).toBe(0);
    expect(headTrim(340, [])).toBe(0);
  });
});
