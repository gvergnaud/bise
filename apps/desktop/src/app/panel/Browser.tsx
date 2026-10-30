// src/app/panel · an artifact in a tab, the browser (page native-mac mockup 4, native-mac-qa §7): the
// panel at 640 (model.ts widthOf 'page'), an address bar (← → ↻, the address 28 r 8, ↗ his
// browser, '…' the artifact's menu) over the page (artifacts' ArtifactOpen). ← and → walk its
// versions (an artifact's history is its versions: the one before, the one after; the address
// says which one and opens the list); ↻ loads the page again. The artifact no longer
// replaces the thread in the centre
// (ArtifactView's centre and column modes are gone, rule 7): the centre keeps the thread, the
// maker's composer is the centre's. A browser tab with no page yet (the launcher's Browser, ⌘L:
// BlankBrowser) has the same bar, its address a field, focused: an artifact's name lists what it
// finds under it (↑/↓ the highlighted row, ⏎ opens it in this tab), a URL goes to his browser on ⏎
// (no web page loads in the app: main/guard.ts), esc clears it, then closes the tab. Owner: nm-panel.
import { useEffect, useMemo, useRef, useState } from 'react';
import { AppIcon, IconButton, Input, Kbd, KindIcon, Press, iconKindOf, tipAttrs } from '@/ds';
import { ds } from '@/proto/words_ds.gen';
import { panel } from '@/proto/words_panel.gen';
import { ArtifactOpen, ArtifactVersions, useAllArtifacts } from '@/artifacts';
import { browserUrl } from '@/artifacts/Open';
import { PRIMARY_KEY } from '@/artifacts/front';
import { useModHeld } from '@/app/held';
import { useEntityMenu } from '@/app/menus/use';
import { closeTabId, openWhat, takeOpened } from './state';
import { launchEnter, launchMatches, stepAt } from './launch';
import { PickRow } from './Launcher';
import { isFrameEsc } from '@/app/ArtifactView';
import { frameEsc } from '@/app/escape';

/** An artifact's tab: its address bar and its page. */
export function Browser({ project, id, tabId }: { project: string; id: string; tabId: string }) {
  const a = useAllArtifacts(project).find((x) => x.a.id === id)?.a;
  const menu = useEntityMenu({ kind: 'artifact', project, id });
  // ↻: the page mounts again (its frame loads it again)
  const [load, setLoad] = useState(0);
  // the address is the version picker (the head's 'page · v2 ⌄' it replaces)
  const [version, setVersion] = useState<number | undefined>(undefined);
  const [picking, setPicking] = useState(false);
  const [now] = useState(() => Date.now());
  const pill = useRef<HTMLButtonElement>(null);
  const bise = (window as Partial<Window>).bise;
  const shown = version ?? a?.version ?? 0;
  // ← the version before, → the one after; the newest is no pick (the page follows new versions)
  const step = (d: 1 | -1) => {
    if (a === undefined) return;
    const n = Math.min(a.version, Math.max(1, shown + d));
    setVersion(n === a.version ? undefined : n);
  };
  // an esc the page didn't use (its frame forwards it, guard.ts ESC_FORWARD): as the window's esc,
  // a zoomed page back to fit first, else the tab closes, back where he was (app/escape.ts frameEsc)
  useEffect(() => {
    const on = (e: MessageEvent) => {
      if (!isFrameEsc(e.data)) return;
      const mine = document.querySelector<HTMLIFrameElement>(`[data-testid="app.panel.browser"][data-artifact="${CSS.escape(id)}"] iframe.art-paper`);
      if (mine !== null && mine.contentWindow === e.source) frameEsc(tabId);
    };
    window.addEventListener('message', on);
    return () => {
      window.removeEventListener('message', on);
    };
  }, [id, tabId]);
  const address = a === undefined ? id : `${a.kind} · ${a.title} v${String(shown)}`;
  // ↗: what the page's head would open; ⌘ held, its ⌘⏎ keycap in place of the glyph (app/held.ts)
  const out = a === undefined ? null : browserUrl(a, shown);
  const cmd = useModHeld('Meta');
  return (
    <div className="pn-browser" data-testid="app.panel.browser" data-artifact={id}>
      <div className="pn-addr" data-testid="app.panel.addr">
        <IconButton label={panel.addr_back} size="bar" disabled={a === undefined || shown <= 1} onClick={() => { step(-1); }} testId="app.panel.addr.back">
          <AppIcon name="back" size={16} />
        </IconButton>
        <IconButton label={panel.addr_forward} size="bar" disabled={a === undefined || shown >= a.version} onClick={() => { step(1); }} testId="app.panel.addr.forward">
          <AppIcon name="forward" size={16} />
        </IconButton>
        <IconButton
          label={panel.addr_reload}
          size="bar"
          onClick={() => {
            setLoad((n) => n + 1);
          }}
          testId="app.panel.addr.reload"
        >
          <AppIcon name="refresh" size={16} />
        </IconButton>
        <Press
          ref={pill}
          look="pn-url"
          aria-haspopup="menu"
          aria-label={address}
          {...tipAttrs(a?.url ?? address)}
          disabled={a === undefined}
          onClick={() => {
            setPicking((x) => !x);
          }}
          data-testid="app.panel.addr.url"
        >
          {a === undefined ? <AppIcon name="file" size={14} /> : <KindIcon kind={iconKindOf(a)} size={14} />}
          <span className="pn-url-t">{address}</span>
          <AppIcon name="down" size={14} />
        </Press>
        {picking && a !== undefined && (
          <ArtifactVersions
            a={a}
            shown={shown}
            now={now}
            anchor={pill}
            onClose={() => {
              setPicking(false);
            }}
            onPick={(n) => {
              setVersion(n === a.version ? undefined : n);
              setPicking(false);
            }}
          />
        )}
        {/* the page's head's '↗ browser', the same url (Open.tsx browserUrl: never a file path) and
            the same ⌘⏎ (artifacts/front.ts: the bare page below runs it) */}
        <IconButton
          label={panel.addr_out}
          keys={out === null ? undefined : PRIMARY_KEY}
          size="bar"
          disabled={out === null}
          onClick={() => {
            if (out !== null) void bise?.openExternal(out);
          }}
          testId="app.panel.addr.out"
        >
          {cmd && out !== null ? <Kbd>{PRIMARY_KEY}</Kbd> : <AppIcon name="open" size={16} />}
        </IconButton>
        {/* the bar's own size, like ← → ⟳ ↗ (ui-review m_42619 #6: ds' MoreButton is a row's 28) */}
        <IconButton label={ds.more_actions} size="bar" onClick={menu.open} testId="app.panel.addr.more">
          <AppIcon name="more" size={16} />
        </IconButton>
      </div>
      <div className="pn-page">
        <ArtifactOpen
          key={load}
          project={project}
          id={id}
          bare
          {...(version === undefined ? {} : { version })}
          composer={false}
          onClose={() => {
            closeTabId(tabId);
          }}
        />
      </div>
    </div>
  );
}

/** A browser tab with no page yet: the bar, its address field focused, what his words find under it. */
export function BlankBrowser({ tabId, shown }: { tabId: string; shown: boolean }) {
  const arts = useAllArtifacts();
  const [q, setQ] = useState('');
  const [at, setAt] = useState(0);
  // ↑/↓ moved the highlight: no hover tint under a resting pointer until it moves (as the launcher)
  const [keyed, setKeyed] = useState(false);
  const field = useRef<HTMLInputElement>(null);
  const found = useMemo(() => launchMatches(arts, q), [arts, q]);
  // just opened (⌘L, the launcher's Browser): the field takes the keys; never at a project switch (his m_46543)
  useEffect(() => {
    if (shown && takeOpened(tabId)) field.current?.focus();
  }, [shown, tabId]);
  const enter = () => {
    const p = launchEnter(found, at, q);
    if (p === undefined) return;
    if ('artifact' in p) openWhat({ kind: 'thing', e: { kind: 'artifact', project: p.artifact.project, id: p.artifact.a.id } }, tabId);
    else void (window as Partial<Window>).bise?.openExternal(p.url);
  };
  const optId = (i: number) => `${tabId}.opt.${String(i)}`;
  return (
    <div className="pn-browser" data-testid="app.panel.browser.blank">
      <div className="pn-addr" data-testid="app.panel.addr">
        <IconButton label={panel.addr_back} size="bar" disabled onClick={() => undefined} testId="app.panel.addr.back">
          <AppIcon name="back" size={16} />
        </IconButton>
        <IconButton label={panel.addr_forward} size="bar" disabled onClick={() => undefined} testId="app.panel.addr.forward">
          <AppIcon name="forward" size={16} />
        </IconButton>
        <label className="pn-url pn-url-field">
          <AppIcon name="search" size={14} />
          <Input
            ref={field}
            look="pn-url-in"
            value={q}
            placeholder={panel.addr_enter}
            aria-label={panel.addr_enter}
            role="combobox"
            aria-expanded={q.trim() !== ''}
            aria-controls={`${tabId}.list`}
            aria-activedescendant={found[at] === undefined ? undefined : optId(at)}
            onChange={(e) => {
              setQ(e.target.value);
              setAt(0);
            }}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                enter();
              } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
                e.preventDefault();
                setKeyed(true);
                setAt((i) => stepAt(i, found.length, e.key === 'ArrowDown' ? 1 : -1));
              } else if (e.key === 'Escape') {
                e.preventDefault();
                e.stopPropagation();
                if (q !== '') {
                  setQ('');
                  setAt(0);
                } else closeTabId(tabId);
              }
            }}
            data-testid="app.panel.addr.field"
          />
        </label>
      </div>
      <div
        className="pn-launch"
        role="listbox"
        id={`${tabId}.list`}
        aria-label={panel.addr_enter}
        data-keyed={keyed || undefined}
        onPointerMove={() => {
          setKeyed(false);
        }}
      >
        {found.map((x, i) => (
          <PickRow
            key={`${x.project}/${x.a.id}`}
            id={optId(i)}
            on={i === at}
            icon={<KindIcon kind={iconKindOf(x.a)} size={16} />}
            title={x.a.title}
            sub={panel.launch_kind_by({ kind: x.a.kind, who: x.a.agent })}
            onPick={() => {
              openWhat({ kind: 'thing', e: { kind: 'artifact', project: x.project, id: x.a.id } }, tabId);
            }}
            onHover={() => {
              setAt(i);
            }}
            testId={`app.panel.browser.found.${x.project}/${x.a.id}`}
          />
        ))}
      </div>
    </div>
  );
}
