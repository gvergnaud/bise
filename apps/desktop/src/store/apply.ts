// The store's reading of one typed event from the core (HubEv | CoreEv): which slice it puts,
// which thread it merges, which pending send it settles. Split out of store.ts (the file cap);
// the Store calls it from apply(). Owner: amb-core (the store), amb-win (the split).

import {
  type BranchesList,
  type Dictation,
  diffKey,
  diffErrKey,
  artDiffKey,
  branchDiffKey,
  guideForKey,
  guideKey,
  type Ev,
  type HubLine,
  type InboxList,
  type PagesList,
  type PrsList,
  type PrActState,
  type QueuedTake,
  takeKey,
  threadKey,
  type ThreadHits,
  type ToolOut,
  toolOutKey,
  type VersionsList,
} from "./types";
import type { Store } from "./store";

/** one shared empty list, so a reader keeps the same reference */
const EMPTY: readonly never[] = [];

/** Applies one typed event to the store's slices and threads. */
export function applyTyped(s: Store, ev: Ev): void {
  switch (ev.ev) {
    case "projects":
      // the hub's rows (HubFacts), then the cards he acted on that they dropped come back to
      // nothing (machine/cards.ts)
      s.rowsCame(ev);
      return;
    case "agents":
      s.agentsCame(ev);
      // its worktrees and their PRs (the TUI's places, P4c-4a): the right bar's marks and groups
      s.put(`places:${ev.project}`, ev.places ?? EMPTY);
      return;
    case "cards":
      // nothing here reads a hub's own cards event: a project's cards are its `projects` row's
      // (the core's list of the same rows, his and bise's own by id: core/hubs.rs inbox_cards),
      // the one copy every view and count reads (inbox-sync: a copy of the last event froze once
      // the core no longer held that hub, the card stayed in its thread)
      return;
    case "thread":
      s.page(ev);
      return;
    case "entry":
      s.entryEv(ev);
      return;
    case "thread_hits":
      s.put(`hits:${threadKey(ev.project, ev.agent)}`, { rid: ev.rid, hits: ev.hits, upto: ev.upto } satisfies ThreadHits);
      return;
    case "typing": {
      const key = threadKey(ev.project, ev.agent);
      const t = s.thread(key);
      if (t.typing === ev.text) return;
      t.typing = ev.text;
      s.emit(`typing:${key}`);
      return;
    }
    case "artifacts":
      // the hubs' facts several slices read (machine/types.ts HubFacts): one copy, as it came
      s.factsCame(ev, `artifacts:${ev.project}`);
      // the library's folders, one list for every project (~/.bise/library/folders.json): each hub
      // sends it with its artifacts, the newest wins (artifacts/folders.ts)
      if (ev.folders !== undefined) s.put("library:folders", ev.folders);
      return;
    case "folders":
    case "library_folders":
      // the library's folders after an edit: a hub's answer (an agent's), the core's (his)
      s.put("library:folders", ev.folders);
      return;
    case "processes":
      // a project's processes, whole each time: running, then ended in the last 24 h (src/running)
      s.put(`processes:${ev.project}`, ev.items);
      return;
    case "proc_out":
      // a range of a process's output: read through onEvent by its terminal (running/Output.tsx), never kept
      return;
    case "scheduled":
    case "worktrees":
    case "merged":
    case "features":
    case "models":
    case "jobs":
      s.put(`${ev.ev}:${ev.project}`, ev.items);
      return;
    case "branches":
      // the answer to branches (scanned on a thread): the review's branch picker
      s.put(`branches:${ev.project}`, {
        base: ev.base,
        rows: ev.rows,
      } satisfies BranchesList);
      return;
    case "versions":
      s.put(`versions:${ev.project}`, {
        current: ev.current,
        dev: ev.dev === true,
        installed: ev.installed === true,
        items: ev.items,
      } satisfies VersionsList);
      return;
    case "flow":
      // the repo's [flow] mode; none = not set
      s.put(`flow:${ev.project}`, ev.flow ?? null);
      return;
    case "switch":
      // where the hub's version switch is (/reload, /restart): at hello and when it moved; none = no switch yet
      s.put(`switch:${ev.project}`, ev.switch ?? null);
      return;
    case "growth":
      // les bims: how much the project has done (the office's stages read it), at hello and on change
      s.put(`growth:${ev.project}`, ev.growth);
      return;
    case "bise_update":
      // update-flow: a project's own line; the window draws the core's fold (machine_update)
      return;
    case "pulse":
      // the H1 sidebar's pulse: the core folds a held hub's hub/pulse into its ProjectRow.pulse and
      // sends the rows again (core/hubs.rs emit_rows), so the sidebar reads it from useProjects;
      // no second copy here
      return;
    case "inbox_history": {
      // a page: the first one (no older held) replaces, an older one (its items older than the
      // held ones) goes after them; one record per card, newest first
      const held = (s.slices.get(`inbox:${ev.project}`) as InboxList | undefined) ?? null;
      const oldest = held?.items[held.items.length - 1]?.card;
      const older = held !== null && oldest !== undefined && ev.items.every((r) => r.card < oldest);
      s.put(`inbox:${ev.project}`, {
        items: older ? [...held.items, ...ev.items] : ev.items,
        more: ev.more,
        answered: ev.answered,
        withdrawn: ev.withdrawn,
      } satisfies InboxList);
      return;
    }
    case "inbox_recorded": {
      const held = (s.slices.get(`inbox:${ev.project}`) as InboxList | undefined) ?? null;
      if (held === null) return;
      const r = ev.record;
      const items = [r, ...held.items.filter((x) => x.card !== r.card)];
      const was = held.items.find((x) => x.card === r.card);
      const count = (st: string) => (was?.state === st ? -1 : 0) + (r.state === st ? 1 : 0);
      s.put(`inbox:${ev.project}`, {
        ...held,
        items,
        answered: held.answered + count("answered"),
        withdrawn: held.withdrawn + count("withdrawn"),
      } satisfies InboxList);
      return;
    }
    case "pages":
      s.put(`pages:${ev.project}`, {
        items: ev.items,
        overdue: ev.overdue,
      } satisfies PagesList);
      return;
    case "release":
    case "update":
      // a /release-bise run (dev builds) or /update's build: its latest step, whole each time
      s.put(`${ev.ev}:${ev.project}`, ev);
      return;
    case "prs":
      s.put(`prs:${ev.project}`, {
        head: ev.head,
        items: ev.items,
        none: ev.none ?? null,
      } satisfies PrsList);
      return;
    // a PR tab's answers: each by its req, the newest one for its key only (a closed tab's, an
    // older ask's, another window's: dropped)
    case "pr_detail":
    case "pr_check_log": {
      const key = s.prTake(ev.req);
      if (key !== undefined) s.put(key, ev);
      return;
    }
    case "pr_acted": {
      const key = s.prTake(ev.req);
      if (key === undefined) return;
      s.put(key, {
        act: ev.act,
        done: true,
        ...(ev.err == null ? {} : { err: ev.err }),
        ...(ev.why == null ? {} : { why: ev.why }),
      } satisfies PrActState);
      return;
    }
    case "tool_out": {
      const key = toolOutKey(ev.project, ev.agent, ev.pos);
      s.toolAsks.delete(key);
      s.put(key, {
        out: ev.out,
        cut: ev.cut,
        ...(ev.total == null ? {} : { total: ev.total }),
      } satisfies ToolOut);
      return;
    }
    case "queued_taken": {
      const key = takeKey(ev.project, ev.agent, ev.id);
      if (s.takeAsk.get(ev.project) === key) s.takeAsk.delete(ev.project);
      // his parts too (msg-parts): the composer gets his chips back from them (partsback.ts)
      s.put(key, { text: ev.text, ...(ev.parts == null ? {} : { parts: ev.parts }) } satisfies QueuedTake);
      return;
    }
    case "diff":
      // an answer to someone else's `req` (a terminal's /diff) never lands on this window's views
      if (ev.req != null) {
        // a PR tab's diff: its answer carries no link, its req says whose it is
        if (s.prReqs.has(ev.req)) {
          const key = s.prTake(ev.req);
          if (key === undefined) return;
          s.put(key, ev);
          if (s.slices.get(diffErrKey(key)) != null) s.put(diffErrKey(key), null);
          return;
        }
        if (!s.diffReqs.has(ev.req)) return;
        s.diffReqs.delete(ev.req);
      }
      // a .diff artifact's, by its id and version; a branch's (no agent, its head the branch asked);
      // else an agent's change or one commit
      {
        const key = ev.artifact != null ? artDiffKey(ev.project, ev.artifact, ev.version ?? 0)
          : ev.agent === '' && ev.head != null && ev.head !== '' ? branchDiffKey(ev.project, ev.head)
          : diffKey(ev.project, ev.agent, ev.commit ?? undefined);
        s.put(key, ev);
        if (s.slices.get(diffErrKey(key)) != null) s.put(diffErrKey(key), null);
      }
      return;
    // a diff's guide by its hash: the answer to `guide`, or one written for every window
    case "guide":
      s.put(guideKey(ev.project, ev.hash), ev);
      // the change it answers for (an older hub names none): its newest answer wins, a new head's too
      if (ev.target != null) s.put(guideForKey(ev.project, ev.target), ev.hash);
      return;
    case "prefs":
      s.put("prefs", ev.prefs);
      return;
    case "accounts":
      s.put("accounts", ev.items);
      s.put("claude_plan", ev.claude_plan);
      // a sign-in's end: its link is no longer open
      if (s.signing() !== null) s.put("signing", null);
      return;
    case "signing":
      s.put("signing", { id: ev.id, url: ev.url });
      return;
    case "provider_check":
      // the last check's state (provider_use): checking, works or failed
      s.put("provider_check", ev);
      return;
    case "found":
      s.put("found", ev.items);
      return;
    case "plugins":
      s.put(`plugins:${ev.project ?? ""}`, ev.items);
      return;
    case "plugins_summary":
      // the sidebar's plugins row and the place's band (bend_plugins::catalog::summary)
      s.put(`plugins_summary:${ev.project ?? ""}`, ev.summary);
      return;
    case "browse":
      // a source's plugins to add: the Mistral catalog, a marketplace (the plugins place's browse)
      s.put(`browse:${ev.source}`, ev.items);
      if (ev.source !== "mistral" && !s.browseSources().includes(ev.source)) s.put("browse_sources", [...s.browseSources(), ev.source]);
      return;
    case "plugin_preview":
      // what an add will run, until he confirms or cancels (the add sheet clears it)
      s.put("plugin_preview", { token: ev.token, preview: ev.preview });
      return;
    case "computer_use":
      s.put("computer_use", ev);
      return;
    case "computer_live":
      // S29: every agent using computer use now, whole each time
      s.put("computer_live", ev.items);
      return;
    case "computer_peek":
      // S29: the latest still of that agent replaces the last (memory only)
      s.put(`peek:${ev.key}`, ev);
      return;
    case "away_summary":
      s.put("away", ev);
      return;
    case "commands":
      s.put("commands", ev.items);
      return;
    case "roles":
      s.put("roles", ev.items);
      return;
    case "setup_gauge":
      s.put("setup_gauge", ev.gauge);
      return;
    case "skills":
      s.put(`skills:${ev.project}`, ev.items);
      return;
    case "files":
      s.filesAnswer(ev);
      return;
    case "heard":
      // fn's talk: the whole text so far, then all of it (final); the capsule's talk view draws it
      s.put("heard", { text: ev.text, final: ev.final });
      return;
    case "shot":
      // fn's talk: the still it will send (file: its stored path), null once it went or was dropped
      s.put("shot", ev.file == null ? null : { file: ev.file, id: ev.id ?? null, app: ev.app ?? null, title: ev.title ?? null });
      return;
    case "level":
      // a meter's input (~30 Hz): only who reads that voice re-renders
      if (s.slices.get(`level:${ev.who}`) !== ev.v)
        s.put(`level:${ev.who}`, ev.v);
      return;
    case "voice_answer":
      // bar N14: his talk answered a card in view; the inbox shows it, then answers
      s.put("voice_answer", ev);
      return;
    case "voice_mode":
      // only the window that turned it on gets it (main's routes); its
      // last (on: false) stays until he turns it on again
      s.put("voice_mode", ev);
      return;
    case "dictation":
      // only the window that started it gets it (main's routes); an id
      // this store never started, or one already over, is ignored
      if (!s.dictating.has(ev.id)) return;
      if (ev.final) s.dictating.delete(ev.id);
      s.put(`dictation:${ev.id}`, {
        text: ev.text,
        final: ev.final,
        cancelled: ev.cancelled ?? false,
        error: ev.error ?? null,
        ...(ev.final && ev.kept === true ? { kept: true } : {}),
        ...(ev.final && typeof ev.note === "string" && ev.note !== "" ? { note: ev.note } : {}),
      } satisfies Dictation);
      return;
    case "error":
      if (ev.cmd === "sign_in" && s.signing() !== null) s.put("signing", null);
      // a typed line the hub's router refused (HubCmd slash): his words to fix, as an error line.
      // A refused answer or close never goes on the project's line (try-gate-f2a7: it showed far
      // from where he answered and stayed after the hub took it late): the window that sent it gets
      // 'card_refused' for that very card and says so on the card (store.ts cardRefused, refused.ts)
      if (ev.cmd === "answer" || ev.cmd === "close") return;
      // a refused diff: the view that asked it says why, with a retry, never 'loading…' forever
      // (pr-changes-blank); one this window never asked is not its own
      if (ev.cmd === "diff") {
        const key = ev.project == null ? undefined : s.diffAsked.get(ev.project);
        if (key !== undefined) s.put(diffErrKey(key), ev.text);
        return;
      }
      // a refused subscribe (no such agent): no page comes, its thread's cue goes (thread-loading)
      if (ev.cmd === "subscribe" && ev.project != null) {
        s.subscribeFailed(ev.project);
        return;
      }
      // ⌘F over a whole thread on a hub without thread/find (older): find searches what is loaded
      if (ev.cmd === "find" && ev.project != null) {
        const f = s.finding.get(ev.project);
        if (f !== undefined) s.put(`hits:${f.key}`, { rid: f.rid, hits: null, upto: -1 } satisfies ThreadHits);
        return;
      }
      // a slash the hub refused, or a queued message it couldn't send now (delivered, taken, the
      // agent down): its words on the line over the composer
      if ((ev.cmd === "slash" || ev.cmd === "queued_steer") && ev.project != null) {
        s.put(`notice:${ev.project}`, {
          text: ev.text,
          level: "error",
        } satisfies HubLine);
        return;
      }
      // a queued message he took back had gone already (delivered, no longer waiting): the hub's words on its row
      if (ev.cmd === "queued_take" && ev.project != null) {
        const key = s.takeAsk.get(ev.project);
        if (key !== undefined) {
          s.takeAsk.delete(ev.project);
          s.put(key, { error: ev.text } satisfies QueuedTake);
          return;
        }
      }
      // the hub found no such call or no result yet: that project's last ask says so (asked again on the next open)
      if (ev.cmd === "tool_out" && ev.project != null) {
        const key = s.toolAsk.get(ev.project);
        if (key !== undefined) {
          s.toolAsks.delete(key);
          s.put(key, { error: ev.text } satisfies ToolOut);
        }
        return;
      }
      // G: his send's error settles its row (machine/sends.ts)
      if (ev.cmd === "send") s.sendFailed(ev);
      return;
    case "notice": {
      // the answer to his own /restart (its cid): the row says the app's version too
      const restart = ev.cid != null && s.restartCids.delete(ev.cid);
      s.put(`notice:${ev.project}`, {
        text: ev.text,
        level: "note",
        ...(restart ? { restart: true } : {}),
      } satisfies HubLine);
      return;
    }
    case "approvals":
      // bar V8/W21: the mode, the checker and the rules, whole each time
      s.put(`approvals:${ev.project}`, ev);
      return;
    case "refused":
      // a hub's own refusal of a connection: the core reads it and says
      // hub_refused; it never reaches a window as is
      return;
    case "index":
      // ⌘K's index: the projects the core doesn't hold, from their files
      s.put("index", ev.projects);
      return;
    case "setup_first":
      // R8: his /model needs that provider's setup first (the core holds the line)
      s.put("setup_first", ev);
      return;
    case "keychain":
      // N1: where his keys and sign-ins are kept, and the last /keychain move's lines
      s.put("keychain", ev);
      return;
    case "telemetry":
      // usage-metrics-plan step 6: his usage counts, as `bise telemetry show` prints them (the id too)
      s.put("telemetry", ev);
      return;
    case "machine_update":
      // update-flow: a newer bise for the machine (every project's, folded by the core)
      s.put("bise_update", ev.update ?? null);
      return;
    case "hub_refused":
      // a project's hub refused this core: its words until he retries
      s.put(`hub_refused:${ev.project}`, ev.error);
      return;
    case "broken":
      // this core can start no hub (its runtime is missing): why, for this core's life
      s.put("broken", ev.reason);
      return;
    case "welcome":
      // its hub took this core (again): no refusal holds
      if (s.slices.get(`hub_refused:${ev.project}`) != null)
        s.put(`hub_refused:${ev.project}`, null);
      // the hub's last plain line was said on the connection before this hello: a /restart's
      // 'restarting the hub …' is over once the hub says hello again, so it goes (his m_22835:
      // it stayed 40 min after the hub came back; designer m_22843: no past line). A refused line
      // (his words to fix) stays. A restart's keeps its mark: the app's staged build still shows.
      {
        const line = s.notice(ev.project);
        if (line?.level === "note" && line.back !== true)
          s.put(`notice:${ev.project}`, line.restart === true ? { ...line, back: true } : null);
      }
      return;
    case "route":
    case "route_done":
    case "job_end":
    case "followed_end":
    case "activity":
    case "open":
    case "page_changed":
    case "card_open":
    case "app_open":
    case "focused":
      // one-shot: read through onEvent (page_changed: a 'v2 published' moment before the next
      // pages; card_open: open that card in the inbox; focused: this connection's thread moved)
      return;
  }
}
