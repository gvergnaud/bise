// W6/K18: an agent's feed in place, without entering its thread (the TUI's
// space on a panel selection, sb.rs draw_sb preview). Its last entries,
// read only, drawn by the thread's own rows; the thread is followed while
// the preview is open. Owner: amb-win.
import { openLinkMenu, openWebLink } from '@/app/panel/web';
import { useEffect, useMemo } from 'react';
import { threadKey, useStore, useThreadOrder } from '@/store';
import { EntryRow, type RowCtx } from '@/thread/rows';
import { LinkMenuContext } from '@/thread/linkmenu';
import { useGoArtifact } from '@/artifacts/leave';
import '@/thread/thread.css';

/** A link's right click in the peek: its menu, as in the thread (web.ts linkMenu, links-os). */
function showLinkMenu(url: string): void {
  void openLinkMenu(url);
}

/** How many of its last entries the preview shows. */
export const PEEK_ROWS = 8;

/** The last `n` positions of a thread's order. Pure. */
export function lastOf(order: readonly number[], n = PEEK_ROWS): number[] {
  return order.slice(Math.max(0, order.length - n));
}

export function Peek({ project, agent }: { project: string; agent: string }) {
  const store = useStore();
  const tkey = threadKey(project, agent);
  useEffect(() => {
    store.subscribeThread(project, agent, 20);
    return () => {
      store.unsubscribeThread(project, agent);
    };
  }, [store, project, agent]);
  const { order } = useThreadOrder(tkey);
  const goArtifact = useGoArtifact();
  const ctx = useMemo<RowCtx>(
    () => ({
      tkey,
      project,
      agent,
      bise: false,
      onLink: (url, how) => {
        openWebLink(url, how ?? 'plain');
      },
      onArtifact: (id) => {
        goArtifact(project, id);
      },
    }),
    [tkey, project, agent, goArtifact],
  );
  const rows = lastOf(order);
  return (
    <div className="ag-peek" data-testid={`agents.peek.${agent}`} aria-label={`${agent}'s feed`}>
      {rows.length === 0 ? (
        <p className="ag-peek-none">nothing in its feed yet</p>
      ) : (
        <LinkMenuContext.Provider value={showLinkMenu}>
          {rows.map((pos) => (
            <EntryRow key={pos} ctx={ctx} pos={pos} />
          ))}
        </LinkMenuContext.Provider>
      )}
      <p className="ag-peek-keys">space closes · ⏎ opens its thread</p>
    </div>
  );
}
