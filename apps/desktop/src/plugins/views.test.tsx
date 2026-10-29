// @vitest-environment happy-dom
// A plugin's views (C8): the VIEWS group's ticks send the view's whole grant; a view never asked
// says 'not granted'; the add screen's ticks become one grant per view after the confirm.
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { AppCmd, PluginPart } from '@/proto';
import { grantsOf, partsOfList, ticked, ViewsGroup, ViewsStep } from './ViewsGroup';
import type { PluginViewsCmd, ViewAnswer } from '@/views/port';
import type { Json } from '@/views/protocol';

const part = (uri: string, declared: string[], granted: string[], asked: boolean): PluginPart => ({
  id: `piano#view:${encodeURIComponent(uri)}`,
  kind: 'view',
  name: uri.split('/').at(-1) ?? uri,
  on: true,
  view: { uri, declared, granted, asked, domains: ['https://api.open-meteo.com'] },
});

afterEach(() => {
  cleanup();
});

describe('ticked', () => {
  it('one tick on or off, the rest kept, in the declared order', () => {
    expect(ticked(['microphone', 'midi'], [], 'midi', true)).toEqual(['midi']);
    expect(ticked(['microphone', 'midi'], ['midi'], 'microphone', true)).toEqual(['microphone', 'midi']);
    expect(ticked(['microphone', 'midi'], ['microphone', 'midi'], 'midi', false)).toEqual(['microphone']);
  });
});

describe('the VIEWS group', () => {
  it("a tick sends the view's whole grant; a view never asked says not granted", () => {
    const sent: AppCmd[] = [];
    const v = render(<ViewsGroup plugin="piano" parts={[part('ui://piano/score', ['microphone', 'midi'], ['midi'], true), part('ui://piano/late', ['camera'], [], false), { id: 'piano#skill:x', kind: 'skill', name: 'x', on: true }]} project="p" send={(c) => sent.push(c)} />);
    expect(v.getByTestId('plugins.views').querySelectorAll('.plg-row')).toHaveLength(2);
    expect(v.queryByTestId('plugins.view.piano#view:ui%3A%2F%2Fpiano%2Fscore.not_granted')).toBeNull();
    expect(v.getByTestId('plugins.view.piano#view:ui%3A%2F%2Fpiano%2Flate.not_granted')).toBeTruthy();
    fireEvent.click(v.getByTestId('plugins.view.piano#view:ui%3A%2F%2Fpiano%2Fscore.perm.microphone'));
    expect(sent).toEqual([{ cmd: 'plugin_grant', project: 'p', plugin: 'piano', uri: 'ui://piano/score', perms: ['microphone', 'midi'] }]);
  });

  it('no views: no group', () => {
    const v = render(<ViewsGroup plugin="docs" parts={[]} send={() => undefined} />);
    expect(v.queryByTestId('plugins.views')).toBeNull();
  });
});

describe('the add screen', () => {
  it('one grant per previewed view after the confirm, unticked ones granted nothing', () => {
    const parts = [part('ui://piano/score', ['midi'], [], false), part('ui://piano/help', [], [], false)];
    expect(grantsOf('piano', parts, { 'ui://piano/score': ['midi'] })).toEqual([
      { cmd: 'plugin_grant', plugin: 'piano', uri: 'ui://piano/score', perms: ['midi'] },
      { cmd: 'plugin_grant', plugin: 'piano', uri: 'ui://piano/help', perms: [] },
    ]);
  });
});

const LIST: Json = {
  views: [
    { server: 'piano', tool: 'compose', uri: 'ui://piano/score', csp: { connectDomains: ['https://api.example.org'] }, permissions: { microphone: {} }, bise_permissions: { midi: {} }, prefersBorder: true, ready: true },
    { server: '~views', tool: 'palette', uri: 'ui://piano/views/palette', ready: true },
    { server: 'piano', uri: 'ui://piano/score', ready: true },
    { server: 'piano', tool: 'x' },
  ],
};

describe("the add's step 2: the server's views once it runs", () => {
  it('reads the hub list: each view once, its permissions (midi too) and domains, step 1 views skipped', () => {
    const ps = partsOfList('piano', LIST, ['ui://piano/views/palette']);
    expect(ps).toHaveLength(1);
    expect(ps[0]?.name).toBe('compose');
    expect(ps[0]?.view).toEqual({ uri: 'ui://piano/score', server: 'piano', declared: ['microphone', 'midi'], domains: ['https://api.example.org'], granted: [], asked: true });
    expect(partsOfList('piano', { nope: 1 })).toEqual([]);
  });

  it('starting, then its views; done grants what he ticked', async () => {
    const asked: PluginViewsCmd[] = [];
    let answer: (a: ViewAnswer) => void = () => undefined;
    const sent: AppCmd[] = [];
    let closed = 0;
    const v = render(<ViewsStep plugin="piano" project="home" skip={[]} ask={(c) => { asked.push(c); return new Promise((r) => { answer = r; }); }} send={(c) => sent.push(c)} onClose={() => { closed += 1; }} />);
    expect(v.getByTestId('plugins.views_step.starting')).toBeTruthy();
    expect(asked).toEqual([{ cmd: 'plugin_views', project: 'home', plugin: 'piano' }]);
    await act(async () => { answer({ result: LIST }); await Promise.resolve(); });
    fireEvent.click(v.getByTestId('plugins.preview.view.piano#view:ui%3A%2F%2Fpiano%2Fscore.perm.midi'));
    fireEvent.click(v.getByTestId('plugins.views_step.done'));
    expect(sent).toEqual([{ cmd: 'plugin_grant', project: 'home', plugin: 'piano', uri: 'ui://piano/score', perms: ['midi'] }, { cmd: 'plugin_grant', project: 'home', plugin: 'piano', uri: 'ui://piano/views/palette', perms: [] }]);
    expect(closed).toBe(1);
  });

  it('nothing found, or the hub can not tell: it closes at once, nothing granted', async () => {
    const sent: AppCmd[] = [];
    let closed = 0;
    render(<ViewsStep plugin="docs" project="home" skip={[]} ask={() => Promise.resolve({ error: { code: -32603, message: 'gone' } })} send={(c) => sent.push(c)} onClose={() => { closed += 1; }} />);
    await act(async () => { await Promise.resolve(); });
    expect(closed).toBe(1);
    expect(sent).toEqual([]);
  });
});
