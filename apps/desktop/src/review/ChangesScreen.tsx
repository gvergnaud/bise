// A project's changes (bise fait mac, page native-mac §9; bar A.6): under the window's bar its banner
// (ds PageBanner: 'Changes', its sentence, how many branches and merged today, the signed vignette),
// then one ds ListRow per thing in ds GroupLabels: its open pull requests (a click: the PRs page), its
// features, its branches (the agents' worktrees and his others; an agent's opens its review) and what
// reached main today (a click opens the commit read-only). A branch's and a commit's '…' and right
// click are their menu (useEntityMenu: open in a tab, a window). The floating composer's room at the
// foot (--float-room). Owner: nm-pages-b (amb-win's before).
import { useOpenChange } from './openChange';
import { useEffect, useRef } from 'react';
import { usePageColumn } from '@/app/pagecol';
import { AppIcon, Dot, Empty, GroupLabel, ListRow, MoreButton, PageBanner } from '@/ds';
import { hero } from '@/proto/words_hero.gen';
import { pages } from '@/proto/words_pages.gen';
import type { Merged, Worktree } from '@/proto';
import { useAgents, useFeatures, useMerged, usePrs, useStore, useWorktrees } from '@/store';
import { StateMark } from '@/agents/AgentsScreen';
import { PageVignette, useBannerGutter } from '@/artifacts/hero/vignettes';
import { distance, stateWords } from './feature';
import { useRouter } from '@/app/router';
import { useEntityMenu } from '@/app/menus/use';
import { clock } from '@/thread/entry';
import { branchEnd, branchesOf } from './model';
import './review.css';
import { useKeptScroll } from '@/app/keep';

export function ChangesScreen({ project }: { project: string }) {
  // where he was in the list comes back after a reload (keep.ts)
  useKeptScroll(`changes:${project}`, () => document.querySelector<HTMLElement>('[data-testid="changes.scroll"]'));
  const store = useStore();
  const router = useRouter();
  const gutter = useBannerGutter();
  const screen = useRef<HTMLElement>(null);
  const col = usePageColumn(screen);
  const worktrees = useWorktrees(project);
  const merged = useMerged(project);
  const features = useFeatures(project);
  const prs = usePrs(project);

  // fresh lists on open (the old window's f8e8330a)
  useEffect(() => {
    store.command({ cmd: 'worktrees', project });
    store.command({ cmd: 'merged', project });
    store.command({ cmd: 'features', project });
    store.command({ cmd: 'prs', project });
  }, [store, project]);

  const branches = branchesOf(worktrees);
  return (
    <section ref={screen} style={col} className="rv-screen" data-testid="changes.screen" data-project={project}>
      <div className="rv-scroll" data-testid="changes.scroll">
        <PageBanner title={hero.changes_title} sentence={hero.changes_say}
          facts={`${hero.branches_n({ n: branches.length })} · ${hero.merged_today({ n: merged.length })}`}
          visual={<PageVignette page="changes" />} icon="changes" gutter={gutter} testId="changes.lead" />
        <div className="rv-col">
          {/* its open PRs (bar A.7, the TUI's /prs): the count, the list one click away */}
          <ListRow testId="changes.prs" state={<AppIcon name="branch" size={14} />}
            title={pages.prs_row}
            sub={prs === null ? pages.prs_asking : prs.items.length === 0 ? pages.prs_none_open : prs.head}
            onClick={() => {
              router.go({ screen: 'prs', project });
            }} />
          {features.length > 0 && (
            <div role="group" aria-label={pages.group_features}>
              <GroupLabel count={features.length} testId="changes.group.features">{pages.group_features}</GroupLabel>
              {features.map((f) => (
                // a feature: its branch, its agents' work merged into it, tried and merged as one
                <ListRow key={f.name} testId={`changes.feature.${f.name}`} state={<AppIcon name="branch" size={14} />}
                  title={<span className="rv-mono">{f.name}</span>}
                  sub={`${stateWords(f)} · ${distance(f)}`}
                  onClick={() => {
                    router.go({ screen: 'feature', project, name: f.name });
                  }} />
              ))}
            </div>
          )}
          <div role="group" aria-label={pages.group_branches}>
            <GroupLabel count={branches.length} testId="changes.group.branches">{pages.group_branches}</GroupLabel>
            {branches.length === 0 ? (
              <Empty testId="changes.branches.none" title={pages.no_branches}>{pages.no_branches_why}</Empty>
            ) : (
              branches.map((w) => <BranchRow key={w.path} project={project} w={w} />)
            )}
          </div>
          <div role="group" aria-label={pages.group_merged}>
            <GroupLabel count={merged.length} testId="changes.group.merged">{pages.group_merged}</GroupLabel>
            {merged.length === 0 ? (
              <Empty testId="changes.merged.none" title={pages.nothing_merged} />
            ) : (
              merged.map((m) => <MergedRow key={m.sha} project={project} m={m} />)
            )}
          </div>
        </div>
      </div>
    </section>
  );
}

/** A branch: as the TUI names it ('sb/checkout'), then whose worktree it is and how far from main. */
function BranchRow({ project, w }: { project: string; w: Worktree }) {
  const open = useOpenChange(project);
  const agent = w.agent ?? undefined;
  const menu = useEntityMenu(agent === undefined ? { kind: 'project', project } : { kind: 'diff', project, agent });
  const tid = `changes.branch.${agent ?? 'his'}`;
  // an agent's branch shows its agent's state, the sidebar's marks (the turning arc while it works;
  // designer m_44030: one mark per meaning, '∿' only for a process); his own, whether it has changes
  const a = useAgents(project).find((x) => x.name === agent);
  return (
    <ListRow testId={tid} state={a !== undefined ? <StateMark a={a} /> : <Dot state={w.dirty ? 'wk' : 'off'} />}
      title={<span className="rv-mono">{w.branch === '' ? pages.detached : w.branch}</span>}
      sub={`${agent ?? pages.yours} · ${branchEnd(w)}`}
      onContextMenu={agent === undefined ? undefined : menu.onContextMenu}
      end={agent === undefined ? undefined : <MoreButton menu={menu} testId={`${tid}.more`} />}
      onClick={agent === undefined ? undefined : () => {
        open({ agent });
      }} />
  );
}

/** A commit that reached main today: its title, then its sha (mono), who and when; opens read-only. */
function MergedRow({ project, m }: { project: string; m: Merged }) {
  const open = useOpenChange(project);
  const menu = useEntityMenu({ kind: 'commit', project, sha: m.sha });
  const tid = `changes.merged.${m.sha.slice(0, 7)}`;
  return (
    <ListRow testId={tid} state={<AppIcon name="check" size={14} />}
      title={m.title}
      // the sha is the machine's text (mono), the rest words (designer m_21379 (4))
      sub={<><span className="rv-mono">{m.sha.slice(0, 7)}</span>{[m.by ?? '', clock(m.at_ms)].filter(Boolean).map((x) => ` · ${x}`).join('')}</>}
      onContextMenu={menu.onContextMenu}
      end={<MoreButton menu={menu} testId={`${tid}.more`} />}
      onClick={() => {
        open({ commit: m.sha });
      }} />
  );
}
