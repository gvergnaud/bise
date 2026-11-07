// A project's open pull requests (bise fait mac, page native-mac §9; bar A.7, the TUI's /prs): under
// the window's bar its banner (ds PageBanner: 'Pull requests', its sentence, GitHub's head line, the
// signed vignette), then one ds ListRow per PR: its tone's dot, '#412' and its branch, then its
// agents and its state in words (changes asked, checks failing and which ones, review); a click
// opens it in his browser, its '…' and a right click are its menu (useEntityMenu). Empty: the hub's
// own words ('no open PR. in PR flow…'). The floating composer's room at the foot (--float-room).
// Owner: nm-pages-b (amb-win's before).
import { useEffect, useRef } from 'react';
import { usePageColumn } from '@/app/pagecol';
import { Dot, Empty, ListRow, MoreButton, PageBanner, Skel } from '@/ds';
import { hero } from '@/proto/words_hero.gen';
import { pages } from '@/proto/words_pages.gen';
import { usePrs, useStore } from '@/store';
import type { Pr } from '@/proto';
import { PageVignette, useBannerGutter } from '@/artifacts/hero/vignettes';
import { useEntityMenu } from '@/app/menus/use';
import { agentsOf, toneOf, wordsOf, type PrTone } from './prs';
import { openPr } from './openPr';
import './review.css';
import { useKeptScroll } from '@/app/keep';

const DOT: Record<PrTone, 'er' | 'nd' | 'wk' | 'ok' | 'off'> = { err: 'er', nd: 'nd', wk: 'wk', ok: 'ok', rest: 'off' };

export function PrsScreen({ project }: { project: string }) {
  useKeptScroll(`prs:${project}`, () => document.querySelector<HTMLElement>('[data-testid="prs.scroll"]'));
  const store = useStore();
  const gutter = useBannerGutter();
  const screen = useRef<HTMLElement>(null);
  const col = usePageColumn(screen);
  const list = usePrs(project);

  // a fresh list on open
  useEffect(() => {
    store.command({ cmd: 'prs', project });
  }, [store, project]);

  return (
    <section ref={screen} style={col} className="rv-screen" data-testid="prs.screen" data-project={project}>
      <div className="rv-scroll" data-testid="prs.scroll">
        <PageBanner title={hero.prs_title} sentence={hero.prs_say}
          facts={list === null ? hero.asking_github : list.head === '' ? hero.none_open : list.head}
          visual={<PageVignette page="prs" />} icon="branch" gutter={gutter} testId="prs.lead" />
        <div className="rv-col">
          {list === null && <Skel />}
          {list !== null && list.items.length === 0 && (
            // one sentence, never the banner's again (designer m_43933 #7)
            <Empty testId="prs.empty" title={<span data-testid="prs.none">{pages.no_pr}</span>} />
          )}
          {list !== null && list.items.length > 0 && (
            <div data-testid="prs.list">
              {list.items.map((p) => <PrRow key={p.number} project={project} p={p} />)}
            </div>
          )}
        </div>
      </div>
    </section>
  );
}

function PrRow({ project, p }: { project: string; p: Pr }) {
  const tone = toneOf(p);
  const menu = useEntityMenu({ kind: 'pr', project, url: p.url });
  const tid = `prs.row.${String(p.number)}`;
  return (
    <div data-tone={tone}>
      <ListRow testId={tid} tip={p.url} state={<Dot state={DOT[tone]} />}
        title={<><span className="pr-num">#{p.number}</span><span className="rv-mono">{p.branch}</span></>}
        sub={<>{agentsOf(p)}{' · '}<span className={`pr-words pr-${tone}`} data-testid={`prs.words.${String(p.number)}`}>{wordsOf(p)}</span></>}
        onContextMenu={menu.onContextMenu}
        end={<MoreButton menu={menu} testId={`${tid}.more`} />}
        onClick={(e) => {
          openPr(project, p.url, e);
        }} />
    </div>
  );
}
