// A thread's agent's links in the window's bar, right after its name (designer m_57254): its open PR
// ('#75286', its tab; ⌘: the forge's page), its worktree's branch ('ψ sb/perf', the branch screen),
// its newest page (its tab). Each a 24 high link, t-12 text-2, its text 8 from the next one's. No
// room: the subtitle gives way first (cut with …, then out), then the links whole, last first (page,
// then branch; the PR last): giveway.ts titleFit, measured here; one that gave way leaves the
// keyboard's order too (designer m_57304). The words and
// what each opens: agentlinks.ts. Owner: nm-frame.
import { useEffect, useRef, type MouseEvent, type RefObject } from 'react';
import { AppIcon, Press, tipAttrs } from '@/ds';
import { panel } from '@/proto/words_panel.gen';
import { useAgents, useArtifacts, useCards, usePlaces } from '@/store';
import { openPr } from '@/review/openPr';
import { agentLinksOf, noLinks } from './agentlinks';
import { titleFit } from './giveway';
import { useRouter } from './router';
import { openTab } from './panel/tabs';

/** An element's width with the gap before it (its own left margin, or `gap`). */
const withGap = (e: HTMLElement, gap: number): number => e.offsetWidth + gap;

/**
 * The title row's give-way (giveway.ts titleFit): reads what the title can take (its content box and
 * the status slot's slack) and each part's whole width, kept from when it last showed; writes the
 * title's data-sub ('cut', 'out') and each link that gave way's data-out (display none: out of the
 * keyboard's order too). Its writes are attributes the bar's observers never watch: no loop.
 */
function useTitleFit(box: RefObject<HTMLElement | null>): void {
  useEffect(() => {
    const el = box.current;
    const title = el?.closest<HTMLElement>('.app-bar-title') ?? null;
    const bar = title?.parentElement ?? null;
    if (el === null || title === null || bar === null) return;
    const kept = new Map<string, number>();
    let subW = 0;
    const read = () => {
      const st = getComputedStyle(title);
      const gap = parseFloat(st.columnGap || '0');
      const links = [...el.children] as HTMLElement[];
      // the first link sits the box's margin nearer the name; the others their own margin apart
      const lead = gap + parseFloat(getComputedStyle(el).marginLeft || '0');
      const ws = links.map((l, i) => {
        const key = l.dataset['testid'] ?? String(i);
        if (l.dataset['out'] === undefined) kept.set(key, withGap(l, parseFloat(getComputedStyle(l).marginLeft || '0')));
        return kept.get(key) ?? 0;
      });
      if (ws.length > 0) ws[0] = (ws[0] ?? 0) + lead;
      const sub = title.querySelector<HTMLElement>('.s');
      if (sub === null) subW = 0;
      else if (title.dataset['sub'] !== 'out') subW = sub.scrollWidth + gap;
      const name = title.querySelector<HTMLElement>('.t');
      const fact = title.querySelector<HTMLElement>('.app-bar-fact');
      const fixed = (name?.offsetWidth ?? 0) + (fact === null || fact.offsetWidth === 0 ? 0 : withGap(fact, gap));
      const slot = bar.querySelector<HTMLElement>(':scope > .app-title-status');
      const slack = slot === null ? 0 : Math.max(0, slot.clientWidth - [...slot.children].reduce((n, c) => n + (c as HTMLElement).offsetWidth, 0));
      const room = title.clientWidth - parseFloat(st.paddingLeft || '0') - parseFloat(st.paddingRight || '0') + slack;
      const fit = titleFit({ room, fixed, links: ws, sub: subW });
      if (fit.sub === 'whole') delete title.dataset['sub'];
      else title.dataset['sub'] = fit.sub;
      links.forEach((l, i) => {
        if (i < fit.links) delete l.dataset['out'];
        else l.dataset['out'] = '';
      });
    };
    read();
    const ro = new ResizeObserver(read);
    ro.observe(bar);
    ro.observe(title);
    const mo = new MutationObserver(read);
    mo.observe(title, { childList: true, subtree: true, characterData: true });
    return () => {
      ro.disconnect();
      mo.disconnect();
      delete title.dataset['sub'];
    };
  }, [box]);
}

export function BarLinks({ project, agent }: { project: string; agent: string }) {
  const router = useRouter();
  const a = useAgents(project).find((x) => x.name === agent);
  const links = agentLinksOf(a, usePlaces(project), useCards(project), useArtifacts(project));
  const box = useRef<HTMLSpanElement>(null);
  useTitleFit(box);
  if (noLinks(links)) return null;
  const { pr, branch, page } = links;
  return (
    <span ref={box} className="app-bar-links" data-testid="app.bar.links">
      {pr !== null && (
        <Press look="app-bar-link" data-mono="" data-tone={pr.tone} aria-label={pr.tip} {...tipAttrs(pr.tip)} onClick={(e: MouseEvent) => {
          openPr(project, pr.url, e);
        }} data-testid="app.bar.links.pr">
          {pr.fails ? <span>✗</span> : <AppIcon name="branch" size={14} />}#{String(pr.n)}{pr.merge && <span className="app-bar-link-said"> · {panel.pr_your_merge}</span>}
        </Press>
      )}
      {branch !== null && (
        <Press look="app-bar-link" data-mono="" aria-label={branch.tip} {...tipAttrs(branch.tip)} disabled={branch.branch === null} onClick={() => {
          if (branch.branch !== null) router.go({ screen: 'branch', project, branch: branch.branch });
        }} data-testid="app.bar.links.branch">
          ψ {branch.label}
        </Press>
      )}
      {page !== null && (
        <Press look="app-bar-link" data-page="" aria-label={page.title} {...tipAttrs(page.title)} onClick={() => {
          openTab({ kind: 'artifact', project, id: page.id });
        }} data-testid="app.bar.links.page">
          <AppIcon name="file" size={14} /><span className="app-bar-link-t">{page.title}</span>
        </Press>
      )}
    </span>
  );
}
