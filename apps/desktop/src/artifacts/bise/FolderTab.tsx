// The folder tab (docs/plugin-apps.md A6): a folder of bise in a tab of the panel, listed by the
// home hub's folder/list through the window's own door (window.bise.views.call; never a view's
// bridge, apps-hub-files m_63095). Its head is the crumb, 'bise › Plugins › Boards': the parents
// text-2 (ink and underlined on hover), the folder itself in ink. Its rows as the ☰ list's: the
// icon, the name, 'Sheets · 2 h', the size text-3 at the right. A folder row opens in this tab
// (the tab takes the new folder), a file row the artifact it is the target of, else the file
// itself (apps-host's 'file' tab). It lists again when the window comes back to the front; an
// empty folder says 'empty'. The rule is folder.ts's. Owner: apps-gallery.
import { useEffect, useState } from 'react';
import { AppIcon, ListRow, Press } from '@/ds';
import { openWhat } from '@/app/panel/state';
import { openTab } from '@/app/panel/tabs';
import { ageWords } from '@/app/projectfacts';
import type { FolderList } from '@/proto/files';
import { library } from '@/proto/words_library.gen';
import { useProjects } from '@/store/hooks';
import { bytesWords } from '../thumbs/words';
import { crumbRow, folderListOf, openOf, rowLine } from './folder';
import './folder.css';

/** The hub's error code for a method it can't serve yet (views VIEW_ERR.notSupported). */
const NOT_SUPPORTED = -32034;

type Listing = { readonly k: 'loading' } | { readonly k: 'list'; readonly l: FolderList } | { readonly k: 'error'; readonly words: string };

/** Asks the home hub for a folder's rows (`entry` null: the root). */
async function listFolder(project: string, entry: string | null): Promise<Listing> {
  const views = (window as Partial<Window>).bise?.views;
  if (views === undefined) return { k: 'error', words: library.folder_not_yet };
  const a = await views.call({ cmd: 'folder_list', project, ...(entry === null ? {} : { folder: entry }) });
  if ('error' in a) return { k: 'error', words: a.error.code === NOT_SUPPORTED ? library.folder_not_yet : library.folder_failed({ why: a.error.message }) };
  const l = folderListOf(a.result);
  return l === null ? { k: 'error', words: library.folder_not_yet } : { k: 'list', l };
}

export function FolderTab({ entry, tabId }: { readonly entry: string | null; readonly tabId: string }) {
  const project = useProjects().find((p) => p.home)?.project ?? '';
  const [listing, setListing] = useState<Listing>({ k: 'loading' });
  const [now, setNow] = useState(() => Date.now());
  // listed when it opens and each time the window comes back to the front (no folder event yet)
  useEffect(() => {
    if (project === '') return undefined;
    let live = true;
    const relist = () => {
      void listFolder(project, entry).then((l) => {
        if (!live) return;
        setNow(Date.now());
        setListing(l);
      });
    };
    relist();
    window.addEventListener('focus', relist);
    return () => {
      live = false;
      window.removeEventListener('focus', relist);
    };
  }, [project, entry]);
  // a folder in place: this tab takes it (the root by null: one tab id for it)
  const goFolder = (id: string | null, name: string) => { openWhat({ kind: 'folder', entry: id, name }, tabId); };

  if (listing.k === 'loading') return <div className="bise-folder" data-testid="bise.folder" aria-busy="true" />;
  if (listing.k === 'error') {
    return <div className="bise-folder" data-testid="bise.folder"><p className="bise-folder-said" data-testid="bise.folder.said">{listing.words}</p></div>;
  }
  const l = listing.l;
  const crumb = crumbRow(l);
  return (
    <div className="bise-folder" data-testid="bise.folder">
      <nav className="bise-crumb" data-testid="bise.folder.crumb">
        {crumb.map((c, i) => (
          <span key={c.id} className="bise-crumb-part">
            {i > 0 && <span className="bise-crumb-sep" aria-hidden="true">›</span>}
            {c.current
              ? <span className="bise-crumb-here" aria-current="page" data-testid={`bise.folder.crumb.${String(i)}`}>{c.name}</span>
              : <Press look="bise-crumb-up" onClick={() => { goFolder(i === 0 ? null : c.id, c.name); }} data-testid={`bise.folder.crumb.${String(i)}`}>{c.name}</Press>}
          </span>
        ))}
      </nav>
      {l.rows.length === 0
        ? <p className="bise-folder-said" data-testid="bise.folder.empty">{library.folder_empty}</p>
        : (
          <div className="bise-folder-rows" role="list">
            {l.rows.map((r, i) => {
              const open = () => {
                const o = openOf(r, project);
                if (o.kind === 'folder') goFolder(o.entry, o.name);
                else if (o.kind === 'artifact') openTab({ kind: 'artifact', project: o.project, id: o.id });
                else openTab({ kind: 'file', project: o.project, entry: o.entry, name: o.name });
              };
              return (
                <ListRow key={r.id} testId={`bise.folder.row.${String(i)}`}
                  state={<AppIcon name={r.kind === 'folder' ? 'folder' : 'file'} size={16} />}
                  title={r.name} sub={rowLine(r, ageWords, now)}
                  end={r.kind === 'file' ? <span className="bise-folder-size">{bytesWords(r.size)}</span> : undefined}
                  onClick={open} />
              );
            })}
          </div>
        )}
    </div>
  );
}
