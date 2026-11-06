import { describe, expect, it } from 'vitest';
import { configOfPrefs, DEFAULT_CONFIG } from './config';

describe("the helper's config from his prefs", () => {
  it('his excluded apps go to the helper', () => {
    const c = configOfPrefs({ excluded_apps: ['com.apple.MobileSMS', 'com.tinyspeck.slackmacgap'], theme: 'dark' });
    expect(c).toEqual({ cmd: 'config', exclude: ['com.apple.MobileSMS', 'com.tinyspeck.slackmacgap'], right_option: true, quote: true });
  });

  it('his context.quote off: the helper never reads his selection; unset or junk: on', () => {
    expect(configOfPrefs({ context: { quote: false } }).quote).toBe(false);
    expect(configOfPrefs({ context: { quote: true } }).quote).toBe(true);
    expect(configOfPrefs({ context: { shot: false } }).quote).toBe(true);
    expect(configOfPrefs({ context: 'x' }).quote).toBe(true);
  });

  it('no prefs yet, or junk: the defaults (the helper keeps its built-in floor)', () => {
    expect(configOfPrefs(undefined)).toEqual(DEFAULT_CONFIG);
    expect(configOfPrefs([1])).toEqual(DEFAULT_CONFIG);
    expect(configOfPrefs({ excluded_apps: [1, 'a.b', null] }).exclude).toEqual(['a.b']);
  });

  it('right option when the prefs carry it; an old quiet.meetings pref is ignored (no calendar)', () => {
    const c = configOfPrefs({ right_option: false, quiet: { meetings: true } });
    expect(c).toEqual({ cmd: 'config', exclude: [], right_option: false, quote: true });
  });
});
