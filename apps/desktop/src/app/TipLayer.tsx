// The hover tip, one for the whole window (designer m_24359, his 'every compact mark explains
// itself': mark → tip → click opens the full thing). A mark says its words with the ds's
// `tipAttrs('4 agents working')` (data-tip, data-tip-keys, data-tip-side, data-tip-clear); this one layer, mounted once at the
// root, reads the pointer and the focus for the whole window and draws the ds's tip (.b-tip) in body.
//
// The tip is never interactive (pointer-events none): the pointer over it still hovers and clicks
// the row under it, and it never flickers when the pointer crosses it. Where and when are the ds's
// (src/ds/tip.ts, pure). A mark's words that change while its tip shows (a turn's age) follow.
// Hidden on a press, a key, a scroll, the pointer leaving the window or the window losing focus.
// Owner: amb-win.
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { besideBox, clearBox, placeTip, tipWait, TIP_WARM_MS, type TipSide } from '@/ds';

/** The mark under an element: the closest one with words. */
function markOf(t: EventTarget | null): HTMLElement | null {
  // a list that says data-tip-quiet holds its tips (the sidebar's projects while one is dragged
  // and after the drop until the pointer moves: a row landed under a still pointer, designer m_49316)
  if (t instanceof Element && t.closest('[data-tip-quiet]') !== null) return null;
  const el = t instanceof Element ? t.closest<HTMLElement>('[data-tip]') : null;
  return el !== null && (el.dataset.tip ?? '') !== '' ? el : null;
}

const focusVisible = (el: Element): boolean => {
  try {
    return el.matches(':focus-visible');
  } catch {
    return false;
  }
};

interface Shown {
  readonly mark: HTMLElement;
  readonly words: string;
  readonly keys: string;
  /** the side it goes first: 'below' or 'left' when the mark asks (data-tip-side), else above */
  readonly first: TipSide;
  /** it clears its outer mark's box (data-tip-clear 'outer'), not only its own */
  readonly clear: boolean;
  /** its left edge on the mark's (data-tip-align 'start'), not centred */
  readonly start: boolean;
  /** shown cold (after the delay): it fades in; a warm one is there at once */
  readonly fade: boolean;
}

export function TipLayer({ testId = 'tip' }: { testId?: string }) {
  const [shown, setShown] = useState<Shown | null>(null);
  const tipRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let current: HTMLElement | null = null; // the mark shown or about to show
    let showing = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let warmUntil = 0;
    let watch: MutationObserver | undefined;
    const read = (mark: HTMLElement, fade: boolean): Shown => ({
      mark,
      words: mark.dataset.tip ?? '',
      keys: mark.dataset.tipKeys ?? '',
      first: mark.dataset.tipSide === 'below' ? 'below' : mark.dataset.tipSide === 'left' ? 'left' : 'above',
      clear: mark.dataset.tipClear === 'outer',
      start: mark.dataset.tipAlign === 'start',
      fade,
    });
    const stop = () => {
      if (timer !== undefined) clearTimeout(timer);
      timer = undefined;
      watch?.disconnect();
      watch = undefined;
    };
    const hide = () => {
      stop();
      if (showing) warmUntil = Date.now() + TIP_WARM_MS;
      current = null;
      showing = false;
      setShown(null);
    };
    const show = (mark: HTMLElement, fade: boolean) => {
      stop();
      current = mark;
      showing = true;
      setShown(read(mark, fade));
      // its words change while it shows ('working for 6 min' → '7 min'): the tip follows
      watch = new MutationObserver(() => {
        if ((mark.dataset.tip ?? '') === '') hide();
        else setShown((s) => (s?.mark === mark ? read(mark, false) : s));
      });
      watch.observe(mark, {
        attributes: true,
        attributeFilter: ['data-tip', 'data-tip-keys', 'data-tip-side', 'data-tip-align'],
      });
    };
    const to = (mark: HTMLElement | null, focus: boolean) => {
      if (mark === current) return;
      if (mark === null) {
        hide();
        return;
      }
      const wait = showing ? 0 : tipWait(Date.now(), warmUntil, focus);
      if (showing) warmUntil = Date.now() + TIP_WARM_MS;
      stop();
      current = mark;
      if (wait === 0) show(mark, false);
      else {
        showing = false;
        setShown(null);
        timer = setTimeout(() => {
          show(mark, true);
        }, wait);
      }
    };
    const over = (e: PointerEvent) => {
      to(markOf(e.target), false);
    };
    const out = (e: PointerEvent) => {
      // out of the window
      if (e.relatedTarget === null) hide();
    };
    const focusIn = (e: FocusEvent) => {
      const t = e.target;
      if (t instanceof Element && focusVisible(t)) to(markOf(t), true);
    };
    const focusOut = (e: FocusEvent) => {
      if (current !== null && e.target instanceof Node && current.contains(e.target)) hide();
    };
    const doc = document;
    doc.addEventListener('pointerover', over, true);
    doc.addEventListener('pointerout', out, true);
    doc.addEventListener('pointerdown', hide, true);
    doc.addEventListener('keydown', hide, true);
    doc.addEventListener('scroll', hide, true);
    doc.addEventListener('focusin', focusIn, true);
    doc.addEventListener('focusout', focusOut, true);
    window.addEventListener('blur', hide);
    return () => {
      stop();
      doc.removeEventListener('pointerover', over, true);
      doc.removeEventListener('pointerout', out, true);
      doc.removeEventListener('pointerdown', hide, true);
      doc.removeEventListener('keydown', hide, true);
      doc.removeEventListener('scroll', hide, true);
      doc.removeEventListener('focusin', focusIn, true);
      doc.removeEventListener('focusout', focusOut, true);
      window.removeEventListener('blur', hide);
    };
  }, []);

  // placed once its size is known, before the paint
  useLayoutEffect(() => {
    const el = tipRef.current;
    if (shown === null || el === null) return;
    const m = shown.mark.getBoundingClientRect();
    // a small mark inside a row that is a mark itself may ask to clear that row (clearBox)
    const o = shown.clear ? (shown.mark.parentElement?.closest<HTMLElement>('[data-tip]')?.getBoundingClientRect() ?? null) : null;
    // a 'left' tip goes off the box it is in (data-tip-box: the outer card), centred on its mark
    const b = shown.first === 'left' ? (shown.mark.closest<HTMLElement>('[data-tip-box]')?.getBoundingClientRect() ?? null) : null;
    const mark = { left: m.left, top: m.top, width: m.width, height: m.height };
    const box = shown.first === 'left'
      ? besideBox(mark, b === null ? null : { left: b.left, top: b.top, width: b.width, height: b.height })
      : clearBox(mark, o === null ? null : { left: o.left, top: o.top, width: o.width, height: o.height });
    const at = placeTip(box, { width: el.offsetWidth, height: el.offsetHeight }, { width: window.innerWidth, height: window.innerHeight }, shown.first, shown.start ? 'start' : 'centre');
    el.style.left = `${String(at.left)}px`;
    el.style.top = `${String(at.top)}px`;
    el.dataset.side = at.side;
    el.style.visibility = 'visible';
  }, [shown]);

  if (shown === null) return null;
  return createPortal(
    <div ref={tipRef} className={shown.fade ? 'b-tip in' : 'b-tip'} role="tooltip" style={{ visibility: 'hidden' }} data-testid={testId}>
      <span className="w" data-testid={`${testId}.words`}>
        {/* `…` parts mono in text (a branch, a folder: wt-sign's ψ words, designer m_24019) */}
        {shown.words.split('`').map((t, i) =>
          i % 2 === 1 ? (
            <span key={i} className="m">
              {t}
            </span>
          ) : (
            t
          ),
        )}
      </span>
      {shown.keys !== '' && <span className="k">{shown.keys}</span>}
    </div>,
    document.body,
  );
}
