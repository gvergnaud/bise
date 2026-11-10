// src/composer/float · whom the floating composer writes to on a page that is no thread (page
// native-mac §6, mockups 6-8): an agent's review → that agent; a project's page → its main; a page
// of no project (inbox, scheduled, plugins, processes) → bise's main (the home hub's). None on a
// thread or bise's own view (their composer is the thread's), an open artifact (its maker's beside
// it), settings (its own window), onboarding, the lab and the debug screens. This only picks the
// default addressee shown in 'to bise ⌄': where a message to a project goes stays the hub's route
// (sb-core). The panel full width (panel-full, his m_45439): the thread is gone, so a thread and
// bise's own view have the floating composer too, to the thread's agent (an open artifact's: its
// maker, the panel's artifact in front too: FloatHost reads them). Pure. Owner: nm-composer.
import { MAIN, projectOf, type Route } from '@/app/route';

/** A floating composer's addressee: a project's hub id and an agent's name. */
export interface FloatTo {
  readonly project: string;
  readonly agent: string;
}

/**
 * The floating composer's default addressee on `r` (`home`: the home hub's id; `full`: the panel
 * is full width, the screen hidden); null: no floating composer there. Pure.
 */
export function floatTarget(r: Route, home: string | undefined, full = false): FloatTo | null {
  if (full && r.screen === 'thread') return { project: r.project, agent: r.agent };
  if (full && r.screen === 'bise') return home === undefined ? null : { project: home, agent: MAIN };
  switch (r.screen) {
    case 'thread':
    case 'bise':
    case 'settings':
    case 'onboarding':
    case 'lab':
    case 'debug':
    case 'notfound':
      return null;
    case 'artifacts':
      // an open artifact has its one composer, the maker's
      if (r.id !== undefined && r.project !== '') return null;
      break;
    default:
      break;
  }
  const p = projectOf(r);
  if (p !== undefined && p !== '') return { project: p, agent: MAIN };
  return home === undefined ? null : { project: home, agent: MAIN };
}
