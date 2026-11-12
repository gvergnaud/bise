// What the window's sidebar shows for a route (bise fait mac, decision d2): the settings nav on
// settings, the artifacts library ('← Projects' and its rows) on the library page, else his
// projects. The library's rows come from the library page itself (artifacts/library.tsx: the page
// publishes its column), so the sidebar shows the library only on the route that mounts that page
// (App.tsx). An artifact opened by id, from anywhere (a notification's option, an inbox line's
// artifact name, a thread's chip, a link, back), opens in a panel beside its maker's thread: the
// sidebar stays on his projects (his m_46395), never '← Projects' over an empty list (his m_46381,
// designer m_46382). Pure. Owner: notif-sidebar.
import type { Route } from './route';

export type SideShows = 'projects' | 'library' | 'settings';

/** The route mounts the artifacts library page (ArtifactsScreen): artifacts with no artifact open. */
export function libraryPage(route: Route): boolean {
  return route.screen === 'artifacts' && route.id === undefined;
}

/** What the sidebar shows under its head for this route. */
export function sideShows(route: Route): SideShows {
  if (route.screen === 'settings') return 'settings';
  return libraryPage(route) ? 'library' : 'projects';
}

/**
 * Where the library's '← Projects' goes (main m_62356): the projects view the library was entered
 * from (`entered`, the last route that showed his projects: that project or bise), never another
 * library page and never by history (his ⌘← keeps the artifacts steps: back from the library can
 * land on one). Never entered from one (a deep link): bise. Pure.
 */
export function projectsBack(entered: Route | null): Route {
  return entered !== null && sideShows(entered) === 'projects' ? entered : { screen: 'bise' };
}
