// The plugins place as the shell mounts it (route `plugins`): the core's rows, summary, browse and
// preview from the store, its commands through the store, the add sheets, and the band marked seen
// once it shows (the sidebar's dot goes). bise's home's list: the place is every project's.
// Owner: plug-desktop.
import { useEffect, useState } from 'react';
import { useBrowse, useBrowseSources, useFileTypes, usePlugins, usePluginsSummary, usePluginPreview, usePrefs, useProjects, useSkillsRefused, useStore } from '@/store';
import { useRouter } from '@/app/router';
import { MAIN } from '@/app/route';
import type { Plugin } from '@/proto';
import { PlaceView } from './Place';
import { PluginPage } from './PluginPage';
import { AddSheet, type AddItem } from './Add';
import { bandShows } from './model';
import { markSeen } from './seen';
import { putWords } from './try';
import { readGuardedOn } from './GuardedSwitch';

export function PluginsScreen({ kind, id }: { kind?: 'skills'; id?: string }) {
  const store = useStore();
  const router = useRouter();
  const rows = usePlugins();
  const summary = usePluginsSummary();
  const fileTypes = useFileTypes();
  const readGuarded = readGuardedOn(usePrefs());
  const skillsRefused = useSkillsRefused();
  const mistral = useBrowse('mistral');
  const sources = useBrowseSources();
  const preview = usePluginPreview();
  const home = useProjects().find((p) => p.home);
  const [sheet, setSheet] = useState<Exclude<AddItem, 'browse'> | null>(null);
  // the list each time the place opens (its answer brings the summary and Mistral's browse)
  useEffect(() => {
    store.command({ cmd: 'plugins' });
  }, [store]);
  // he saw the band: the sidebar's dot goes until something new comes
  useEffect(() => {
    if (id === undefined && bandShows(summary)) markSeen(summary);
  }, [id, summary]);
  const markets = sources.map((source) => ({ source, items: store.browse(source) ?? [] }));
  const browse = [{ source: 'mistral', items: mistral ?? [] }, ...markets];
  const send = store.command.bind(store);
  const setUpComputer = () => {
    router.go({ screen: 'settings', page: 'computer' });
  };
  if (id !== undefined) {
    const p: Plugin | undefined = (rows ?? []).find((r) => (r.id === '' ? r.name : r.id) === id) ?? browse.flatMap((b) => b.items).find((r) => r.id === id);
    if (p === undefined) return <PlaceView rows={rows} summary={summary} browse={browse} ports={{ send, open: (x) => { router.go({ screen: 'plugins', id: x }); }, setUpComputer, add: () => {} }} />;
    return (
      <PluginPage p={p} types={fileTypes} ports={{
        send,
        back: () => { router.back(); },
        setUpComputer,
        // plug-tidy's wire: 'plugins' again once it's gone (an error line if the core refuses)
        remove: (name) => {
          send({ cmd: 'plugin_remove', name });
          router.go({ screen: 'plugins' });
        },
        tryIt: (words) => {
          if (home === undefined) return;
          putWords(window.localStorage, `${home.project}/${MAIN}`, words);
          router.go({ screen: 'bise' });
        },
      }} />
    );
  }
  return (
    <>
      <PlaceView key={kind ?? 'all'} rows={rows} summary={summary} browse={browse} fileTypes={fileTypes} readGuarded={readGuarded} skillsRefused={skillsRefused} {...(kind === undefined ? {} : { kind })}
        ports={{ send, open: (x) => { router.go({ screen: 'plugins', id: x }); }, setUpComputer, add: (item) => { if (item !== 'browse') setSheet(item); } }} />
      <AddSheet item={sheet} project={undefined} summary={summary} preview={preview} ports={{ send }} onClose={() => { setSheet(null); store.put('plugin_preview', null); }} />
    </>
  );
}
