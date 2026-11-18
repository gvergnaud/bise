// Where a link he clicks goes, and a web tab's title (page newtab-browser, designer m_64680).
import { describe, expect, it } from 'vitest';
import { closesOnVerdict, goneFront, linkGoes, linkMenu, linkMenuHow, linkPlan, siteOfUrl, webKey } from './web';
import { linkHowOf } from '@/shared/linkhow';
import { menus } from '@/proto/words_menus.gen';
import { webTitle } from './look';

describe('a link he clicks', () => {
  it('a web tab; his browser with ⌘, for a site he always opens there, or anything not http(s)', () => {
    expect(linkGoes('https://github.com/x', false, [])).toBe('tab');
    expect(linkGoes('https://github.com/x', true, [])).toBe('browser');
    expect(linkGoes('https://www.slack.com/client', false, ['slack.com'])).toBe('browser');
    expect(linkGoes('mailto:a@b.c', false, [])).toBe('browser');
    expect(siteOfUrl('https://WWW.Notion.so/x')).toBe('notion.so');
    expect(siteOfUrl('file:///x')).toBeNull();
  });
  it("a page of one of his hubs opens in the panel, never a web tab (panel-page-blank, his m_65005)", () => {
    const projects = [
      { project: 'dashboard', pages_url: 'http://127.0.0.1:47520/p' },
      { project: 'harness', pages_url: 'http://127.0.0.1:7841/p' },
    ];
    expect(linkPlan('http://127.0.0.1:47520/p/vibebot-sprint-critique', 'plain', projects)).toEqual({ go: 'artifact', project: 'dashboard', id: 'vibebot-sprint-critique' });
    expect(linkPlan('http://127.0.0.1:7841/p/plan/v/2', 'alt', projects)).toEqual({ go: 'artifact', project: 'harness', id: 'plan', v: 2 });
    // ⌘: his browser, as for any link
    expect(linkPlan('http://127.0.0.1:47520/p/vibebot-sprint-critique', 'cmd', projects)).toEqual({ go: 'browser' });
  });
  it('an outside link goes to his default browser; ⌥ opens it in a web tab of bise (links-os, his m_66366)', () => {
    const projects = [{ project: 'dashboard', pages_url: 'http://127.0.0.1:47520/p' }];
    expect(linkPlan('https://app.notion.com/p/41b6ba59', 'plain', projects)).toEqual({ go: 'browser' });
    expect(linkPlan('https://linear.app/x/issue/VIB2-162', 'cmd', projects)).toEqual({ go: 'browser' });
    expect(linkPlan('https://app.notion.com/p/41b6ba59', 'alt', projects)).toEqual({ go: 'tab' });
    // a port no hub of his serves (a dev server): the web's rule
    expect(linkPlan('http://127.0.0.1:5173/p/x', 'plain', projects)).toEqual({ go: 'browser' });
    expect(linkPlan('http://127.0.0.1:5173/p/x', 'alt', projects)).toEqual({ go: 'tab' });
    // not http(s): never a tab (main's openExternal takes mailto, refuses the rest)
    expect(linkPlan('mailto:a@b.c', 'alt', projects)).toEqual({ go: 'browser' });
  });
  it("a link's right click: his browser by its name, bise with ⌥-click under it, then Copy link", () => {
    const projects = [{ project: 'dashboard', pages_url: 'http://127.0.0.1:47520/p' }];
    const ids = (url: string, name: string | null) => linkMenu(url, name, projects).map((i) => ('id' in i ? `${i.id}:${i.label}${'sublabel' in i && i.sublabel !== undefined ? `/${i.sublabel}` : ''}` : '-'));
    const inBise = `in:${menus.link_in}/${menus.link_in_hint}`;
    const copy = `copy:${menus.link_copy}`;
    const arc = `out:${menus.link_out({ browser: 'Arc' })}`;
    expect(ids('https://app.notion.com/p/x', 'Arc')).toEqual([arc, inBise, '-', copy]);
    expect(ids('https://app.notion.com/p/x', null)).toEqual([`out:${menus.link_out_any}`, inBise, '-', copy]);
    // a bise page opens in bise anyway: the same three; mailto has no bise side
    expect(ids('http://127.0.0.1:47520/p/plan', 'Arc')).toEqual([arc, inBise, '-', copy]);
    expect(ids('mailto:a@b.c', 'Arc')).toEqual([arc, '-', copy]);
    expect(linkMenuHow('out')).toBe('cmd');
    expect(linkMenuHow('in')).toBe('alt');
    expect(linkMenuHow(null)).toBeNull();
  });
  it('a click reads ⌥ first, then ⌘ or ⌃', () => {
    expect(linkHowOf({ altKey: false, metaKey: false, ctrlKey: false })).toBe('plain');
    expect(linkHowOf({ altKey: false, metaKey: true, ctrlKey: false })).toBe('cmd');
    expect(linkHowOf({ altKey: false, metaKey: false, ctrlKey: true })).toBe('cmd');
    expect(linkHowOf({ altKey: true, metaKey: true, ctrlKey: false })).toBe('alt');
  });
  it('his go out or refused closes a tab that never showed a page, never one with a page', () => {
    const page = { url: 'https://github.com/', title: 'GitHub', favicon: null, back: false, forward: false, loading: false, failed: false, zoom: 1, entries: [{ url: 'https://github.com/', title: 'GitHub' }], index: 0 };
    expect(closesOnVerdict('external', null)).toBe(true);
    expect(closesOnVerdict('refused', { ...page, url: '', entries: [] })).toBe(true);
    expect(closesOnVerdict('external', page)).toBe(false);
    expect(closesOnVerdict('shown', null)).toBe(false);
  });
  it("a page that closed itself: its opener's tab in front when it is in this panel, else the panel's pick (browser-oauth)", () => {
    expect(goneFront('acme|web:1', 'acme')).toBe('web:1');
    expect(goneFront('other|web:1', 'acme')).toBeNull();
    expect(goneFront(null, 'acme')).toBeNull();
  });
  it("a tab's key names its project's panel", () => {
    expect(webKey('acme-1', 'web:2')).toBe('acme-1|web:2');
  });
});

describe("a web tab's title", () => {
  it('whole when short, else cut at a word with …', () => {
    expect(webTitle('GitHub')).toBe('GitHub');
    expect(webTitle('The quick brown fox jumps over the lazy dog again and again')).toBe('The quick brown fox jumps over the lazy…');
    expect(webTitle('x'.repeat(50))).toBe(`${'x'.repeat(40)}…`);
  });
});
