// The thread's catching-up cue (thread-loading): he opens a thread the window held from before
// (or never held), its newest page is on its way (store.threadSyncing), and the agent may have
// written since. Under the last row, over the list's bottom edge and out of the layout: nothing
// moves when it comes or goes, so a thread with nothing newer looks the same once it goes. It
// shows only after a short wait (a page in time never flashes it) and fades out (thread.css
// .th-sync). Always mounted, so it can fade. Owner: thread-loading.
// The rows of the core's copy from before (boot-speed: a hub still booting) show it at once: they
// are known to be old, so a moment without it would read as live (ui-review m_54190).
import { useThreadCopy, useThreadSyncing } from '@/store';

export function SyncCue({ tkey }: { tkey: string }) {
  const on = useThreadSyncing(tkey);
  const copy = useThreadCopy(tkey);
  return (
    <div className="th-sync" data-on={on} data-copy={copy} data-testid="thread.sync" aria-hidden={!on}>
      <span className="th-sync-dots">
        <i />
        <i />
        <i />
      </span>
    </div>
  );
}
