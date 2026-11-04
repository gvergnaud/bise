// The composer's lists from the store, for the screen that mounts it (amb-win's thread): the
// project's live agents for `@` and for an agent argument, its archived ones, its open cards by
// number. Files, skills and commands come from the core once amb-core's step 4 generates their
// events (until then their lists are empty: the pickers show nothing, never a guess).
import { useEffect, useMemo } from 'react';
import type { Plugin } from '@/proto';
import { useAgents, useArtifacts, useCards, usePlugins, usePrefs, useProjects, useStore } from '@/store';
import { getPref, type Prefs } from '@/settings/prefs';
import { mapOf, mapPrefKey } from '@/bims/maps/ids';
import { readyMaps } from '@/bims/maps';
import { mapChoices } from '@/bims/maps/words';
import { prefixFirst } from './tokens';
import { saveImageFile } from './attach';
import { NO_SOURCES, type Choice, type ComposerSources, type DictatePort, type PickPlugin } from './sources';

// K4 (the TUI's attach.rs): `inView` is the agent the composer sends to; its row says its model
// and whether that model reads images (`vision`, from bise's model catalog). Images are refused
// only when the catalog says no (`vision === false`): an unknown model is never refused on a guess.
export function useComposerSources(project: string, extra: Partial<ComposerSources> = {}, inView?: string): ComposerSources {
  const agents = useAgents(project);
  // at-plugins: the project's plugins the `@` list offers (the core's row says `pickable`), asked
  // once per mount and project (the core sends a project's list only when asked or on a change)
  const pluginRows = usePlugins(project);
  const cards = useCards(project);
  const made = useArtifacts(project);
  const root = useProjects().find((p) => p.project === project)?.path;
  const store = useStore();
  // at-plugins: the core sends a project's plugins only when asked (or on a change): asked once
  // per mount and project
  useEffect(() => {
    store.command({ cmd: 'plugins', project });
  }, [store, project]);
  const map = mapOf(getPref(usePrefs() as Prefs | null, mapPrefKey(project)));
  // the mic through the store (bar I): only this window gets its words; a reload or a quit keeps the takes (dictation-keep)
  const dictate = useMemo<DictatePort>(
    () => ({
      start: () => store.dictateStart(),
      stop: (id) => {
        store.dictateStop(id);
      },
      cancel: (id) => {
        store.dictateCancel(id);
      },
      retry: (id) => {
        store.dictateRetry(id);
      },
      drop: (id) => {
        store.dictateDrop(id);
      },
      done: (id) => {
        store.dictateDone(id);
      },
      read: (id) => store.dictation(id),
      subscribe: (id, fn) => store.subscribe(`dictation:${id}`, fn),
      // his voice level from the core's level {who: 'you'} (amb-core 99b3ddd9), for the meter
      level: (fn) => {
        fn(store.level('you'));
        return store.subscribe('level:you', () => {
          fn(store.level('you'));
        });
      },
    }),
    [store],
  );
  const { files, skills, commands, saveImage, voice } = extra;
  return useMemo<ComposerSources>(() => {
    const live = agents.filter((a) => !a.archived);
    const target = inView === undefined ? undefined : agents.find((a) => a.name === inView);
    const noVision = target?.vision === false ? (target.model ?? target.name) : undefined;
    const lists = {
      // an agent argument (/archive, /rename, /stop…) never offers main (the TUI's agent_choices)
      agent: live.filter((a) => !a.main).map((a): Choice => ({ value: a.name, desc: a.title })),
      archived: agents.filter((a) => a.archived).map((a): Choice => ({ value: a.name, desc: a.title })),
      card: cards.map((c): Choice => ({ value: String(c.id), desc: `${c.agent}: ${c.question}` })),
      map: mapChoices(readyMaps(), map),
    };
    return {
      // `@` lists the others: not the agent he writes to (the TUI's filter_mentions)
      agents: live.filter((a) => a.name !== inView).map((a) => ({ name: a.name, desc: a.title })),
      plugins: pickablePlugins(pluginRows),
      artifacts: made.map((a) => ({ id: a.id, title: a.title, kind: a.kind, agent: a.agent, url: a.url, atMs: a.at_ms })),
      skills: skills ?? NO_SOURCES.skills,
      commands: commands ?? NO_SOURCES.commands,
      files: files ?? NO_SOURCES.files,
      // B8/R14: the core keeps his picks per project (ranks them first in their tier)
      filePicked: (path) => {
        store.filePicked(project, path);
      },
      // file-chips: an @ pick of a document is a chip by its absolute path
      ...(root === undefined ? {} : { root }),
      // R41: a pasted or dropped image goes to bise's image store through main, its path in the send's files
      saveImage: saveImage ?? saveImageFile,
      ...(noVision === undefined ? {} : { noVision }),
      dictate,
      ...(voice === undefined ? {} : { voice }),
      choices: (kind, query) =>
        kind === 'agent' || kind === 'archived' || kind === 'card' || kind === 'map' ? prefixFirst(lists[kind], query, (c) => c.value) : [],
    };
  }, [agents, pluginRows, cards, made, files, skills, commands, saveImage, inView, dictate, voice, project, store, root, map]);
}

/** The `@` list's plugins from the core's rows: the ones its row says `pickable` (one rule,
 *  bend_plugins' Plugin::pickable), each with its description's first sentence. Pure. */
export function pickablePlugins(rows: readonly Plugin[] | null): PickPlugin[] {
  return (rows ?? [])
    .filter((p) => p.pickable === true)
    .map((p) => {
      const waits = p.pick_waits === 'login' || p.pick_waits === 'failed' ? p.pick_waits : undefined;
      return { name: p.name, desc: firstSentence(p.what ?? ''), ...(waits === undefined ? {} : { waits }) };
    });
}

const firstSentence = (d: string): string => {
  const t = d.trim();
  const i = t.indexOf('. ');
  return i < 0 ? t : t.slice(0, i + 1);
};
