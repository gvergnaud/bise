// A thread's agent's links in the window's bar, right after its name (designer m_57254): its open PR
// ('#75286', its tab; ⌘: the forge's page), its worktree's branch ('ψ sb/perf', the branch screen),
// its newest page (its tab). Each a 24 high link, t-12 text-2, its text 8 from the next one's. No
// room: they give way whole, last first (page, then branch; the PR last), after the subtitle's
// words (app.css .app-bar-links); one that gave way leaves the keyboard's order too. The words and
// what each opens: agentlinks.ts. Owner: nm-frame.
import { useEffect, useRef, type MouseEvent, type RefObject } from 'react';
import { AppIcon, Press, tipAttrs } from '@/ds';
import { panel } from '@/proto/words_panel.gen';
import { useAgents, useArtifacts, useCards, usePlaces } from '@/store';
import { openPr } from '@/review/openPr';
import { agentLinksOf, noLinks } from './agentlinks';
import { useRouter } from './router';
import { openTab } from './panel/tabs';

/** The links that gave way (wrapped out of the bar's one line) leave the tab order, the others come back. */
function useOutOfLine(box: RefObject<HTMLElement | null>): void {
  useEffect(() => {
    const el = box.current;
    if (el === null) return;
    const io = new IntersectionObserver((seen) => {
      for (const s of seen) {
        const out = s.intersectionRatio < 1;
        const t = s.target as HTMLElement;
        t.tabIndex = out ? -1 : 0;
        if (out) t.setAttribute('aria-hidden', 'true');
        else t.removeAttribute('aria-hidden');
      }
    }, { root: el, threshold: 1 });
    for (const c of el.children) io.observe(c);
    return () => {
      io.disconnect();
    };
  });
}

export function BarLinks({ project, agent }: { project: string; agent: string }) {
  const router = useRouter();
  const a = useAgents(project).find((x) => x.name === agent);
  const links = agentLinksOf(a, usePlaces(project), useCards(project), useArtifacts(project));
  const box = useRef<HTMLSpanElement>(null);
  useOutOfLine(box);
  if (noLinks(links)) return null;
  const { pr, branch, page } = links;
  return (
    <span ref={box} className="app-bar-links" data-testid="app.bar.links">
      {pr !== null && (
        <Press look="app-bar-link" data-mono="" data-tone={pr.tone} aria-label={pr.tip} {...tipAttrs(pr.tip)} onClick={(e: MouseEvent) => {
          openPr(project, pr.url, e);
        }} data-testid="app.bar.links.pr">
          {pr.fails ? '✗' : <AppIcon name="branch" size={14} />}#{String(pr.n)}{pr.merge && <span className="app-bar-link-said"> · {panel.pr_your_merge}</span>}
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
