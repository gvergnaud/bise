// A thread's agent's links in the window's bar, right after its name (designer m_57254): its open PR
// ('#75286', its tab; ⌘: the forge's page), its worktree's branch ('ψ sb/perf', its change in the
// panel: review/openChange.ts),
// its newest page (its tab). Each a 24 high link, t-12 text-2, its text 8 from the next one's. No
// room: they give way with the rest of the title row (titlefit.ts, giveway.ts rowFit, designer
// m_67200): the page cut at a word then out, the branch cut with … then out, then the PR out; one
// that gave way leaves the keyboard's order too (designer m_57304). The words and what each
// opens: agentlinks.ts. Owner: nm-frame.
import type { MouseEvent } from 'react';
import { AppIcon, Press, tipAttrs, WordCut } from '@/ds';
import { panel } from '@/proto/words_panel.gen';
import { useAgents, useArtifacts, useCards, usePlaces } from '@/store';
import { useOpenPr } from '@/review/openPr';
import { agentLinksOf, noLinks } from './agentlinks';
import { useOpenChange } from '@/review/openChange';
import { openTab } from './panel/tabs';

export function BarLinks({ project, agent }: { project: string; agent: string }) {
  const open = useOpenChange(project);
  const openPr = useOpenPr();
  const a = useAgents(project).find((x) => x.name === agent);
  const links = agentLinksOf(a, usePlaces(project), useCards(project), useArtifacts(project));
  if (noLinks(links)) return null;
  const { pr, branch, page } = links;
  return (
    <span className="app-bar-links" data-testid="app.bar.links">
      {pr !== null && (
        <Press look="app-bar-link" data-mono="" data-tone={pr.tone} aria-label={pr.tip} {...tipAttrs(pr.tip)} onClick={(e: MouseEvent) => {
          openPr(project, pr.url, e);
        }} data-testid="app.bar.links.pr">
          {pr.fails ? <span>✗</span> : <AppIcon name="branch" size={14} />}#{String(pr.n)}{pr.merge && <span className="app-bar-link-said"> · {panel.pr_your_merge}</span>}
        </Press>
      )}
      {branch !== null && (
        <Press look="app-bar-link" data-mono="" aria-label={branch.tip} {...tipAttrs(branch.tip)} disabled={branch.branch === null} onClick={() => {
          if (branch.branch !== null) open({ agent });
        }} data-testid="app.bar.links.branch">
          ψ <span className="app-bar-link-x">{branch.label}</span>
        </Press>
      )}
      {page !== null && (
        <Press look="app-bar-link" data-page="" aria-label={page.title} {...tipAttrs(page.title)} onClick={() => {
          openTab({ kind: 'artifact', project, id: page.id });
        }} data-testid="app.bar.links.page">
          <AppIcon name="file" size={14} /><WordCut className="app-bar-link-t" text={page.title} />
        </Press>
      )}
    </span>
  );
}
