// at-plugins: the `@` list offers the plugins on (the core's `pickable`), after the agents and
// with their own mark; a pick is a chip sent as Part::Plugin, the model reads `@name`.
import { describe, expect, it } from 'vitest';
import type { Plugin } from '@/proto';
import { agentRows, pluginRows, pluginWords } from './pick';
import { triggerAt } from './tokens';
import { buildMessage, chipLabel, type Chip } from './message';
import { segmentsOfParts } from './partsback';
import { pickablePlugins } from './useSources';
import { youView, partsGist } from '../thread/parts';
import { composer, settings } from '../proto/words.gen';

const row = (name: string, pickable: boolean, what?: string): Plugin => ({
  id: name,
  name,
  kind: 'plugin',
  scope: 'built-in',
  title: name,
  state: pickable ? 'loaded' : 'disabled',
  on: pickable,
  ...(pickable ? { pickable } : {}),
  ...(what === undefined ? {} : { what }),
});

describe('the @ list offers plugins', () => {
  const plugins = pickablePlugins([row('computer', true, 'Computer use: drive his browser. More.'), row('dataviz', false, 'Charts.'), row('docs', true)]);

  it('lists only the pickable rows, each with its first sentence', () => {
    expect(plugins).toEqual([
      { name: 'computer', desc: 'Computer use: drive his browser.' },
      { name: 'docs', desc: '' },
    ]);
    expect(pickablePlugins(null)).toEqual([]);
  });

  it('says its tagline for him over the description written for the model', () => {
    const tagged = { ...row('computer', true, 'computer use: you can drive the user’s own browser. More.'), tagline: 'an agent uses your browser and Mac apps' };
    expect(pickablePlugins([tagged]).map(pluginWords)).toEqual([`${composer.at_plugin} · an agent uses your browser and Mac apps`]);
  });

  it('keeps a plugin that waits on him, its state in place of its sentence', () => {
    const waiting = pickablePlugins([{ ...row('linear', true, 'Issues.'), pick_waits: 'login' }, { ...row('maps', true, 'Maps.'), pick_waits: 'failed' }]);
    expect(waiting.map(pluginWords)).toEqual([`${composer.at_plugin} · ? ${settings.plugins_part_needs_login}`, `${composer.at_plugin} · ✗ ${composer.at_plugin_failed}`]);
    const t = triggerAt('@', true);
    if (t === null) throw new Error('no trigger');
    expect(pluginRows(t, waiting).map((r) => [r.waits, r.pick])).toEqual([
      ['login', { kind: 'plugin', name: 'linear' }],
      ['failed', { kind: 'plugin', name: 'maps' }],
    ]);
  });

  it('shows @computer as a plugin row, apart from the agents', () => {
    const t = triggerAt('@comp', true);
    if (t === null) throw new Error('no trigger');
    const rows = [...agentRows(t, [{ name: 'computer-qa' }]), ...pluginRows(t, plugins)];
    expect(rows.map((r) => [r.tag, r.label])).toEqual([
      ['agent', '@computer-qa'],
      ['plugin', '@computer'],
    ]);
    expect(rows[1]?.desc).toBe(`${composer.at_plugin} · Computer use: drive his browser.`);
    expect(rows[1]?.pick).toEqual({ kind: 'plugin', name: 'computer' });
    // never in a path query
    const path = triggerAt('@src/', true);
    expect(path === null ? [] : pluginRows(path, plugins)).toEqual([]);
  });

  it('sends the chip as Part::Plugin, the model reading @name', () => {
    const chip: Chip = { kind: 'plugin', n: 1, name: 'computer' };
    expect(chipLabel(chip)).toBe('⊞ computer');
    const out = buildMessage([{ kind: 'chip', chip }, { kind: 'text', text: ' open my last Gmail tab' }]);
    expect(out.parts).toEqual([
      { kind: 'plugin', name: 'computer' },
      { kind: 'text', text: ' open my last Gmail tab' },
    ]);
    expect(out.text).toBe('@computer open my last Gmail tab');
    // back into the composer as its chip, drawn as @name in his bubble
    expect(segmentsOfParts(out.parts ?? [])[0]).toEqual({ kind: 'chip', chip: { kind: 'plugin', n: 1, name: 'computer' } });
    expect(youView(out.parts ?? []).words).toBe('@computer open my last Gmail tab');
    expect(partsGist(out.parts ?? [])).toBe('@computer open my last Gmail tab');
  });
});
