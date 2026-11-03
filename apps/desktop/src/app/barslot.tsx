// The page's room in the window's bar (bise fait mac, app-polish #4; native-lead m_42140): the
// title bar draws <BarSlot/> right of the crumb, before the panel's tabs and its ▯; a page renders
// <InBar>{its '…'}</InBar> anywhere in its tree and that content shows
// in the bar (a portal), gone when the page unmounts. One page's content at a time: the route's
// screen (App.tsx remounts the screen on each route). The slot drags the window like the bar;
// what a page puts in it does not (app.css .app-barslot > *).
// <InTitle>{node}</InTitle> (nm-thread m_43271) does the same for the bar's title: while mounted, its
// node replaces the title's text (the thread's inline rename field); the subtitle stays.
// barGroundDoubleClick: a double-click on the bar's own ground (the bar, and the panel's tab row in
// the same row: one bar, his m_44307) zooms or minimises as his Mac says. Owner: nm-frame
// (r2-titlebar this round).
import { useEffect, useSyncExternalStore, type MouseEvent, type ReactNode } from 'react';
import { createPortal } from 'react-dom';

/** A double-click on the bar's ground: zoom or minimise (window.bise.barDoubleClick); on a control it is the control's. */
export function barGroundDoubleClick(e: MouseEvent<HTMLElement>): void {
  if (e.target !== e.currentTarget && (e.target as HTMLElement).closest("button, a, input, [role='button'], [role='tab']") !== null) return;
  (window as Partial<Window>).bise?.barDoubleClick();
}

/** One portal target in the bar: its element (null: no bar drawn, the onboarding's window) and how many fill it. */
function slotStore() {
  let el: HTMLElement | null = null;
  let users = 0;
  const listeners = new Set<() => void>();
  const tell = () => {
    for (const l of listeners) l();
  };
  const subscribe = (cb: () => void) => {
    listeners.add(cb);
    return () => {
      listeners.delete(cb);
    };
  };
  return {
    set: (next: HTMLElement | null): void => {
      if (el === next) return;
      el = next;
      tell();
    },
    useEl: (): HTMLElement | null => useSyncExternalStore(subscribe, () => el),
    useFilled: (): boolean => useSyncExternalStore(subscribe, () => users > 0),
    /** Counts one filler while it is mounted. */
    useFill(): void {
      useEffect(() => {
        users += 1;
        tell();
        return () => {
          users -= 1;
          tell();
        };
      }, []);
    },
  };
}

const bar = slotStore();
const title = slotStore();
// the slots' ref callbacks, plain module functions (a store's field read in render reads as a ref to the lint)
const setBar = bar.set;
const setTitle = title.set;

/** The slot, in the title bar (Frame.tsx): empty until a page fills it. */
export function BarSlot() {
  return <div ref={setBar} className="app-barslot" data-testid="app.bar.slot" />;
}

/** What a page shows in the window's bar: its action ('…'); nothing once the page leaves. A page's
 *  search is never there: it goes in its list's row (ds ListBar, his m_52658). */
export function InBar({ children }: { children: ReactNode }) {
  const el = bar.useEl();
  return el === null ? null : createPortal(children, el);
}

/** The bar's title: its text, or what an <InTitle> puts there while it is mounted. */
export function TitleSlot({ text }: { text: ReactNode }) {
  const filled = title.useFilled();
  return (
    <span ref={setTitle} className="t" data-testid="app.bar.title.text">
      {filled ? null : text}
    </span>
  );
}

/** What replaces the bar's title text while it is mounted (the thread's rename field); nothing once it leaves. */
export function InTitle({ children }: { children: ReactNode }) {
  title.useFill();
  const el = title.useEl();
  return el === null ? null : createPortal(children, el);
}
