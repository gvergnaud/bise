// The thread title row's give-way, on the DOM (designer m_67200, his m_67197): the bar's title holds
// the agent's name, its links (PR, branch, page), its task chip, its subtitle and its state fact;
// when the row runs short each part gives way in giveway.ts ROW_ORDER (rowFit, pure), cut with one
// '…' then out, the task chip down to its mark alone, the name cut last. Every part shows within
// the width it got (overflow hidden), so no two ever draw over each other; one cut or out keeps its
// full text in a tip (its own; the parts that went out, in the name's). Measures, then writes:
// widths, data-out, the chip's data-mark, cut words and tips; the bar's observers never watch those
// attributes, and its text writes are idempotent: no loop. Owner: nm-frame.
import { useEffect, type RefObject } from 'react';
import { cutWords, textWidth } from '@/ds';
import { CUT_MIN_PX, ROW_ORDER, rowFit, type RowKey, type RowPart } from './giveway';

/** A part on the page: its box, the text in it that cuts (none: it never cuts), its words when a WordCut. */
interface Part {
  readonly el: HTMLElement;
  readonly text: HTMLElement | null;
  /** a WordCut's whole words (its data-text): cut here at a word; none: cut by CSS with … */
  readonly words: string | null;
}

const px = (s: string | undefined): number => parseFloat(s ?? '') || 0;
const NONE: RowPart = { whole: 0, least: 0, last: 0 };

/** The parts of the title row there now, by their places in it (BarLinks, TaskChipsOf, Frame's Bar). */
function partsOf(title: HTMLElement): Partial<Record<RowKey, Part>> {
  const one = (sel: string) => title.querySelector<HTMLElement>(sel);
  const part = (el: HTMLElement | null, textSel: string | null): Part | undefined => {
    if (el === null) return undefined;
    const text = textSel === null ? null : textSel === '' ? el : el.querySelector<HTMLElement>(textSel);
    return { el, text, words: text?.dataset['text'] ?? null };
  };
  const out: Partial<Record<RowKey, Part>> = {};
  const set = (k: RowKey, p: Part | undefined) => {
    if (p !== undefined) out[k] = p;
  };
  set('name', part(one(':scope > .t'), ''));
  set('pr', part(one('[data-testid="app.bar.links.pr"]'), null));
  set('branch', part(one('[data-testid="app.bar.links.branch"]'), '.app-bar-link-x'));
  set('page', part(one('[data-testid="app.bar.links.page"]'), '.app-bar-link-t'));
  set('chip', part(one(':scope > .tk-chips'), '.tk-chip-name'));
  set('sub', part(one(':scope > .s'), ''));
  set('fact', part(one(':scope > .app-bar-fact'), null));
  return out;
}

/** Back to whole: what the last fit wrote, undone, so this layout measures each part whole. A
 *  WordCut's words stay as they are (its whole is measured off its data-text): a text written here
 *  would wake the observer again, for ever. */
function reset(title: HTMLElement, parts: Partial<Record<RowKey, Part>>): void {
  for (const p of Object.values(parts)) {
    delete p.el.dataset['out'];
    delete p.el.dataset['mark'];
    if (p.text !== null) {
      p.text.style.width = '';
      p.text.style.flex = '';
    }
  }
  const links = title.querySelector<HTMLElement>(':scope > .app-bar-links');
  if (links !== null) delete links.dataset['out'];
  delete title.dataset['sub'];
}

/** Each part at its own width while it is measured: none shrunk by the row's flex (app.css data-measure). */
function measured<T>(title: HTMLElement, read: () => T): T {
  title.dataset['measure'] = '';
  try {
    return read();
  } finally {
    delete title.dataset['measure'];
  }
}

/** A text's whole width: a WordCut's words in its font, up to its own cap (its max-width); else what it holds. */
function textWhole(p: Part): number {
  if (p.text === null) return 0;
  if (p.words === null) return p.text.scrollWidth;
  const st = getComputedStyle(p.text);
  const cap = st.maxWidth.endsWith('px') ? px(st.maxWidth) : Infinity;
  return Math.min(Math.ceil(textWidth(st.font)(p.words)), cap);
}

/** The whole, the least and the last form of each part, with the gap before it (measured whole). */
function measure(title: HTMLElement, parts: Partial<Record<RowKey, Part>>): Record<RowKey, RowPart> {
  const gap = px(getComputedStyle(title).columnGap);
  const links = title.querySelector<HTMLElement>(':scope > .app-bar-links');
  // the links' box sits 4 nearer the name (its negative margin); the first link shown carries it
  const linksLead = links === null ? 0 : gap + px(getComputedStyle(links).marginLeft);
  const firstLink = links?.firstElementChild ?? null;
  const out = {} as Record<RowKey, RowPart>;
  for (const k of ROW_ORDER) {
    const p = parts[k];
    // not there, or hidden by the bar's own give-way (the fact under the status pill: data-give)
    if (p === undefined || p.el.getClientRects().length === 0) {
      out[k] = NONE;
      continue;
    }
    const inLinks = k === 'pr' || k === 'branch' || k === 'page';
    const before = k === 'name' ? 0 : inLinks ? px(getComputedStyle(p.el).marginLeft) + (p.el === firstLink ? linksLead : 0) : gap;
    const t = textWhole(p);
    // the box around its text: an icon, a padding, the mark
    const chrome = p.el.offsetWidth - (p.text === null ? 0 : p.text.offsetWidth);
    const whole = before + chrome + t;
    const least = p.text === null ? whole : before + chrome + Math.min(t, CUT_MIN_PX);
    if (k === 'name') out[k] = { whole, least, last: least };
    else if (k === 'chip') {
      // its mark alone: the chip's padding and mark, no name, no '+N' (tasks.css data-mark)
      const chip = p.el.querySelector<HTMLElement>('.tk-chip');
      const mark = p.el.querySelector<HTMLElement>('.tk-chip-mark');
      const pad = chip === null ? 0 : px(getComputedStyle(chip).paddingLeft) + px(getComputedStyle(chip).paddingRight);
      out[k] = { whole, least, last: before + pad + (mark?.offsetWidth ?? 0) };
    } else out[k] = { whole, least, last: 0 };
  }
  return out;
}

/** What the title can take: its content box and the status slot's slack (Frame's StatusSlot). */
function roomOf(title: HTMLElement): number {
  const st = getComputedStyle(title);
  const slot = title.parentElement?.querySelector<HTMLElement>(':scope > .app-title-status') ?? null;
  const slack = slot === null ? 0 : Math.max(0, slot.clientWidth - [...slot.children].reduce((n, c) => n + (c as HTMLElement).offsetWidth, 0));
  // a px less: the parts' widths are read rounded (offsetWidth), their texts draw at fractions
  return title.clientWidth - px(st.paddingLeft) - px(st.paddingRight) + slack - 1;
}

/** A part's words for its tip: its own tip, else its text. */
const wordsOf = (p: Part): string => p.el.dataset['tip'] ?? p.words ?? p.el.textContent.trim();

/** The fit's writes: each part's width, out, the chip's mark alone, a WordCut's words, the tips. */
function apply(title: HTMLElement, parts: Partial<Record<RowKey, Part>>, sizes: Record<RowKey, RowPart>, room: number): void {
  const fit = rowFit({ room, ...sizes });
  const gone: string[] = [];
  for (const k of ROW_ORDER) {
    const p = parts[k];
    const f = fit[k];
    if (p === undefined || sizes[k].whole === 0) continue;
    if (f.form === 'last' && k !== 'name' && k !== 'chip') {
      p.el.dataset['out'] = '';
      gone.push(wordsOf(p));
      continue;
    }
    if (f.form === 'last' && k === 'chip') {
      p.el.dataset['mark'] = '';
      continue;
    }
    if (f.form === 'whole') {
      // a WordCut whole still keeps to its cap (the chip's 14ch, the page's 24ch): at a word
      if (p.text !== null && p.words !== null) cutTo(p.text, p.words, Infinity);
      continue;
    }
    if (p.text === null) continue;
    // cut (or the name at its least): its text gets what's left of its width, at a word when a WordCut
    const w = Math.max(0, f.width - (sizes[k].whole - textWhole(p)));
    p.text.style.width = `${String(Math.floor(w))}px`;
    p.text.style.flex = 'none';
    if (p.words !== null) cutTo(p.text, p.words, w);
  }
  // no link shown: their box out too, so the title's gap before it goes with them
  const links = title.querySelector<HTMLElement>(':scope > .app-bar-links');
  if (links !== null && [...links.children].every((c) => (c as HTMLElement).dataset['out'] !== undefined)) links.dataset['out'] = '';
  if (fit.sub.form === 'last' && parts.sub !== undefined && sizes.sub.whole > 0) title.dataset['sub'] = 'out';
  // the subtitle cut: its whole words in its tip
  const sub = parts.sub;
  if (sub !== undefined) setTip(sub.el, fit.sub.form === 'cut' ? sub.el.textContent : '');
  // the name cut, or parts gone: the name's tip says them all, one a line
  const name = parts.name;
  if (name !== undefined) {
    const cut = fit.name.form !== 'whole';
    setTip(name.el, cut || gone.length > 0 ? [name.el.textContent, ...gone].filter((s) => s !== '').join('\n') : '');
  }
}

/** `el`'s words cut after a whole word to `room` px and its own cap, one '…' (ds cutWords). */
function cutTo(el: HTMLElement, words: string, room: number): void {
  const st = getComputedStyle(el);
  const cap = Math.min(room, st.maxWidth.endsWith('px') ? px(st.maxWidth) : Infinity);
  const tw = textWidth(st.font);
  const t = cutWords(words, (s) => tw(s) <= cap + 0.5);
  if (el.textContent !== t) el.textContent = t;
}

function setTip(el: HTMLElement, words: string): void {
  if (words === '') {
    if (el.dataset['tip'] !== undefined && el.dataset['fitTip'] !== undefined) {
      delete el.dataset['tip'];
      delete el.dataset['fitTip'];
    }
    return;
  }
  // never over a tip its own component gave it
  if (el.dataset['tip'] !== undefined && el.dataset['fitTip'] === undefined) return;
  el.dataset['tip'] = words;
  el.dataset['fitTip'] = '';
}

/** The bar title's give-way, kept as the window, the bar or what the title holds change. */
export function useTitleFit(box: RefObject<HTMLElement | null>): void {
  useEffect(() => {
    const title = box.current;
    const bar = title?.parentElement ?? null;
    if (title === null || bar === null) return;
    const read = () => {
      const parts = partsOf(title);
      reset(title, parts);
      const [sizes, room] = measured(title, () => [measure(title, parts), roomOf(title)] as const);
      apply(title, parts, sizes, room);
    };
    read();
    const ro = new ResizeObserver(read);
    ro.observe(bar);
    ro.observe(title);
    // a part comes or goes, a text changes (the fact's minutes, a chip's name): read again; the
    // writes here are attributes it doesn't watch, and texts written only when they differ
    const mo = new MutationObserver(read);
    mo.observe(title, { childList: true, subtree: true, characterData: true });
    return () => {
      ro.disconnect();
      mo.disconnect();
      reset(title, partsOf(title));
    };
  }, [box]);
}
