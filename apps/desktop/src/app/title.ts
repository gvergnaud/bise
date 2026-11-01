// What the window's bar and title say (bise fait mac, page native-mac §6, mockup 1): the title is
// what he looks at, 13/600 ('perf', 'inbox'), its subtitle 13/400 text-2 says where it sits
// ('profiling the hub · acme': the agent's title, then its project), and the window's own title
// (macOS's Window menu, Mission Control) is that title, never 'bise' alone but on bise's own thread.
// Built on the crumb (side.ts crumbOf, crumbGoOf): the crumb's last part is the title, the earlier
// ones, nearest first, the subtitle, each still a way back where crumbGoOf gives one. Pure.
// Owner: nm-frame.
import type { Route } from './route';
import { crumbGoOf, crumbOf, type PlaceCrumb } from './side';
import { frame } from '@/proto/words_frame.gen';
import { shortAge } from '@/agents/model';

/** One part of the subtitle: its words, and where a click goes (none: plain). */
export interface SubPart {
  readonly c: string;
  readonly go?: Route;
}

export interface BarTitle {
  readonly title: string;
  readonly sub: readonly SubPart[];
}

/**
 * The bar's title and subtitle for a route. `what`: the thread's agent's own title ('profiling the
 * hub'), first in the subtitle. `plugin`: a plugin's title from its id (crumbOf). Pure.
 */
export function barTitleOf(route: Route, name: (project: string) => string, place: PlaceCrumb = () => [], what = '', plugin?: (id: string) => string): BarTitle {
  const crumb = crumbOf(route, name, place, plugin);
  const title = crumb[crumb.length - 1] ?? '';
  const earlier = crumbGoOf(route, name, place).reverse();
  const sub: SubPart[] = [...(what === '' ? [] : [{ c: what }]), ...earlier.map((p) => (p.go === undefined ? { c: p.c } : { c: p.c, go: p.go }))];
  return { title, sub };
}

/** The subtitle's one fact on a thread (designer m_43892): what waits on him, else its work. */
export type StateFact = { readonly kind: 'waits'; readonly text: string } | { readonly kind: 'working'; readonly text: string };

/**
 * The thread's agent's state as the bar's subtitle says it, from its typed row (never its words):
 * it waits on him ('waits for you', the ask icon in the accent before it), else it works ('working
 * · 3 min', its turn's age, text-3), else nothing: idle, done, archived say nothing, nor the branch
 * (the composer's strip has it). Pure. nm-thread (a declared exception in nm-frame's file).
 */
export function stateFactOf(a: { readonly status: string; readonly waits: number; readonly since_ms: number; readonly turn_ms?: number | null; readonly archived: boolean; readonly waiting_on?: { readonly who: string } | null } | undefined, now: number): StateFact | null {
  if (a === undefined || a.archived) return null;
  if (a.waits > 0 || a.waiting_on?.who === 'you') return { kind: 'waits', text: frame.bar_waits };
  if (a.status !== 'working') return null;
  // turn_ms: how long its turn has run (a duration); none yet: since its status changed
  return { kind: 'working', text: frame.bar_working({ age: shortAge(a.turn_ms ?? now - a.since_ms) }) };
}

/** The window's title: the bar's title (macOS shows it in the Window menu and Mission Control). Pure. */
export const windowTitleOf = (t: BarTitle): string => t.title;
