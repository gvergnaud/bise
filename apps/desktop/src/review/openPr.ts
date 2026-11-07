// A PR's one door (pr-integration A rule 7): its right-bar row, the agent's ⑂ chip, the thread's
// 'PR #412' line and the Pull requests screen all open its tab in the panel; ⌘-click on any of
// them opens the forge's page instead. Owner: pr-artifacts.
import type { MouseEvent } from 'react';
import { openTab } from '@/app/panel/tabs';

/** ⌘-click: the forge's page in his browser; a plain click: the PR's tab. */
export function openPr(project: string, url: string, e?: Pick<MouseEvent, 'metaKey'>): void {
  if (e?.metaKey === true) {
    void (window as Partial<Window>).bise?.openExternal(url);
    return;
  }
  openTab({ kind: 'pr', project, url });
}
