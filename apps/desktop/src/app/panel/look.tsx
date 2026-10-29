// src/app/panel · what a tab looks like in the bar and in the overflow menu: its words and its
// icon (page native-mac §6: icon 14, 13/500, 160 at most with '…'). The words come from the
// catalog (words_panel), the names from the store's rows (an artifact's title, a process's
// command, a project's name), never from the tab. Owner: nm-panel.
import type { ReactNode } from 'react';
import { AppIcon, KindIcon, iconKindOf } from '@/ds';
import { panel } from '@/proto/words_panel.gen';
import { useAllArtifacts } from '@/artifacts';
import { useAllProcesses } from '@/running';
import { useProjects } from '@/store';
import type { Tab } from './model';
import { viewTitle } from '@/views/Tab';

export interface TabLook {
  readonly title: string;
  readonly icon: ReactNode;
}

/** Every tab's words and icon, from the rows they name (the bar's tabs and its overflow menu). */
export function useTabLooks(): (tab: Tab) => TabLook {
  const projects = useProjects();
  const arts = useAllArtifacts();
  const procs = useAllProcesses();
  return (tab) => lookOf(tab, projects, arts, procs);
}

function lookOf(tab: Tab, projects: ReturnType<typeof useProjects>, arts: ReturnType<typeof useAllArtifacts>, procs: ReturnType<typeof useAllProcesses>): TabLook {
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
    case 'diff':
      return { title: panel.tab_changes({ agent: e.agent }), icon: <AppIcon name="changes" size={14} /> };
    case 'commit':
      return { title: e.sha.slice(0, 7), icon: <AppIcon name="branch" size={14} /> };
    case 'pr':
      return { title: panel.tab_pr({ number: e.number }), icon: <KindIcon kind="pr" size={14} /> };
    case 'process': {
      const p = procs.find((x) => x.project === e.project && x.p.agent === e.agent && x.p.id === e.id)?.p;
      return { title: p?.cmd ?? e.id, icon: <AppIcon name="terminal" size={14} /> };
    }
    case 'plugin':
      return { title: e.id, icon: <AppIcon name="plugins" size={14} /> };
    case 'view':
      // an MCP App's view: its tool's name (src/views/Tab.tsx viewTitle)
      return { title: viewTitle(e), icon: <AppIcon name="plugins" size={14} /> };
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
