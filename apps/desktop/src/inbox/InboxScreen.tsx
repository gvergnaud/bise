// The inbox screen as the shell mounts it (route inbox/all, the sidebar's '? inbox' row): every
// project's open cards (the same list the block above the composer shows, blockList) and the
// hub's records of what he answered or what was withdrawn (useInboxRecords), into InboxView
// (List.tsx). ⏎ on a waiting item opens it above this screen's composer (openInbox, as ⌘I); on a
// past one, the item whole in the pane beside the list; ⌘⏎ goes to his answer: the thread at that
// very entry (its record's pos: the store reads the page around it, however old) with one
// th-flash, a history step (answered-link's openAnswer, the one jump), or the page in the window
// at its block (notif-open's useOptAct). Owner: inbox-screen.
import { useCallback, useMemo } from 'react';
import { useProjects, useStore } from '@/store';
import { useRouter } from '@/app/router';
import { useKeptScroll } from '@/app/keep';
import { openInbox } from './Block';
import { useOptAct } from './optAct';
import { useWaitingList } from './waiting';
import { InboxView } from './List';
import { openAnswer } from '@/thread/answerlink';
import { useInboxRecords } from './records';
import type { Jump } from './past';

export function InboxScreen() {
  const rows = useProjects();
  const store = useStore();
  const router = useRouter();
  const records = useInboxRecords(rows);
  useKeptScroll('inbox', () => document.querySelector<HTMLElement>('[data-testid="inbox.list"]'));
  // the block's own list, bise's notifications ask included: one inbox, one number (designer m_25639)
  const cards = useWaitingList();
  const names = useMemo(() => Object.fromEntries(rows.map((r) => [r.project, r.name])), [rows]);
  const projects = useMemo(() => rows.map((r) => ({ project: r.project, name: r.name })), [rows]);
  const optAct = useOptAct();
  const jump = useCallback(
    (j: Jump) => {
      if (j.kind === 'thread') {
        openAnswer(router, { project: j.project, agent: j.agent, card: j.card, ...(j.pos === undefined ? {} : { pos: j.pos }) });
        return;
      }
      // a page answer: the page in the window beside the thread, at the block he answered (notif-open's useOptAct)
      optAct(j.project, { act: 'open', artifact: j.page, at: j.block ?? null });
    },
    [router, optAct],
  );
  return (
    <InboxView
      cards={cards}
      records={records.items}
      {...(records.more ? { onEnd: records.older } : {})}
      names={names}
      projects={projects}
      pageOf={(project, page) => {
        const a = store.artifacts(project).find((x) => x.id === page);
        return a === undefined ? undefined : { title: a.title, version: a.version };
      }}
      ports={{
        answer: (project, card) => {
          openInbox({ project, card });
        },
        jump,
        openPage: (project, page, block) => {
          optAct(project, { act: 'open', artifact: page, at: block ?? null });
        },
      }}
    />
  );
}
