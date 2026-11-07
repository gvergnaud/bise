// The Recent row on all artifacts (page art-recent, designer m_46292; Gabriel m_46271: « toujours
// une ligne des 20 derniers artifacts créés, indépendamment de leur position dans la structure de
// folder »): under the toolbar, above Folders, the 20 newest by creation across every folder, newest
// first. One sideways row of 160-wide cards (the grid's thumb, its name and 'new', then 'age ·
// folder path' or 'unfiled'); in the ☰ list, 36 px one-line chips, the folder in the tip. ‹ › on the
// row's hover scroll a page; 'all ›' opens the Recent place. It follows the scope and the kind chips
// (the caller gives it the shown items) and the caller hides it while searching. Owner:
// artifact-folders.
import { useRef, useState } from 'react';
import { Press, tipAttrs } from '@/ds';
import { library } from '@/proto/words_library.gen';
import { panel } from '@/proto/words_panel.gen';
import { frame } from '@/proto/words_frame.gen';
import { agoWords } from '@/composer/pick';
import { Thumb } from './thumbs';
import { PrWords } from './PrWords';
import { pathOf, type Library } from './folders';
import { keyOf, type Item } from './model';

/** How many the row shows. */
export const RECENT_ROW_N = 20;

/** When an artifact was made: its first version, not its last (a new version never brings it back). */
export const createdOf = (it: Item): number => it.a.created_ms ?? it.a.versions?.[0]?.at_ms ?? it.a.at_ms;

/** The row's items: the `n` newest by creation, newest first, whatever their folder. Pure. */
export function recentOf(items: readonly Item[], n = RECENT_ROW_N): Item[] {
  return [...items].sort((x, y) => createdOf(y) - createdOf(x)).slice(0, n);
}

/** A card's second line, 'age · folder path' (designer: 'docs › perf', else 'unfiled'). Pure. */
export function recentLine(lib: Library, it: Item, now: number): { age: string; folder: string } {
  const f = lib.folderOf(it);
  const folder = f === null ? library.unfiled : pathOf(lib, f).replace(' / ', ' › ');
  return { age: agoWords(createdOf(it), now).replace(/ ago$/, ''), folder };
}

export function RecentRow({ items, lib, now, list, onOpen, onAll, kind = 'recent', testId }: {
  /** 'pinned': the row of what he pinned (his m_49100), above Recent, its items as given, no 'all ›' */
  kind?: 'recent' | 'pinned';
  /** already the shown ones (scope, kind chip), any order */
  items: readonly Item[];
  lib: Library;
  now: number;
  /** the ☰ list view: one-line chips */
  list: boolean;
  onOpen: (it: Item) => void;
  onAll?: () => void;
  testId: string;
}) {
  const strip = useRef<HTMLDivElement>(null);
  // scrolled: the fade shows at the left edge too
  const [scrolled, setScrolled] = useState(false);
  // 'pinned': the ones he pinned, as given (app/pins.ts: pin order), no 'all ›'
  const pinned = kind === 'pinned';
  const row = pinned ? items : recentOf(items);
  if (row.length === 0) return null;
  const page = (dir: 1 | -1) => {
    const el = strip.current;
    if (el === null) return;
    el.scrollBy({ left: dir * Math.max(160, el.clientWidth - 48), behavior: 'smooth' });
  };
  return (
    <section className={`art-recent${list ? ' list' : ''}`} data-testid={testId} aria-label={pinned ? frame.pinned : library.recent}>
      <header className="art-recent-h">
        <span className="t">{pinned ? frame.pinned : library.recent}</span>
        <span className="n" data-testid={`${testId}.n`} {...tipAttrs(pinned ? undefined : library.recent_row_tip)}>{row.length}</span>
        <span className="sp" />
        <Press look="art-recent-pg" aria-label={library.recent_row_back} {...tipAttrs(library.recent_row_back)} onClick={() => { page(-1); }} data-testid={`${testId}.back`}>‹</Press>
        <Press look="art-recent-pg" aria-label={library.recent_row_next} {...tipAttrs(library.recent_row_next)} onClick={() => { page(1); }} data-testid={`${testId}.next`}>›</Press>
        {onAll !== undefined && <Press look="art-recent-all" {...tipAttrs(library.recent_row_all_tip)} onClick={onAll} data-testid={`${testId}.all`}>{library.recent_row_all} ›</Press>}
      </header>
      <div className="art-recent-s" ref={strip} data-testid={`${testId}.strip`} data-scrolled={scrolled || undefined}
        onScroll={(e) => { setScrolled(e.currentTarget.scrollLeft > 0); }}>
        {row.map((it) => {
          const { age, folder } = recentLine(lib, it, now);
          const k = keyOf(it);
          const name = it.a.title || it.a.id;
          return list ? (
            <Press key={k} look="art-recent-chip" onClick={() => { onOpen(it); }} data-testid={`${testId}.item.${k}`} {...tipAttrs(`${name} · ${folder}`)}>
              <span className="nm">{name}</span>
              <span className="ag">{age}</span>
            </Press>
          ) : (
            <Press key={k} look="art-recent-card" onClick={() => { onOpen(it); }} data-testid={`${testId}.item.${k}`}>
              <span className="art-card-th rc"><Thumb item={it} visible /></span>
              <span className="tt">
                <span className="nm" {...tipAttrs(name)}>{name}</span>
                {it.a.new && <span className="art-new" data-testid={`${testId}.item.${k}.new`} {...tipAttrs(library.tip_new)}>{panel.new_chip}</span>}
              </span>
              {/* a PR: its line, as its small tile and its row (designer m_54766) */}
              <span className="mt" data-testid={`${testId}.item.${k}.line`}>
                {it.a.pr != null ? <PrWords a={it.a} fit /> : <>{age} · <span className="fd">{folder}</span></>}
              </span>
            </Press>
          );
        })}
      </div>
    </section>
  );
}
