// src/shared · Entity: a thing the desktop shows that he can open in a tab, right click, or name in
// a menu (bise fait mac, plan native-mac-plan v4 §3). One type for every place that hands a thing
// to another chunk: openTab(e) (app/panel/tabs.ts, nm-panel), useEntityMenu(e) (app/menus/use.ts,
// nm-menus), the right side's cards (nm-side), the pages' rows (nm-pages-a/-b). Each kind carries
// the ids that find it, the same as its route's (app/route.ts) and the hub's rows: no words, no
// order, no state in it (what a menu shows is decided from the hub's typed state, never from
// here). Pure types and one pure key. Owner: nm-ds.
//
// The ids (architect m_42089):
// - `project` is always the rows' hub id, never '' for bise's own: the home hub has an id like any
//   project, and an artifact belongs to one project's store (the shared library's too), so its
//   project is that store's hub id. Two lists keyed two ways break where they meet (art-tui).
// - a display name never goes in an Entity.
// - `agent` is the agent's name (rows::Agent has no other stable id): a rename changes its key, so
//   whoever keeps entities (nm-panel's kept tabs, nm-frame's pins) drops or re-keys an entry whose
//   agent is no longer in the rows, and never shows a dead tab.

/** One thing he can open, menu or tab. */
export type Entity =
  /** an agent of a project by its name ('main' is the project's lead); a rename changes its key */
  | { readonly kind: 'agent'; readonly project: string; readonly agent: string }
  /** a project by its hub id (the home hub's too, never '') */
  | { readonly kind: 'project'; readonly project: string }
  /** a project's bims office (the world's click opens it in a tab: nm-side m_42152) */
  | { readonly kind: 'office'; readonly project: string }
  /** an artifact by its id, in its store's project (the hub id, the shared library's too) */
  | { readonly kind: 'artifact'; readonly project: string; readonly id: string }
  /**
   * a pull request the project's hub follows, by its link in the hub's one form (forge::pr_link:
   * lowercased, `https://github.com/o/r/pull/412`): a number alone is ambiguous across repos (a PR
   * an agent only reviews may live in another one)
   */
  | { readonly kind: 'pr'; readonly project: string; readonly url: string }
  /** a commit of a project, by its sha */
  | { readonly kind: 'commit'; readonly project: string; readonly sha: string }
  /** a branch's diff against its trunk, by its name ('sb/x', slashes kept): a try card's diff, '/diff sb/x' */
  | { readonly kind: 'branch'; readonly project: string; readonly branch: string }
  /**
   * an agent's changes, or one file of them (`path`, from the repo's root); `since`: only its commits
   * past that sha (its open PR's head: the PR panel's 'show them', page pr-guided V2)
   */
  | { readonly kind: 'diff'; readonly project: string; readonly agent: string; readonly path?: string; readonly since?: string }
  /** a process an agent left running (running/: its id in the agent's list) */
  | { readonly kind: 'process'; readonly project: string; readonly agent: string; readonly id: string }
  /** a plugin by its id */
  | { readonly kind: 'plugin'; readonly id: string }
  /** a plugin's skill by its plugin's id and its own name (the skill tab, nm-pages-b) */
  | { readonly kind: 'skill'; readonly plugin: string; readonly name: string }
  /** a scheduled task by its hub's id (rows::ScheduledTask.id: ids are per hub, nm-menus m_42167) */
  | { readonly kind: 'scheduled'; readonly project: string; readonly id: number }
  /** an MCP App's view: the agent's tool call that showed it (its C2 call id), in its panel tab (src/views) */
  | { readonly kind: 'view'; readonly project: string; readonly agent: string; readonly call: string }
  /** a tab of the panel itself (its own menu: close, close others, move...) */
  | { readonly kind: 'tab'; readonly id: string };

/** Every kind, in the order above. */
export type EntityKind = Entity['kind'];

/**
 * One string per entity, the same for the same thing: a tab's id, a map's key, a test id's last
 * part ('agent:acme/perf', 'pr:acme/12', 'scheduled:acme/7'). Pure.
 */
export function entityKey(e: Entity): string {
  switch (e.kind) {
    case 'agent':
      return `agent:${e.project}/${e.agent}`;
    case 'project':
      return `project:${e.project}`;
    case 'office':
      return `office:${e.project}`;
    case 'artifact':
      return `artifact:${e.project}/${e.id}`;
    case 'pr':
      return `pr:${e.project}/${e.url}`;
    case 'commit':
      return `commit:${e.project}/${e.sha}`;
    case 'branch':
      return `branch:${e.project}/${e.branch}`;
    case 'diff':
      if (e.since !== undefined) return `diff:${e.project}/${e.agent}@${e.since}`;
      return e.path === undefined ? `diff:${e.project}/${e.agent}` : `diff:${e.project}/${e.agent}/${e.path}`;
    case 'process':
      return `process:${e.project}/${e.agent}/${e.id}`;
    case 'plugin':
      return `plugin:${e.id}`;
    case 'skill':
      return `skill:${e.plugin}/${e.name}`;
    case 'scheduled':
      return `scheduled:${e.project}/${String(e.id)}`;
    case 'view':
      return `view:${e.project}/${e.agent}/${e.call}`;
    case 'tab':
      return `tab:${e.id}`;
  }
}
