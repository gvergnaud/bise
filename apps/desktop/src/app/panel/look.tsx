// src/app/panel · what a tab looks like in the bar and in the overflow menu: its words and its
// icon (page native-mac §6: icon 14, 13/500, 160 at most with '…'). The words come from the
// catalog (words_panel), the names from the store's rows (an artifact's title, a process's
// command, a project's name), never from the tab. Owner: nm-panel.
import type { ReactNode } from 'react';
import { AppIcon, KindIcon, iconKindOf } from '@/ds';
import { panel } from '@/proto/words_panel.gen';
import { pages } from '@/proto/words_pages.gen';
import { views } from '@/proto/words_views.gen';
import { aheadOf as aheadOfAgent } from '@/review/model';
import { useAllArtifacts } from '@/artifacts';
import { useAllProcesses } from '@/running';
import { useProjects, useStore } from '@/store';
import type { Tab } from './model';
import { viewTitle } from '@/views/Tab';
import { viewsStore } from '@/views/window';

export interface TabLook {
  readonly title: string;
  readonly icon: ReactNode;
}

/** Every tab's words and icon, from the rows they name (the bar's tabs and its overflow menu). */
export function useTabLooks(): (tab: Tab) => TabLook {
  const projects = useProjects();
  const arts = useAllArtifacts();
  const procs = useAllProcesses();
  const store = useStore();
  const branchOf = (project: string, agent: string) => store.agents(project).find((a) => a.name === agent)?.branch ?? null;
  const ahead = (project: string, agent: string) => aheadOfAgent(agent, store.agents(project), store.places(project));
  return (tab) => lookOf(tab, projects, arts, procs, branchOf, ahead);
}

function lookOf(tab: Tab, projects: ReturnType<typeof useProjects>, arts: ReturnType<typeof useAllArtifacts>, procs: ReturnType<typeof useAllProcesses>, branchOf: (project: string, agent: string) => string | null,
  aheadOf: (project: string, agent: string) => ReturnType<typeof aheadOfAgent>): TabLook {
  const what = tab.what;
  const project = what.kind === 'terminal' ? what.project : what.kind === 'thing' && 'project' in what.e ? what.e.project : undefined;
  const name = projects.find((p) => p.project === project)?.name ?? project ?? '';
  switch (what.kind) {
    case 'launcher':
      return { title: panel.tab_new, icon: <AppIcon name="browser" size={14} /> };
    case 'browser':
      return { title: panel.tab_browser, icon: <AppIcon name="browser" size={14} /> };
    case 'terminal':
      return {
        title: what.n === 0 ? panel.tab_terminal({ project: name }) : panel.tab_terminal_n({ n: what.n + 1, project: name }),
        icon: <AppIcon name="terminal" size={14} />,
      };
    case 'folder':
      // a folder of bise (FolderTab.tsx): its name at the last listing, the root 'bise'
      return { title: what.name === '' ? panel.bise : what.name, icon: <AppIcon name="folder" size={14} /> };
    case 'thing':
      break;
  }
  const e = what.e;
  switch (e.kind) {
    case 'artifact': {
      const a = arts.find((x) => x.project === e.project && x.a.id === e.id)?.a;
      return { title: a?.title ?? e.id, icon: a === undefined ? <AppIcon name="file" size={14} /> : <KindIcon kind={iconKindOf(a)} size={14} /> };
    }
    case 'agent':
      return { title: e.agent, icon: <AppIcon name="thread" size={14} /> };
    case 'diff': {
      // the tab names the title's thing (designer m_58359): its branch, as a PR's tab its '#412';
      // its commits past the PR: '2 commits not on #412' (designer m_58488)
      const ahead = e.since === undefined ? null : aheadOf(e.project, e.agent);
      if (ahead !== null && ahead.n > 0) return { title: pages.review_ahead_title({ n: ahead.n, tag: `#${String(ahead.pr.number)}` }), icon: <AppIcon name="changes" size={14} /> };
      return { title: branchOf(e.project, e.agent) ?? panel.tab_changes({ agent: e.agent }), icon: <AppIcon name="changes" size={14} /> };
    }
    case 'commit':
      return { title: e.sha.slice(0, 7), icon: <AppIcon name="branch" size={14} /> };
    case 'branch':
      return { title: e.branch, icon: <AppIcon name="branch" size={14} /> };
    case 'pr': {
      // its row's tag, the hub's one read of the link ('#412', '!412'); the plain word until it comes
      const tag = arts.find((x) => x.project === e.project && x.a.pr != null && x.a.url === e.url)?.a.pr?.tag;
      return { title: tag == null ? panel.tab_pr_plain : panel.tab_pr({ tag }), icon: <KindIcon kind="pr" size={14} /> };
    }
    case 'process': {
      const p = procs.find((x) => x.project === e.project && x.p.agent === e.agent && x.p.id === e.id)?.p;
      return { title: p?.cmd ?? e.id, icon: <AppIcon name="terminal" size={14} /> };
    }
    case 'plugin':
      return { title: e.id, icon: <AppIcon name="plugins" size={14} /> };
    case 'file': {
      // a ~/bise file: its name once its view said it, before that the name its opener knew (a
      // folder tab's row, apps-gallery m_63102); its row id is no name
      const s = viewsStore.state({ project: e.project, target: { kind: 'entry', id: e.entry } });
      const name = s.kind === 'live' && s.doc !== undefined ? s.doc.document.name : (e.name ?? '');
      return { title: name === '' ? views.doc_tab : name, icon: <AppIcon name="file" size={14} /> };
    }
    case 'view':
      // an MCP App's view: its tool's name (src/views/Tab.tsx viewTitle)
      return { title: viewTitle(e, viewsStore), icon: <AppIcon name="plugins" size={14} /> };
    case 'skill':
      // one glyph on the tab: its '$' (designer m_43933 #12), no plug icon too
      return { title: `$ ${e.name}`, icon: null };
    case 'scheduled':
      return { title: panel.tab_scheduled, icon: <AppIcon name="scheduled" size={14} /> };
    case 'project':
      return { title: name, icon: <AppIcon name="folder" size={14} /> };
    case 'office':
      return { title: panel.tab_office({ project: name }), icon: <AppIcon name="folder" size={14} /> };
    case 'tab':
      return { title: e.id, icon: null };
  }
}
