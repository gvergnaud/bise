// src/app/panel · a thing in a tab (page native-mac §4 #2: « toute entité s'y ouvre »): an artifact
// in the browser (Browser.tsx), an agent's thread, its changes, a commit, a PR
// (its project's pull requests), a process's output, a plugin's page, a scheduled task (the
// scheduled list), a project's lead thread, a project's office. Each draws the screen it has in the
// centre, in the panel's width. Owner: nm-panel.
import type { Entity } from '@/shared/entity';
import { ThreadScreen } from '@/thread/ThreadScreen';
import { ReviewScreen } from '@/review/ReviewScreen';
import { PrTab } from '@/review/PrTab';
import { PluginsScreen } from '@/plugins';
import { SkillTab } from '@/plugins/SkillTab';
import { ScheduledScreen } from '@/scheduled';
import { ProcPanel } from '@/running/Panel';
import { useProjects } from '@/store';
import { MAIN } from '@/app/route';
import { Browser } from './Browser';
import { OfficeTab } from './Office';
import { ViewTab } from '@/views/Tab';
import { viewsStore } from '@/views/window';

/** The body of a thing's tab. */
export function ThingTab({ e, tabId }: { e: Exclude<Entity, { kind: 'tab' }>; tabId: string }) {
  const projects = useProjects();
  const row = 'project' in e ? projects.find((p) => p.project === e.project) : undefined;
  switch (e.kind) {
    case 'artifact':
      return <Browser project={e.project} id={e.id} tabId={tabId} />;
    case 'agent':
    case 'project': {
      const agent = e.kind === 'agent' ? e.agent : MAIN;
      return (
        <div className="pn-screen" data-testid="app.panel.thread">
          <ThreadScreen project={e.project} agent={agent} bise={row?.home === true && agent === MAIN} {...(row === undefined ? {} : { projectName: row.name })} />
        </div>
      );
    }
    case 'diff':
      return (
        <div className="pn-screen" data-testid="app.panel.diff">
          <ReviewScreen project={e.project} agent={e.agent} />
        </div>
      );
    case 'commit':
      return (
        <div className="pn-screen" data-testid="app.panel.commit">
          <ReviewScreen project={e.project} agent={MAIN} commit={e.sha} />
        </div>
      );
    case 'pr':
      return (
        <div className="pn-screen" data-testid="app.panel.pr">
          <PrTab project={e.project} url={e.url} />
        </div>
      );
    case 'process':
      return <ProcTab project={e.project} agent={e.agent} id={e.id} />;
    case 'plugin':
      return (
        <div className="pn-screen" data-testid="app.panel.plugin">
          <PluginsScreen id={e.id} />
        </div>
      );
    case 'scheduled':
      return (
        <div className="pn-screen" data-testid="app.panel.scheduled">
          <ScheduledScreen />
        </div>
      );
    case 'office':
      return <OfficeTab project={e.project} />;
    case 'view':
      // an MCP App's view, fullscreen: the layer puts its one frame over this tab (src/views)
      return <ViewTab project={e.project} agent={e.agent} call={e.call} store={viewsStore} />;
    case 'skill':
      // a plugin's skill (page skill-panel, pick A): its file, its frontmatter, its folder (nm-pages-b)
      return (
        <div className="pn-screen" data-testid="app.panel.skill">
          <SkillTab plugin={e.plugin} name={e.name} />
        </div>
      );
  }
}

/** A process: its title and state, who started it, its command, its resources, its output (running/Panel.tsx). */
function ProcTab({ project, agent, id }: { project: string; agent: string; id: string }) {
  return <ProcPanel project={project} agent={agent} id={id} />;
}
