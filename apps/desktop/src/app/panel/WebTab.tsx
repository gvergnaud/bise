// src/app/panel · a web page in a browser tab (page newtab-browser; designer m_64680, architect
// m_64682): the address bar (← → ↻ or × while it loads, the address, ↗ his browser, ⋯) over the
// page. The page is main's (main/webview/views.ts): a native view laid over this tab's box; the tab
// sends its key, its box's rect and whether it shows (its tab in front, nothing over it:
// ds/cover.ts), and verbs; main sends the page's state back (web.ts), which the tab keeps (its URL,
// title, back/forward). The address is ⌘T's omnibox (Omnibox.tsx): a click or ⌘L selects it, his
// words list his pages and artifacts over the page (the page hides meanwhile), ⏎ goes in this tab,
// ⌘⏎ in a web tab behind, esc gives the page back. ⌘F finds in the page (findInPage). Under the
// bar, one line when needed: a download saved ('pricing.pdf saved to Downloads · Show in Finder'),
// a page that didn't load. ⋯: 'Always open <site> in your browser' (kept per site, Settings ›
// Privacy lists them), Open in your browser. Owner: newtab-omnibox.
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { AppIcon, IconButton, Input, Press, useCovered, useCoverWhile } from '@/ds';
import { panel } from '@/proto/words_panel.gen';
import { useActionTaker } from '@/app/menu';
import { panelAt, takeOpened } from './state';
import { OmniList, useOmni } from './Omnibox';
import { shownUrl } from './omni';
import { dropSaved, noteOpened, siteOfUrl, useWebSide, webKey, webOpened } from './web';
import { useWebHistory } from './webpages';
import type { WebPageOf } from './model';

/** The box's rect in the window's CSS pixels. Pure. */
function rectOf(el: HTMLElement, visible: boolean) {
  const r = el.getBoundingClientRect();
  return { x: r.left, y: r.top, w: r.width, h: r.height, visible };
}

/** A web page's tab: its bar and its page's box. */
export function WebTab({ tabId, shown, page }: { tabId: string; shown: boolean; page: WebPageOf }) {
  const bise = (window as Partial<Window>).bise;
  const web = bise?.web;
  const [key] = useState(() => webKey(panelAt(), tabId));
  const side = useWebSide(key);
  // one signal: an overlay of the page (a sheet, a pop, this omnibox's list) hides the page (ds/cover.ts)
  const covered = useCovered();
  const hist = useWebHistory();
  const url = side.state?.url !== undefined && side.state.url !== '' ? side.state.url : page.url;
  // the address field: his words while he edits, else the page's address
  const [editing, setEditing] = useState(false);
  const [q, setQ] = useState('');
  const [finding, setFinding] = useState<string | null>(null);
  const field = useRef<HTMLInputElement>(null);
  const findField = useRef<HTMLInputElement>(null);
  const box = useRef<HTMLDivElement>(null);
  const omni = useOmni({
    tabId,
    ref: field,
    typed: q,
    setTyped: setQ,
    page: (to, behind) => {
      if (behind) return false;
      web?.go(key, to, true);
      setEditing(false);
      web?.focus(key);
      return true;
    },
  });
  const listing = editing && q.trim() !== '' && q !== url;
  // the omnibox's list over the page: the page hides while it shows (architect m_64682 rule 1)
  useCoverWhile(listing && shown);
  // its view, made once: its kept back/forward, else its first address
  useEffect(() => {
    if (web === undefined || webOpened(key)) return;
    noteOpened(key);
    web.open(key, page.entries !== undefined && page.index !== undefined ? { entries: page.entries, index: page.index } : { url: page.url, typed: page.typed === true });
    // its first page only: the tab keeps what main says after
  }, [web, key, page]);
  // the box's rect, at each layout, and hidden when the tab goes
  const visible = shown && !covered;
  useLayoutEffect(() => {
    const el = box.current;
    if (web === undefined || el === null) return;
    const send = () => {
      web.rect(key, rectOf(el, visible));
    };
    send();
    const ro = new ResizeObserver(send);
    ro.observe(el);
    window.addEventListener('resize', send);
    // the panel slides and its splitter moves the box without resizing it: follow each frame while shown
    let raf = 0;
    const follow = () => {
      send();
      raf = requestAnimationFrame(follow);
    };
    if (visible) raf = requestAnimationFrame(follow);
    return () => {
      ro.disconnect();
      window.removeEventListener('resize', send);
      cancelAnimationFrame(raf);
      web.rect(key, { ...rectOf(el, false), visible: false });
    };
  }, [web, key, visible]);
  // ⌘L on a web tab (or just opened by its launcher): the address, selected
  useEffect(() => {
    if (!shown || !takeOpened(tabId)) return;
    if (page.url === '') field.current?.focus();
    else web?.focus(key);
  }, [shown, tabId, page.url, web, key]);
  // ⌘F with this tab in front: find in its page
  useActionTaker('find', () => {
    if (!shown) return false;
    setFinding((f) => f ?? '');
    requestAnimationFrame(() => {
      findField.current?.focus();
      findField.current?.select();
    });
    return true;
  });
  const s = side.state;
  const site = siteOfUrl(url);
  const outOn = site !== null && hist.out.includes(site);
  const more = () => {
    if (bise === undefined) return;
    const items = [
      { id: 'browser', label: panel.addr_out },
      ...(site === null ? [] : [{ id: 'out', label: panel.web_out_site({ site }), checked: outOn }]),
    ];
    void bise.popupMenu(items).then((id) => {
      if (id === 'browser') void bise.openExternal(url);
      else if (id === 'out' && site !== null) void bise.webHistory?.out(url, !outOn);
    });
  };
  const edit = () => {
    setQ(url);
    setEditing(true);
    requestAnimationFrame(() => {
      field.current?.select();
    });
  };
  const optId = (i: number) => `${tabId}.opt.${String(i)}`;
  return (
    <div className="pn-browser" data-testid="app.panel.web" data-url={url}>
      <div className="pn-addr" data-testid="app.panel.addr">
        <IconButton label={panel.addr_back} size="bar" disabled={s?.back !== true} onClick={() => { web?.act(key, 'back'); }} testId="app.panel.addr.back">
          <AppIcon name="back" size={16} />
        </IconButton>
        <IconButton label={panel.addr_forward} size="bar" disabled={s?.forward !== true} onClick={() => { web?.act(key, 'forward'); }} testId="app.panel.addr.forward">
          <AppIcon name="forward" size={16} />
        </IconButton>
        {s?.loading === true ? (
          <IconButton label={panel.web_stop} size="bar" onClick={() => { web?.act(key, 'stop'); }} testId="app.panel.addr.stop">
            <AppIcon name="close" size={16} />
          </IconButton>
        ) : (
          <IconButton label={panel.addr_reload} size="bar" onClick={() => { web?.act(key, 'reload'); }} testId="app.panel.addr.reload">
            <AppIcon name="refresh" size={16} />
          </IconButton>
        )}
        <label className="pn-url pn-url-field">
          <AppIcon name="search" size={14} />
          <Input
            ref={field}
            look="pn-url-in"
            value={editing ? omni.value : shownUrl(url)}
            aria-label={panel.addr_enter}
            role="combobox"
            aria-expanded={listing}
            aria-controls={`${tabId}.list`}
            aria-activedescendant={listing && omni.rows[omni.field.at] !== undefined ? optId(omni.field.at) : undefined}
            onFocus={() => {
              if (!editing) edit();
            }}
            onBlur={() => {
              setEditing(false);
            }}
            onChange={(e) => {
              omni.input(e.target.value, e.nativeEvent as InputEvent);
            }}
            onKeyDown={(e) => {
              if (listing && omni.key(e)) return;
              if (e.key === 'Enter') {
                e.preventDefault();
                const r = omni.rows[0];
                if (r !== undefined) omni.open(r, e.metaKey || e.ctrlKey);
              } else if (e.key === 'Escape') {
                // his words dropped, the page back in front (Chrome)
                e.preventDefault();
                e.stopPropagation();
                setEditing(false);
                web?.focus(key);
              }
            }}
            data-testid="app.panel.addr.field"
          />
        </label>
        <IconButton label={panel.addr_out} size="bar" onClick={() => { void bise?.openExternal(url); }} testId="app.panel.addr.out">
          <AppIcon name="open" size={16} />
        </IconButton>
        <IconButton label={panel.web_more} size="bar" onClick={more} testId="app.panel.addr.more">
          <AppIcon name="more" size={16} />
        </IconButton>
      </div>
      {finding !== null && (
        <div className="pn-webline" data-testid="app.panel.web.find">
          <Input
            ref={findField}
            look="pn-url-in"
            value={finding}
            placeholder={panel.web_find}
            aria-label={panel.web_find}
            onChange={(e) => {
              setFinding(e.target.value);
              web?.find(key, e.target.value, false, false);
            }}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                web?.find(key, finding, e.shiftKey, true);
              } else if (e.key === 'Escape') {
                e.preventDefault();
                e.stopPropagation();
                web?.find(key, '', false, false);
                setFinding(null);
                web?.focus(key);
              }
            }}
            data-testid="app.panel.web.find.field"
          />
          {finding !== '' && side.found !== null && (
            <span className="pn-webline-t">{side.found.n === 0 ? panel.web_no_match : panel.web_found({ at: side.found.at, n: side.found.n })}</span>
          )}
        </div>
      )}
      {side.saved !== null && (
        <div className="pn-webline" data-testid="app.panel.web.saved">
          <span className="pn-webline-t">{panel.web_saved({ name: side.saved.name, folder: side.saved.folder })}</span>
          <Press
            look="pn-webline-act"
            onClick={() => {
              if (side.saved !== null) void bise?.showInFinder(side.saved.path);
              dropSaved(key);
            }}
            data-testid="app.panel.web.saved.show"
          >
            {panel.web_show}
          </Press>
        </div>
      )}
      {s?.failed === true && (
        <div className="pn-webline" data-testid="app.panel.web.failed">
          <span className="pn-webline-t">{panel.web_failed}</span>
          <Press look="pn-webline-act" onClick={() => { web?.act(key, 'reload'); }} data-testid="app.panel.web.failed.retry">
            {panel.web_retry}
          </Press>
        </div>
      )}
      {listing ? (
        <div className="pn-launch" role="listbox" id={`${tabId}.list`} aria-label={panel.addr_enter}>
          <OmniList omni={omni} prefix="app.panel.web" optId={optId} />
        </div>
      ) : (
        <div ref={box} className="pn-page pn-webpage" data-testid="app.panel.web.page" />
      )}
      {web === undefined && <p className="pn-none">{shownUrl(url)}</p>}
    </div>
  );
}
