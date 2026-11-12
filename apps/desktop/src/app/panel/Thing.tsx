// src/app/panel · a thing in a tab (page native-mac §4 #2: « toute entité s'y ouvre »): an artifact
// in the browser (Browser.tsx), an agent's thread, its changes, a commit, a branch's diff, a PR
// (each its guided review: ChangeTab, PrTab), a process's output, a plugin's page, a scheduled task (the
// scheduled list), a project's lead thread, a project's office, a ~/bise file drawn by its app
// (Document.tsx). Each draws the screen it has in the
// centre, in the panel's width. Owner: nm-panel.
import type { Entity } from '@/shared/entity';
import { ThreadScreen } from '@/thread/ThreadScreen';
import { ChangeTab } from '@/review/ChangeTab';
import { PrTab } from '@/review/PrTab';
import { PluginsScreen } from '@/plugins';
import { SkillTab } from '@/plugins/SkillTab';
import { ScheduledScreen } from '@/scheduled';
import { ProcPanel } from '@/running/Panel';
import { useProjects, useStore } from '@/store';
import { useAllArtifacts } from '@/artifacts';
import { MAIN } from '@/app/route';
import { Browser } from './Browser';
import { OfficeTab } from './Office';
import { ViewTab } from '@/views/Tab';
import { viewsStore } from '@/views/window';
import { DocumentTab } from './Document';

/** The body of a thing's tab. */
export function ThingTab({ e, tabId }: { e: Exclude<Entity, { kind: 'tab' }>; tabId: string }) {
  const projects = useProjects();
  const row = 'project' in e ? projects.find((p) => p.project === e.project) : undefined;
  switch (e.kind) {
    case 'artifact':
      // drawn by its app when its row names one (docs/plugin-apps.md A3, A5), else today's page
      return <ArtifactTab project={e.project} id={e.id} tabId={tabId} />;
    case 'agent':
    case 'project': {
      const agent = e.kind === 'agent' ? e.agent : MAIN;
      return (
        <div className="pn-screen" data-testid="app.panel.thread">
          <ThreadScreen project={e.project} agent={agent} bise={row?.home === true && agent === MAIN} {...(row === undefined ? {} : { projectName: row.name })} />
        </div>
      );
    }
    // a change, as a PR's tab draws its PR (page pr-guided V2: one home per change)
    case 'diff':
      return (
        <div className="pn-screen" data-testid="app.panel.diff">
          <ChangeTab project={e.project} change={e.since === undefined ? { agent: e.agent } : { agent: e.agent, since: e.since }} />
        </div>
      );
    case 'commit':
      return (
        <div className="pn-screen" data-testid="app.panel.commit">
          <ChangeTab project={e.project} change={{ commit: e.sha }} />
        </div>
      );
    case 'branch':
      return (
        <div className="pn-screen" data-testid="app.panel.branch">
          <ChangeTab project={e.project} change={{ branch: e.branch }} />
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
    case 'file':
      // a ~/bise file that is no artifact, drawn by its app (docs/plugin-apps.md A6)
      return <DocumentTab docRef={{ project: e.project, target: { kind: 'entry', id: e.entry } }} tabId={tabId} store={viewsStore} title={e.name ?? ''} rowApp="" />;
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

/**
 * An artifact's tab: its document drawn by the app its row names (the hub's pick, A2), else the
 * browser of today's page until its type's migration step deletes that path (rule 7).
 */
function ArtifactTab({ project, id, tabId }: { project: string; id: string; tabId: string }) {
  const a = useAllArtifacts(project).find((x) => x.a.id === id)?.a;
  const store = useStore();
  const opener = a?.opener;
  if (a === undefined || opener == null) return <Browser project={project} id={id} tabId={tabId} />;
  // who saved its newest version (the newer line's 'perf saved v4')
  const by = a.versions?.find((v) => v.v === a.version)?.by;
  return (
    <DocumentTab
      docRef={{ project, target: { kind: 'artifact', id } }}
      tabId={tabId}
      store={viewsStore}
      title={a.title}
      rowApp={opener.app}
      row={{ newest: a.version, gone: a.gone === true, ...(by == null ? {} : { by }) }}
      turnOn={() => {
        store.command({ cmd: 'plugin_set', project, name: opener.plugin, on: true });
      }}
    />
  );
}

/** A process: its title and state, who started it, its command, its resources, its output (running/Panel.tsx). */
function ProcTab({ project, agent, id }: { project: string; agent: string; id: string }) {
  return <ProcPanel project={project} agent={agent} id={id} />;
}
