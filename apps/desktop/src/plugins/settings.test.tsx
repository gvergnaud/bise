// @vitest-environment happy-dom
// A plugin's settings (dev.bise.settings, designer m_51514): each row says not set, set or '?
// needed to work'; Set… opens the field, ⏎ saves through `plugin_setting` and the row says
// 'applies from the next call'; esc cancels; Forget asks once, then sends no value. The value
// and the variable's name are never drawn.
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AppCmd, Plugin, PluginSetting } from '@/proto';
import { settings } from '@/proto/words.gen';
import { APPLIES_MS, SettingsGroup, stateOf } from './SettingsGroup';
import { badgeOf } from './model';

const key: PluginSetting = { var: 'GOOGLE_MAPS_API_KEY', label: 'Google Maps API key', secret: true, help: 'optional', set: false };
const units: PluginSetting = { var: 'UNITS', label: 'Units', required: true, placeholder: 'metric', set: false };
const id = (s: PluginSetting) => `plugins.setting.maps.${s.var}`;

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe('stateOf', () => {
  it('set, needed for a required one with no value, else not set', () => {
    expect(stateOf({ ...key, set: true })).toBe('set');
    expect(stateOf(units)).toBe('needed');
    expect(stateOf(key)).toBe('not_set');
  });
});

describe('the Settings group', () => {
  it('nothing for a plugin with no setting', () => {
    const v = render(<SettingsGroup plugin="maps" list={[]} send={() => {}} />);
    expect(v.container.innerHTML).toBe('');
  });

  it('each row says its state; the variable name never shows', () => {
    const v = render(<SettingsGroup plugin="maps" list={[key, units, { ...key, var: 'K2', set: true }]} send={() => {}} />);
    expect(v.getByTestId(`${id(key)}.not_set`).textContent).toBe(settings.plugins_setting_not_set);
    expect(v.getByTestId(`${id(units)}.needed`).textContent).toBe(`? ${settings.plugins_setting_needed}`);
    expect(v.getByTestId('plugins.setting.maps.K2.set').textContent).toBe(settings.plugins_setting_set);
    expect(v.getByTestId('plugins.setting.maps.K2.forget')).toBeTruthy();
    expect(v.queryByTestId(`${id(key)}.forget`)).toBeNull();
    expect(v.container.textContent).not.toContain('GOOGLE_MAPS_API_KEY');
  });

  it('Set… opens a password field for a secret; enter saves, then the row says it applies from the next call', () => {
    vi.useFakeTimers();
    const sent: AppCmd[] = [];
    const v = render(<SettingsGroup plugin="maps" project="p" list={[key]} send={(c) => sent.push(c)} />);
    fireEvent.click(v.getByTestId(`${id(key)}.edit`));
    const input = v.getByTestId(`${id(key)}.input`) as HTMLInputElement;
    expect(input.type).toBe('password');
    fireEvent.change(input, { target: { value: 'abc' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(sent).toEqual([{ cmd: 'plugin_setting', project: 'p', name: 'maps', var: 'GOOGLE_MAPS_API_KEY', value: 'abc' }]);
    expect(v.getByTestId(`${id(key)}.applies`).textContent).toBe(settings.plugins_setting_applies);
    expect(v.queryByTestId(`${id(key)}.field`)).toBeNull();
    act(() => { vi.advanceTimersByTime(APPLIES_MS); });
    expect(v.queryByTestId(`${id(key)}.applies`)).toBeNull();
  });

  it('a plain one is a text field with its placeholder; esc cancels and sends nothing; an empty value never saves', () => {
    const sent: AppCmd[] = [];
    const v = render(<SettingsGroup plugin="maps" list={[units]} send={(c) => sent.push(c)} />);
    fireEvent.click(v.getByTestId(`${id(units)}.edit`));
    const input = v.getByTestId(`${id(units)}.input`) as HTMLInputElement;
    expect([input.type, input.placeholder]).toEqual(['text', 'metric']);
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(sent).toEqual([]);
    fireEvent.change(input, { target: { value: 'imperial' } });
    fireEvent.keyDown(input, { key: 'Escape' });
    expect(sent).toEqual([]);
    expect(v.queryByTestId(`${id(units)}.field`)).toBeNull();
  });

  it('Forget asks once, then sends no value; Keep sends nothing', () => {
    const sent: AppCmd[] = [];
    const set = { ...key, set: true };
    const v = render(<SettingsGroup plugin="maps" list={[set]} send={(c) => sent.push(c)} />);
    fireEvent.click(v.getByTestId(`${id(set)}.forget`));
    // the label as the plugin wrote it (ui-review m_51732)
    expect(v.getByTestId(`${id(set)}.ask`).textContent).toContain('forget the Google Maps API key?');
    fireEvent.click(v.getByTestId(`${id(set)}.ask.no`));
    expect(sent).toEqual([]);
    fireEvent.click(v.getByTestId(`${id(set)}.forget`));
    fireEvent.click(v.getByTestId(`${id(set)}.ask.yes`));
    expect(sent).toEqual([{ cmd: 'plugin_setting', name: 'maps', var: 'GOOGLE_MAPS_API_KEY' }]);
  });
});

describe("the plugin's badge", () => {
  it('a required setting with no value waits on him: ?', () => {
    const p = { id: 'maps', name: 'maps', kind: 'plugin', scope: 'built-in', title: 'maps', state: 'loaded', on: true } as Plugin;
    expect(badgeOf({ ...p, settings: [units] })).toBe('?');
    expect(badgeOf({ ...p, settings: [{ ...units, set: true }, key] })).toBeNull();
    expect(badgeOf({ ...p, on: false, settings: [units] })).toBeNull();
  });
});
