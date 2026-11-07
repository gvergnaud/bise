// The hub's plain line to this window (bar I9/C46/S30): a typed command's
// result, /restart's words, as one quiet row at the thread's end. The
// yes/no strip that /archive's question had here went (drop-card, architect
// m_37339): his /archive asks with the inbox's 1/2 card. Owner: amb-home.
import { useContext, useEffect, useState } from 'react';
import { formatRoute } from '@/app/route';
import { RouterContext } from '@/app/router';
import { inbox } from '@/proto/words_inbox.gen';
import { Act, Kbd } from '@/ds';
import { useFlow, useNotice, useStore, useVersions } from '@/store';
import { appBuild, appSwap, restartTail, tailWords, type AppBuild, type RestartTail } from './restart';
import './inbox.css';

/**
 * The hub's last plain line to this window for the project (a typed command's result: /flow,
 * /help, a no's '@x stays: nothing archived', /restart's words) as ONE quiet row at the thread's end
 * (designer's desktop-notices page (1)): the thread's notice look, dim, no mark. ThreadScreen
 * mounts it after the working row (amb-win m_20575). A refused line is the composer's key line.
 */
/** /flow's answer, the hub's devflow::show words ('flow: lands on main · …'). Pure. */
export function isFlowNote(text: string): boolean {
  return text.startsWith('flow: ');
}

export function HubNoteRow({ project }: { project: string }) {
  const notice = useNotice(project);
  const tail = useRestartTail(project, notice?.restart === true);
  const flow = useFlow(project);
  if (notice === null || notice.level === 'error') return null;
  // /flow's answer ('flow: lands on main · …', the hub's devflow::show): the composer's strip says
  // where the work lands once the flow is known, never a loose line above the dock (designer m_43996)
  if (isFlowNote(notice.text) && (flow === 'trunk' || flow === 'pr')) return null;
  // the hub is back (its hello): a restart's words are over; only a staged app still shows
  if (notice.back === true && tail?.kind !== 'staged') return null;
  const words = notice.back === true ? '' : notice.text;
  return (
    <div className="th-notice info th-hubnote" role="status" data-level="info" data-testid="confirm.notice" data-app={tail?.kind}>
      {/* no mark and no empty mark column: its words on the column's left edge, as the thread's
          text above it (designer m_55413) */}
      <span>
        {words}
        {tail !== null && (
          <span data-testid="confirm.notice.app">
            {words === '' ? '' : ' · '}
            {tailWords(tail)}
          </span>
        )}
        {tail?.kind === 'staged' && <SwapAct />}
      </span>
    </div>
  );
}

/** The restart row's app part: this app's build against the hub's version, read once per restart line. */
function useRestartTail(project: string, restart: boolean): RestartTail | null {
  const store = useStore();
  const hub = useVersions(project)?.current ?? null;
  const [app, setApp] = useState<AppBuild | null>(null);
  useEffect(() => {
    if (!restart) return;
    let live = true;
    void appBuild().then((b) => {
      if (live) setApp(b);
    });
    // the hub's version: the versions' current, asked when this window never had them
    if (store.versions(project) === null) store.askVersions(project);
    return () => {
      live = false;
    };
  }, [restart, project, store]);
  return restart ? restartTail(app, hub) : null;
}

/** '⏎ restart to update' (the settings update row's words, designer m_22982): the staged build in place of this one, here again (amb-mac's swap). */
function SwapAct() {
  // the screen he is on, read at his click (the swapped app reopens it)
  const router = useContext(RouterContext);
  const [why, setWhy] = useState<string | null>(null);
  const swap = () => {
    const at = router?.get();
    void appSwap(at === undefined ? undefined : formatRoute(at)).then(setWhy);
  };
  return (
    <>
      {' '}
      <Act onClick={swap} testId="confirm.notice.swap">
        <Kbd>⏎</Kbd> {inbox.restart_update}
      </Act>
      {why !== null && <span className="th-hubnote-why"> · {why}</span>}
    </>
  );
}
