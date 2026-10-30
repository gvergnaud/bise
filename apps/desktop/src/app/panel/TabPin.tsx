// src/app/panel · a tab's pin (tab-pin, his m_49983: « pin un artifact depuis la tab dans laquelle il
// est ouvert »; his m_50763: « cette icône seulement au hover dans la tab, au côté gauche de la
// croix »): the shared PinButton (pinbutton.tsx) inside each tab, just left of its ×, shown only while
// the pointer is on the tab (panel.css .pn-tab-pin): outline + 'Pin' or filled + 'Unpin', no pressed
// square. It sits over the title's end, which fades under it, so a tab never changes width. Its click
// writes the same prefs as the right side's rows (pins.ts pinThing), so a pin made here shows there at
// once and the other way round, and never picks the tab. On an artifact (a page too), an agent's
// thread, a process; none on Scheduled (the tab is the whole list, its rows carry their pins) nor on
// what has no pin. Shown only while its thing is in its list (a pin never names a thing gone). The
// tab's menu has Pin/Unpin too (its thing's menu, menus/registry.ts): the keyboard's way. It holds no
// list's order (no list under it). Owner: nm-panel.
import type { Entity } from '@/shared/entity';
import { useAgents, useArtifacts, useProcesses } from '@/store';
import { MAIN } from '@/app/route';
import { PinButton } from '@/app/pinbutton';
import { pinnedIn, pinRefOf, procPinId, usePins, type PinRef } from '@/app/pins';

/** The pin a tab shows for `e`: its thing's, but a Scheduled tab's (the list). Pure. */
export function tabPinOf(e: Entity): PinRef | undefined {
  return e.kind === 'scheduled' ? undefined : pinRefOf(e);
}

/** A tab's pin (module header); nothing when its thing has none or left its list. */
export function TabPin({ e, testId }: { e: Entity; testId: string }) {
  const ref = tabPinOf(e);
  const project = ref?.project ?? '';
  const agents = useAgents(project);
  const arts = useArtifacts(project);
  const procs = useProcesses(project);
  const pins = usePins(ref?.kind ?? 'artifact');
  if (ref === undefined) return null;
  // its row, by its ids: an agent by its name ('main' by its flag), the rest by their pin ids
  let id: string | undefined;
  switch (ref.kind) {
    case 'agent':
      id = agents.find((a) => (ref.id === MAIN ? a.main : a.name === ref.id))?.name;
      break;
    case 'artifact':
      id = arts.some((a) => a.id === ref.id) ? ref.id : undefined;
      break;
    case 'process':
      id = procs.some((p) => procPinId(p.agent, p.id) === ref.id) ? ref.id : undefined;
      break;
    case 'schedule':
      id = undefined;
      break;
  }
  if (id === undefined) return null;
  return (
    <PinButton
      pin={{ kind: ref.kind, project, id, on: pinnedIn(pins, project, id) }}
      look="pn-tab-pin"
      size="row"
      hold={false}
      testId={testId}
    />
  );
}
