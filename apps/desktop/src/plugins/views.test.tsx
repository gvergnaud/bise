// @vitest-environment happy-dom
// A plugin's views (C8): the VIEWS group's ticks send the view's whole grant; a view never asked
// says 'not granted'; the add screen's ticks become one grant per view after the confirm.
import { cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { AppCmd, PluginPart } from '@/proto';
import { grantsOf, ticked, ViewsGroup } from './ViewsGroup';

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
