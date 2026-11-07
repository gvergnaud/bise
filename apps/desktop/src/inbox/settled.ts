// An answered or withdrawn record as the waiting card it was (inbox-answered, Gabriel's pick A: the
// inbox page's pane mounts InboxCard settled, one component, no new look): the card's fields from
// the record, what its chip shows (the question's own artifact, the record's `about`; an older
// record without one: the page he answered on, never a link guessed from the words), the page he
// answered on when it is another one, the option he picked and the foot's words. Pure. Owner:
// inbox-answered (inbox-screen's pane).
import type { ArtLink } from '@/proto/rows';
import { inbox } from '@/proto/words_inbox.gen';
import type { InboxItem } from './listed';
import { closedMs, dayHead, saidWords, whereWords, type InboxRecord } from './past';
import { timeOf } from '@/artifacts/model';

/** The card it was: its asker, question and options; its link the question's own (or none). Pure. */
export function settledCard(r: InboxRecord): InboxItem {
  const about = aboutOfRecord(r);
  return {
    id: r.card,
    project: r.project,
    kind: r.kind,
    agent: r.asker,
    question: r.question,
    options: r.options.map((label, i) => ({ n: i + 1, label })),
    urgent: false,
    since_ms: r.asked_ms,
    artifact: about,
    refused: null,
  };
}

/** The page he answered on, as a chip's link: its block the anchor. Null: not a page answer. Pure. */
export function answeredPage(r: InboxRecord): ArtLink | null {
  const w = r.where;
  if (r.state !== 'answered' || w?.kind !== 'page') return null;
  return { id: w.page, kind: 'page', title: w.page, v: null, url: null, at: w.block === undefined ? null : { kind: 'block', block: w.block } };
}

/** What the question is about: the record's `about` (the card's link as it opened); an older
 *  record's: the page he answered on; else null. Pure. */
export function aboutOfRecord(r: InboxRecord): ArtLink | null {
  return r.about ?? answeredPage(r);
}

/** The page he answered on, when it is not the question's own (the second chip); else null. Pure. */
export function alsoPage(r: InboxRecord): ArtLink | null {
  const p = answeredPage(r);
  const about = aboutOfRecord(r);
  return p === null || about === null || (about.kind === 'page' && about.id === p.id) ? null : p;
}

/** The option he picked (its n): his number, else the option his words are; null for his own words. Pure. */
export function pickedN(r: InboxRecord): number | null {
  if (r.state !== 'answered') return null;
  if (r.pick_n != null && r.pick_n >= 1 && r.pick_n <= r.options.length) return r.pick_n;
  const i = r.options.indexOf(saidWords(r));
  return i < 0 ? null : i + 1;
}

/** 'today 14:02' → '14:02', else 'oct 12 14:02'; unknown: the day's word. Pure. */
export function whenOf(ms: number, now: number): string {
  const day = dayHead(ms, now);
  return ms <= 0 ? day : day === inbox.day_today ? timeOf(ms) : `${day} ${timeOf(ms)}`;
}

/** The jump's words (⌘⏎, designer m_55234: one line in the 320 pane): 'open your answer' (its
 *  thread entry or its page block), withdrawn 'open the thread' (the asker's). Pure. */
export function jumpWords(r: InboxRecord): string {
  return r.state === 'answered' ? inbox.jump_answer : inbox.jump_asker;
}

/** The foot's words: 'answered on the page · 21:29' (the chip above it), 'picked 2 in the inbox ·
 *  21:29', "answered in perf's thread · 21:29", 'withdrawn · 15:02'. Pure. */
export function footWords(r: InboxRecord, now: number): string {
  const when = whenOf(closedMs(r), now);
  if (r.state !== 'answered') return inbox.when_withdrawn({ when });
  if (r.where == null) return inbox.when_answered({ when });
  const place = r.where.kind === 'page' ? inbox.where_the_page : whereWords(r.where);
  const n = r.pick_n != null && r.pick_n >= 1 && r.pick_n <= r.options.length ? r.pick_n : null;
  return n === null ? inbox.when_answered_at({ place, when }) : inbox.when_picked_at({ n, place, when });
}
