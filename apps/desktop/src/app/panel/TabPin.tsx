// src/app/panel · the front tab's pin at the tab row's end (tab-pin, his m_49983: « pin un artifact
// depuis la tab dans laquelle il est ouvert »): the shared PinButton (pinbutton.tsx) at bar size,
// always shown, outline + 'Pin' or filled + 'Unpin', no pressed square; its click writes the same
// prefs as the right side's rows (pins.ts pinThing), so a pin made here shows there at once and the
// other way round. On an artifact (a page too), an agent's thread, a process; none on Scheduled (the
// tab is the whole list, its rows carry their pins) nor on what has no pin. Shown only while its
// thing is in its list (a pin never names a thing gone). The tab's menu has Pin/Unpin too (its
// thing's menu, menus/registry.ts). It holds no list's order (no list under it). Owner: nm-panel.
import type { Entity } from '@/shared/entity';
import { useAgents, useArtifacts, useProcesses } from '@/store';
import { MAIN } from '@/app/route';
import { PinButton } from '@/app/pinbutton';
import { pinnedIn, pinRefOf, procPinId, usePins, type PinRef } from '@/app/pins';

/** The pin the tab row's end shows for `e`: its thing's, but a Scheduled tab's (the list). Pure. */
export function tabPinOf(e: Entity): PinRef | undefined {
  return e.kind === 'scheduled' ? undefined : pinRefOf(e);
}

/** The front tab's pin (module header); nothing when its thing has none or left its list. */
export function TabPin({ e }: { e: Entity }) {
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
      look="pn-pin"
      size="bar"
      hold={false}
      testId="app.panel.pin"
    />
  );
}
