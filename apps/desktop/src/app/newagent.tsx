// 'New agent ⌘N' (bise fait mac, page native-mac mockup 1; native-lead m_42216): one action for
// every place that starts an agent: the sidebar's first row, ⌘N, the right side's agents card '+'
// (nm-side). newAgent(project) opens today's new agent sheet (agents/Tune.tsx NewAgentSheet: its
// brief, a name, its own worktree; agents/actions.ts sends the hub's `new`) over the window, for
// that project; <NewAgentHost/> (App.tsx, once) draws it. No new hub command. Owner: nm-frame.
import { useSyncExternalStore } from 'react';
import { Sheet } from '@/ds';
import { NewAgentSheet } from '@/agents/Tune';
import { frame } from '@/proto/words_frame.gen';
import { useProjects } from '@/store';
import { keep } from './keep';

// the open sheet is kept (keep.ts, a view bit), so a reload or a relaunch opens it again with his
// half-typed brief and name (app/keep-all: 'the new-agent sheet', lost when the sheet left the
// agents page for the window); read once, at its first use
const KEPT = 'newagent.asked';
let asked: string | null | undefined;
const listeners = new Set<() => void>();

function now(): string | null {
  if (asked === undefined) {
    const v = keep.view(KEPT);
    asked = typeof v === 'string' ? v : null;
  }
  return asked;
}

function set(project: string | null): void {
  if (now() === project) return;
  asked = project;
  keep.setView(KEPT, project ?? undefined);
  for (const l of listeners) l();
}

/** Opens the new agent sheet for `project` (its hub id). */
export function newAgent(project: string): void {
  set(project);
}

/** The project whose sheet is open now (null: none). */
export function useNewAgentAsked(): string | null {
  return useSyncExternalStore(
    (cb) => {
      listeners.add(cb);
      return () => {
        listeners.delete(cb);
      };
    },
    now,
  );
}

/** The sheet over the window while one is asked; esc, cancel or the new agent's row close it. */
export function NewAgentHost() {
  const project = useNewAgentAsked();
  const name = useProjects().find((p) => p.project === project)?.name ?? project ?? '';
  const close = () => {
    set(null);
  };
  return (
    <Sheet open={project !== null} onClose={close} title={frame.new_agent} sub={frame.new_agent_in({ project: name })} testId="app.newagent">
      {project !== null && <NewAgentSheet project={project} onDone={close} />}
    </Sheet>
  );
}
