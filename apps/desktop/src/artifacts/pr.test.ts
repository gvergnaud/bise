// pr-integration A: a PR artifact's line and the Pull requests filter's two sides (artifacts/pr.ts,
// folders.ts prSideItems), on the hub's status as the wire carries it.
import { describe, expect, it } from 'vitest';
import type { Artifact, Pr } from '@/proto';
import { library } from '@/proto/words_library.gen';
import { chipOf, chipsOf, chipWord, prSideItems } from './folders';
import type { Item } from './model';
import { prLine, prParts, prSettled } from './pr';

const status = (over: Partial<Pr> = {}): Pr => ({
  number: 412, url: 'https://github.com/acme/web/pull/412', branch: 'sb/perf', agents: ['perf'], state: 'open', checks: 'pass',
  review: 'approved', words: 'approved · checks ✓ 12/12 · your merge', text: '', yours: 'merge', ...over,
});
const prArt = (id: string, over: Partial<Artifact> = {}, s: Pr | null = status()): Artifact => ({
  id, kind: 'pr', title: 'Faster signup', agent: 'perf', url: 'https://github.com/acme/web/pull/412', at_ms: 1, version: 1, new: false,
  pr: { repo: 'acme/web', number: 412, tag: '#412', status: s }, ...over,
});
const item = (a: Artifact): Item => ({ project: 'acme', projectName: 'acme', a });

describe('a PR artifact line', () => {
  it('says the sigil, the hub words, pink only when it waits on him', () => {
    expect(prLine(prArt('a'))).toEqual({ sigil: '#412', words: 'approved · checks ✓ 12/12 · your merge', yours: true, late: '' });
    const changes = prLine(prArt('b', {}, status({ words: 'changes asked', yours: null })));
    expect(changes?.yours).toBe(false);
    expect(changes?.words).toBe('changes asked');
  });

  it('says the hub’s tag (a merge request’s own sigil), and an older hub’s row by its number', () => {
    const mr = status({ tag: '!', provider: 'gitlab' });
    expect(prLine(prArt('a', { pr: { repo: 'acme/web', number: 412, tag: '!412', status: mr } }))?.sigil).toBe('!412');
    expect(prLine(prArt('b', { pr: { repo: 'acme/web', number: 7, status: null } }))?.sigil).toBe('#7');
  });

  it('says how late the forge is, and nothing made up before it answers', () => {
    const late = prLine(prArt('a', {}, status({ stale_ms: 12 * 60_000 })));
    expect(late?.late).toMatch(/^as of 12 ?m(in)? ago$/u);
    const none = prLine(prArt('a', {}, null));
    expect(none).toEqual({ sigil: '#412', words: '', yours: false, late: '' });
    expect(none === null ? [] : prParts(none)).toEqual(['#412']);
  });

  it('is null for any other artifact', () => {
    expect(prLine({ pr: null })).toBeNull();
  });
});

describe('the Pull requests filter', () => {
  const open = item(prArt('open'));
  const draft = item(prArt('draft', {}, status({ state: 'draft' })));
  const merged = item(prArt('merged', { archived_at: 5 }, status({ state: 'merged' })));
  const closed = item(prArt('closed', {}, status({ state: 'closed' })));
  const unknownArchived = item(prArt('old', { archived_at: 5 }, null));
  const page = item({ ...prArt('page'), kind: 'page', url: 'http://127.0.0.1/p/x', pr: null });
  const all = [open, draft, merged, closed, unknownArchived, page];

  it('is its own kind, named Pull requests', () => {
    expect(chipOf(open)).toBe('prs');
    expect(chipWord('prs')).toBe(library.kind_prs);
    expect(chipsOf([open, page]).map((c) => c.kind).sort()).toEqual(['pages', 'prs']);
  });

  it('shows open & draft first, merged & closed on its other side, archived or not', () => {
    expect(prSideItems(all, 'open').map((it) => it.a.id)).toEqual(['open', 'draft']);
    expect(prSideItems(all, 'settled').map((it) => it.a.id)).toEqual(['merged', 'closed', 'old']);
  });

  it('reads the forge state before the archive (he unarchived a merged PR)', () => {
    expect(prSettled({ pr: merged.a.pr, archived_at: null })).toBe(true);
    expect(prSettled({ pr: open.a.pr, archived_at: 9 })).toBe(false);
  });
});
