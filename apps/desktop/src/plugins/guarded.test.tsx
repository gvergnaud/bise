// @vitest-environment happy-dom
// His 'Read skills in Documents…' switch (GuardedSwitch.tsx, skills-scan-tcc): only when a skill links
// into a folder macOS guards; its count line; his click sends skills_read_guarded; a refusal says so
// with Open Settings.
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { AppCmd, Plugin, PluginPart } from '@/proto';
import { settings } from '@/proto/words.gen';
import { folderList, GuardedSwitch, guardedOf, readGuardedOn } from './GuardedSwitch';

afterEach(cleanup);

const skill = (name: string, folder?: string, read = false): PluginPart => ({ id: `skills:vibe#skill:${name}`, kind: 'skill', name, on: true, ...(folder === undefined ? {} : { guarded: { folder, read } }) });
const folder = (parts: PluginPart[]): Plugin => ({ id: 'skills:vibe', name: 'skills:vibe', kind: 'skills', scope: 'user', title: 'Vibe skills', state: 'loaded', on: true, parts });

describe('the skills switch', () => {
  it('counts the linked skills, read or not, and lists their folders', () => {
    const rows = [folder([skill('a', 'Documents'), skill('b'), skill('c', 'Desktop', true), skill('d', 'Documents')])];
    expect(guardedOf(rows)).toEqual({ n: 3, folders: ['Documents', 'Desktop'] });
    expect(folderList(['Documents'])).toBe('Documents');
    expect(folderList(['Documents', 'Desktop', 'Downloads'])).toBe(settings.plugins_and({ a: 'Documents, Desktop', b: 'Downloads' }));
    expect(readGuardedOn({ skills: { read_guarded: true } })).toBe(true);
    expect(readGuardedOn(null) || readGuardedOn({ skills: {} })).toBe(false);
  });

  it('shows only when a skill links in; his click sends skills_read_guarded', () => {
    const sent: AppCmd[] = [];
    const { rerender } = render(<GuardedSwitch rows={[folder([skill('b')])]} on={false} refused={[]} send={(c) => sent.push(c)} />);
    expect(screen.queryByTestId('plugins.guarded')).toBeNull();
    rerender(<GuardedSwitch rows={[folder([skill('a', 'Documents')])]} on={false} refused={[]} project="p" send={(c) => sent.push(c)} />);
    expect(screen.getByTestId('plugins.guarded.count').textContent).toBe(settings.plugins_guarded_count({ n: 1, folders: 'Documents' }));
    fireEvent.click(screen.getByTestId('plugins.guarded.toggle'));
    expect(sent).toEqual([{ cmd: 'skills_read_guarded', project: 'p', on: true }]);
  });

  it('macOS said no: the line says so and Open Settings opens Files & Folders', () => {
    let opened = 0;
    render(<GuardedSwitch rows={[folder([skill('a', 'Documents')])]} on={false} refused={['Documents']} send={() => {}} openSettings={() => { opened += 1; }} />);
    expect(screen.getByTestId('plugins.guarded.refused').textContent).toContain(settings.onb_refused_in({ folder: 'Documents' }));
    fireEvent.click(screen.getByTestId('plugins.guarded.settings'));
    expect(opened).toBe(1);
  });
});
