// src/app/panel · the panel's tabs and its ▯ (page native-mac §6 and mockups 1-4). BarTabs: the ▯
// at the end of the window's title bar (Frame.tsx mounts it, so nm-panel never edits Frame.tsx),
// pressed while the panel is open; ⌥⌘B does the same (⌥⌘0 too). PanelTabs: the tabs in the panel's top row,
// the window's one 52 bar over the panel's column (his m_44307: no band of its own; it drags and
// double-clicks as the bar's ground; mockup 3: from the panel's left + 12; ui-review m_42482 #1): 28 high, padding 0 12, r 8, 13/500, the icon 14, 160 at most with '…';
// × on the one in front and on hover, its slot kept so a tab never changes width (#4); its thing's
// pin just left of the × on hover, over the title's end (TabPin.tsx, his m_50763); Overview,
// the home, has none; '+' 28 opens New tab. As many whole tabs as the row holds, six at most
// (HIG), the rest behind an overflow button whose native menu lists them, none cut (#3); a drag
// reorders; a right click is the tab's menu (nm-menus' useEntityMenu({ kind: 'tab', id })).
// Owner: nm-panel (r2-titlebar this round).
import { useLayoutEffect, useRef, useState, type DragEvent, type KeyboardEvent, type ReactNode, type RefObject } from 'react';
import { AppIcon, IconButton, Press, tipAttrs } from '@/ds';
import { useModHeld } from '@/app/held';
import { BarButton } from '@/app/barbutton';
import { panel } from '@/proto/words_panel.gen';
import { useEntityMenu } from '@/app/menus/use';
import { barGroundDoubleClick } from '@/app/barslot';
import { fitTabs, shownOf, TAB_GAP, type Tab } from './model';
import { activateTab, closeTabId, moveTabTo, openLauncher, panelToggle, usePanelState } from './state';
import { useTabLooks, type TabLook } from './look';
import { PANEL_KEYS } from './keys';
import { TabPin } from './TabPin';

/** The panel's ▯ at the title bar's end (⌥⌘B), pressed while it is open; ⌥⌘ held, its ⌥⌘B keycap in place of the glyph (designer m_45008). */
export function BarTabs() {
  const s = usePanelState();
  const held = useModHeld('AltMeta');
  const label = s.open ? panel.panel_hide({ key: PANEL_KEYS.toggle }) : panel.panel_show({ key: PANEL_KEYS.toggle });
  return (
    <div className="pn-bar" data-testid="app.panel.bar" data-open={s.open || undefined}>
      <BarButton icon="panel" label={label} tip={label} keys={PANEL_KEYS.toggle} hint={held} pressed={s.open} onClick={panelToggle} testId="app.panel.toggle" />
    </div>
  );
}

/** The width an element has, live. */
function useWidthOf(): readonly [RefObject<HTMLDivElement | null>, number] {
  const ref = useRef<HTMLDivElement>(null);
  const [w, setW] = useState(0);
  useLayoutEffect(() => {
    const el = ref.current;
    if (el === null) return;
    setW(el.clientWidth);
    const ro = new ResizeObserver(() => {
      setW(el.clientWidth);
    });
    ro.observe(el);
    return () => {
      ro.disconnect();
    };
  }, []);
  return [ref, w];
}

/** What the tab strip keeps past its tabs: '+' (28) and the overflow button (⌄ N), each with its gap. */
const ADD_W = 28 + TAB_GAP;
const OVER_W = 48 + TAB_GAP;
/** The full width's lead and the first tab: 12 apart. */
const LEAD_GAP = 12;

/**
 * The tabs in the panel's top row, '+', and at its end what the tab in front adds (`end`). A sheet's
 * row starts with its tabs too: the bar's ▯ above it, at the window's edge, folds it (designer m_44391).
 */
export function PanelTabs({ end, start }: { end?: ReactNode; start?: ReactNode }) {
  const s = usePanelState();
  const look = useTabLooks();
  const [strip, room] = useWidthOf();
  // `start` (the full width's lead, designer m_45641): 12 before the tabs, the first to go when
  // they would not all fit; while out it stays measured, unseen
  const [leadEl, leadW] = useWidthOf();
  // the lead and the strip's row: what the tabs have when the lead is out
  const [row, roomAll] = useWidthOf();
  // each tab's width at its title (designer m_43050 #5), measured in a hidden row of them all
  const measure = useRef<HTMLDivElement>(null);
  const [widths, setWidths] = useState<readonly number[]>([]);
  const titles = s.tabs.list.map((t) => `${t.id}\u0000${look(t).title}`).join('\u0001');
  useLayoutEffect(() => {
    const el = measure.current;
    if (el === null) return;
    setWidths(Array.from(el.children, (c) => (c as HTMLElement).offsetWidth));
  }, [titles]);
  const active = s.tabs.list.findIndex((t) => t.id === s.tabs.active);
  // before the first measure (a test's DOM has no layout): six, the HIG's most
  const measured = room > 0 && widths.length === s.tabs.list.length && widths.every((w) => w > 0);
  const leadIn = start !== undefined && (!measured || leadW === 0 || roomAll === 0 || fitTabs(widths, active, roomAll - leadW - LEAD_GAP - ADD_W) >= s.tabs.list.length);
  const all = measured ? fitTabs(widths, active, room - ADD_W) : undefined;
  const fit = all === undefined || all >= s.tabs.list.length ? all : fitTabs(widths, active, room - ADD_W - OVER_W);
  const { shown, over } = shownOf(s.tabs, fit);
  const [drag, setDrag] = useState<string | null>(null);
  return (
    // the bar's own row over the panel's column: its ground zooms on a double-click as the bar's
    <div className="pn-top" data-testid="app.panel.top" onDoubleClick={barGroundDoubleClick}>
        <div className="pn-measure" ref={measure} aria-hidden>
          {s.tabs.list.map((t) => {
            const l = look(t);
            return (
              <span key={t.id} className="pn-tab">
                {l.icon}
                <span className="pn-tab-t">{l.title}</span>
                {(
                  <span className="pn-tab-x">
                    <AppIcon name="close" size={14} />
                  </span>
                )}
              </span>
            );
          })}
        </div>
        <div className="pn-row" ref={row}>
        {start !== undefined && (
          <div className="pn-lead" ref={leadEl} data-out={!leadIn || undefined} aria-hidden={!leadIn || undefined}>
            {start}
          </div>
        )}
        <div className="pn-tabs" ref={strip} role="tablist" aria-label={panel.tabs_label} data-testid="app.panel.tabs">
          {shown.map((t) => (
            <BarTab
              key={t.id}
              tab={t}
              look={look(t)}
              on={t.id === s.tabs.active}
              dragging={drag === t.id}
              onDragStart={() => {
                setDrag(t.id);
              }}
              onDropAt={() => {
                if (drag !== null && drag !== t.id) moveTabTo(drag, s.tabs.list.indexOf(t));
                setDrag(null);
              }}
              onDragEnd={() => {
                setDrag(null);
              }}
            />
          ))}
          {over.length > 0 && <OverButton over={over} look={look} />}
          <IconButton label={panel.tab_add({ key: PANEL_KEYS.add })} size="bar" onClick={openLauncher} testId="app.panel.add">
            <AppIcon name="plus" size={16} />
          </IconButton>
        </div>
        </div>
        {end}
    </div>
  );
}

function BarTab({ tab, look, on, dragging, onDragStart, onDropAt, onDragEnd }: {
  tab: Tab;
  look: TabLook;
  on: boolean;
  dragging: boolean;
  onDragStart: () => void;
  onDropAt: () => void;
  onDragEnd: () => void;
}) {
  const menu = useEntityMenu({ kind: 'tab', id: tab.id });
  const [near, setNear] = useState(false);
  // a tip only when the title is cut, never one that repeats the label he reads (ui-review m_42619 #5)
  const titleEl = useRef<HTMLSpanElement>(null);
  const [cut, setCut] = useState(false);
  const pick = () => {
    activateTab(tab.id);
  };
  const keys = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.target !== e.currentTarget) return;
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      pick();
    }
  };
  const close = panel.tab_close({ key: PANEL_KEYS.close });
  return (
    <div
      className="pn-tab"
      role="tab"
      tabIndex={0}
      aria-selected={on}
      aria-label={look.title}
      {...(cut ? tipAttrs(look.title) : {})}
      onPointerEnter={() => {
        const el = titleEl.current;
        setCut(el !== null && el.scrollWidth > el.clientWidth);
      }}
      data-on={on || undefined}
      data-drag={dragging || undefined}
      data-near={near || undefined}
      data-testid={`app.panel.tab.${tab.id}`}
      draggable
      onClick={pick}
      onKeyDown={keys}
      onContextMenu={menu.onContextMenu}
      onDragStart={(e: DragEvent<HTMLDivElement>) => {
        e.dataTransfer.effectAllowed = 'move';
        onDragStart();
      }}
      onDragOver={(e) => {
        e.preventDefault();
        setNear(true);
      }}
      onDragLeave={() => {
        setNear(false);
      }}
      onDrop={(e) => {
        e.preventDefault();
        setNear(false);
        onDropAt();
      }}
      onDragEnd={onDragEnd}
    >
      {look.icon}
      <span className="pn-tab-t" ref={titleEl}>{look.title}</span>
      {tab.what.kind === 'thing' && <TabPin e={tab.what.e} testId={`app.panel.tab.${tab.id}.pin`} />}
      {<Press
        look="pn-tab-x"
        aria-label={close}
        {...tipAttrs(close)}
        tabIndex={on ? 0 : -1}
        onClick={(e) => {
          e.stopPropagation();
          closeTabId(tab.id);
        }}
        data-testid={`app.panel.tab.${tab.id}.close`}
      >
        <AppIcon name="close" size={14} />
      </Press>}
    </div>
  );
}

/** Past six tabs: a button whose native menu lists the rest; a pick brings it to the front. */
function OverButton({ over, look }: { over: readonly Tab[]; look: (t: Tab) => TabLook }) {
  const words = panel.tabs_more({ n: over.length });
  return (
    <Press
      look="pn-over"
      aria-label={words}
      aria-haspopup="menu"
      {...tipAttrs(words)}
      data-testid="app.panel.over"
      onClick={(e) => {
        const box = e.currentTarget.getBoundingClientRect();
        const bise = (window as Partial<Window>).bise;
        void bise?.popupMenu(over.map((t) => ({ id: t.id, label: look(t).title })), { x: box.left, y: box.top, width: box.width, height: box.height }).then((id) => {
          if (id !== null) activateTab(id);
        });
      }}
    >
      <AppIcon name="down" size={14} />
      <span>{over.length}</span>
    </Press>
  );
}
