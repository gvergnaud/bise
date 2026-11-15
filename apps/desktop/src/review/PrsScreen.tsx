// A project's open pull requests (bise fait mac, page native-mac §9; bar A.7, the TUI's /prs): under
// the window's bar its banner (ds PageBanner: 'Pull requests', its sentence, '2 PRs open', the
// signed vignette), then one ds ListRow per PR off its places (prs.ts openPrs, the source the right
// bar and the PR tab read): its tone's dot, '#412' and its branch, then its
// agents and its state in words (changes asked, checks failing and which ones, review); a click
// opens it in his browser, its '…' and a right click are its menu (useEntityMenu). Empty: the hub's
// own words ('no open PR. in PR flow…'). The floating composer's room at the foot (--float-room).
// Owner: nm-pages-b (amb-win's before).
import { useRef } from 'react';
import { usePageColumn } from '@/app/pagecol';
import { Dot, Empty, ListRow, MoreButton, PageBanner } from '@/ds';
import { hero } from '@/proto/words_hero.gen';
import { pages } from '@/proto/words_pages.gen';
import { usePlaces } from '@/store';
import type { Pr } from '@/proto';
import { PageVignette, useBannerGutter } from '@/artifacts/hero/vignettes';
import { useEntityMenu } from '@/app/menus/use';
import { agentsOf, openPrs, toneOf, wordsOf, type PrTone } from './prs';
import { useOpenPr } from './openPr';
import './review.css';
import { useKeptScroll } from '@/app/keep';

const DOT: Record<PrTone, 'er' | 'nd' | 'wk' | 'ok' | 'off'> = { err: 'er', nd: 'nd', wk: 'wk', ok: 'ok', rest: 'off' };

export function PrsScreen({ project }: { project: string }) {
  useKeptScroll(`prs:${project}`, () => document.querySelector<HTMLElement>('[data-testid="prs.scroll"]'));
  const gutter = useBannerGutter();
  const screen = useRef<HTMLElement>(null);
  const col = usePageColumn(screen);
  // its places' PRs, live: the one source the right bar and the PR tab read (the hub's poller
  // keeps them fresh, so opening the screen asks nothing)
  const list = openPrs(usePlaces(project));

  return (
    <section ref={screen} style={col} className="rv-screen" data-testid="prs.screen" data-project={project}>
      <div className="rv-scroll" data-testid="prs.scroll">
        <PageBanner title={hero.prs_title} sentence={hero.prs_say}
          facts={list.length === 0 ? hero.none_open : pages.prs_open({ n: list.length })}
          visual={<PageVignette page="prs" />} icon="branch" gutter={gutter} testId="prs.lead" />
        <div className="rv-col">
          {list.length === 0 && (
            // one sentence, never the banner's again (designer m_43933 #7)
            <Empty testId="prs.empty" title={<span data-testid="prs.none">{pages.no_pr}</span>} />
          )}
          {list.length > 0 && (
            <div data-testid="prs.list">
              {list.map((p) => <PrRow key={p.number} project={project} p={p} />)}
            </div>
          )}
        </div>
      </div>
    </section>
  );
}

function PrRow({ project, p }: { project: string; p: Pr }) {
  const openPr = useOpenPr();
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
