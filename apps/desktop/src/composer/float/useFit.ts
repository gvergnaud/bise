// src/composer/float · useFloatFit: measures the floating composer and writes fit.ts's answer on
// its root, with no render of its own: data-tall (the docked box's layout), --cmp-field-max (the
// field's max height), data-attach-cut (the attachment list cut to 2 rows) and data-inbox-fold
// (the inbox's other lines folded away). float.css reads
// them. Its DOM side only; what gives way is fit.ts's (pure, tested). Owner: composer-grow.
import { useLayoutEffect, type RefObject } from 'react';
import { floatFit, floatTall, type FloatSizes } from './fit';
import type { FloatState } from './state';

const px = (v: string): number => {
  const n = parseFloat(v);
  return Number.isFinite(n) ? n : 0;
};

/** The float's column boxes: its children, and the children of a box that draws none
 *  (display: contents, as ThreadComposer's focus wrapper around the composer). */
function columnItems(el: Element): HTMLElement[] {
  return [...el.children].flatMap((c) => {
    if (!(c instanceof HTMLElement)) return [];
    return getComputedStyle(c).display === 'contents' ? columnItems(c) : [c];
  });
}

/** What the inbox block holds at its natural height, though its open item may be cut and its other
 *  lines folded now: the block, the open question's text, the other lines (a folded line counts
 *  the height it had shown, kept on it). */
function inboxNatural(block: HTMLElement): { block: number; question: number; others: number } {
  // the open item sits in the block's box of rows (Block.tsx), or on top of it (a closed card)
  const box = block.querySelector<HTMLElement>(':scope > .strip-rows');
  const open = block.querySelector<HTMLElement>(':scope > .inbox-block-open, :scope > .strip-rows > .inbox-block-open');
  const card = open?.querySelector<HTMLElement>('.ic') ?? null;
  const q = card?.querySelector<HTMLElement>('.ic-q') ?? null;
  // the other lines: the head, and the box's rows when it holds the open item (each folds alone)
  const parts = [...block.children].flatMap((c) => (c === box && open !== null && box.contains(open) ? [...box.children] : [c]));
  let others = 0;
  let shown = 0;
  for (const c of parts) {
    if (!(c instanceof HTMLElement) || c === open) continue;
    if (c.offsetHeight > 0) c.dataset.fitH = String(c.offsetHeight);
    others += px(c.dataset.fitH ?? '0');
    shown += c.offsetHeight;
  }
  const hidden = (e: HTMLElement | null): number => (e === null ? 0 : Math.max(0, e.scrollHeight - e.clientHeight));
  const question = q === null ? 0 : q.scrollHeight;
  return { block: block.offsetHeight + hidden(card) + hidden(q) + (others - shown), question, others };
}

/** The heights fit.ts reads, off the float's DOM; null while its field isn't there. */
function measure(root: HTMLElement): { sizes: FloatSizes; lines: number; attached: boolean; empty: boolean } | null {
  const field = root.querySelector<HTMLElement>('.b-composer .b-field');
  if (field === null) return null;
  const list = root.querySelector<HTMLElement>('.b-composer .b-attach');
  const rows = list === null ? [] : [...list.querySelectorAll<HTMLElement>('.b-attach-row')];
  const fs = getComputedStyle(field);
  const line = px(fs.lineHeight) || 22;
  const ls = list === null ? null : getComputedStyle(list);
  const listChrome = ls === null ? 0 : px(ls.marginTop) + px(ls.marginBottom) + px(ls.paddingTop) + px(ls.paddingBottom) + px(ls.borderBottomWidth);
  const listNow = list === null || ls === null ? 0 : list.offsetHeight + px(ls.marginTop) + px(ls.marginBottom);
  // every box of the float's column at its natural height: the open question whole, never as cut now
  let all = 0;
  let question = 0;
  let others = 0;
  for (const el of columnItems(root)) {
    if (!el.classList.contains('inbox-block')) {
      all += el.offsetHeight;
      continue;
    }
    const nat = inboxNatural(el);
    all += nat.block;
    question = nat.question;
    others = nat.others;
  }
  const maxH = px(getComputedStyle(root).maxHeight);
  const sizes: FloatSizes = {
    maxH,
    rest: all - field.offsetHeight - listNow,
    words: field.scrollHeight,
    line,
    cap: maxH / 2,
    rows: rows.length,
    rowH: rows[0]?.offsetHeight ?? 0,
    listChrome,
    question,
    others,
  };
  const empty = field.textContent.trim() === '' && field.querySelector('.b-chip') === null;
  return { sizes, lines: Math.round(field.scrollHeight / line), attached: rows.length > 0, empty };
}

/** Fits the float now and on each change of its size or the window's, until the returned stop. */
function watchFit(el: HTMLElement, state: FloatState): () => void {
  const clear = (): void => {
    delete el.dataset.tall;
    delete el.dataset.attachCut;
    delete el.dataset.inboxFold;
    el.style.removeProperty('--cmp-field-max');
  };
  if (state === 'rest') {
    clear();
    return clear;
  }
  const fit = (): void => {
    const m = measure(el);
    if (m === null) return;
    const tall = floatTall(el.dataset.tall !== undefined, m);
    if (tall) el.dataset.tall = '';
    else delete el.dataset.tall;
    // open, the mini thread gives way first (its own flex): only the focused float fits itself
    if (state !== 'focus') {
      delete el.dataset.attachCut;
      delete el.dataset.inboxFold;
      el.style.removeProperty('--cmp-field-max');
      return;
    }
    const f = floatFit(m.sizes);
    if (f.cutAttach) el.dataset.attachCut = '';
    else delete el.dataset.attachCut;
    if (f.foldOthers) el.dataset.inboxFold = '';
    else delete el.dataset.inboxFold;
    const max = `${String(Math.floor(f.fieldMax))}px`;
    if (el.style.getPropertyValue('--cmp-field-max') !== max) el.style.setProperty('--cmp-field-max', max);
  };
  fit();
  if (typeof ResizeObserver === 'undefined') return clear;
  // the float's size follows his words, the list and the question; the window's height moves the max
  const ro = new ResizeObserver(fit);
  ro.observe(el);
  const field = el.querySelector('.b-composer .b-field');
  if (field !== null) ro.observe(field);
  window.addEventListener('resize', fit);
  return () => {
    ro.disconnect();
    window.removeEventListener('resize', fit);
    clear();
  };
}

/** Keeps the float's fit current while it is focused or open (at rest it is one line). */
export function useFloatFit(root: RefObject<HTMLElement | null>, state: FloatState): void {
  useLayoutEffect(() => {
    const el = root.current;
    return el === null ? undefined : watchFit(el, state);
  }, [root, state]);
}
