// '@ put it in a message' (desktop-screens v6, designer m_11837; attach.rs insert_artifact): the
// artifact goes as a chip at the end of the agent's composer draft, then his window shows that
// thread, the caret after the chip. Through the composer's own drafts (drafts.ts + entry.ts), so a
// draft he had keeps every word and chip. Pure over a Storage-like store. Owner: amb-kit.
import { keepUnread, load, save, type Store } from '@/composer/drafts';
import { entryOf, entrySegments, messageOf, type DraftSeg } from '@/composer/entry';
import { nextChipNumber, type Segment } from '@/composer/message';

/**
 * Words at the end of `target`'s draft ('Ask perf to fix' on a PR's failing check: the check and
 * its log, never sent by itself): a draft he had keeps every word and chip, a line between.
 */
export function putTextInDraft(store: Store, target: string, text: string): void {
  const saved = load(store, target);
  let segs: DraftSeg[] = [];
  if (saved.draft !== null) {
    const read = entrySegments(saved.draft);
    if (read === null) keepUnread(store, target, saved.draft);
    else segs = read;
  }
  const gap = segs.length === 0 ? [] : [{ kind: 'text', text: '\n\n' } as const];
  save(store, target, { ...saved, draft: entryOf([...segs, ...gap, { kind: 'text', text }]) });
}

/** The artifact as a chip at the end of `target`'s draft (a project's agent: 'p/agent'). */
export function putInDraft(store: Store, target: string, a: { id: string; title: string; url: string }): void {
  const saved = load(store, target);
  let segs: DraftSeg[] = [];
  if (saved.draft !== null) {
    const read = entrySegments(saved.draft);
    // a draft the composer can't read is never dropped: it goes aside first
    if (read === null) keepUnread(store, target, saved.draft);
    else segs = read;
  }
  const last = segs.at(-1);
  const gap = last !== undefined && !(last.kind === 'text' && /\s$/u.test(last.text)) ? [{ kind: 'text', text: ' ' } as const] : [];
  const chip: Segment = { kind: 'chip', chip: { kind: 'artifact', n: nextChipNumber(messageOf(segs)), id: a.id, title: a.title, url: a.url } };
  save(store, target, { ...saved, draft: entryOf([...segs, ...gap, chip, { kind: 'text', text: ' ' }]) });
}
