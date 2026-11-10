// An artifact's route (artifacts/<project>/<id>: a link, a notification, back and forward, a deep
// link): the maker's thread in the centre, its one composer about the artifact ('to perf · about
// “…”'; the project's main when the maker is archived), and the artifact itself in a tab of the
// panel (app/panel/Browser.tsx, opened here). The artifact no longer replaces the thread in the
// centre nor takes a column of its own (bise fait mac, page native-mac §3 #6: its centre, column
// and narrow modes went with the right column, rule 7). An esc the artifact's page didn't use
// reaches the window as a message from its frame (main's guard.ts ESC_FORWARD): isFrameEsc reads
// it, the tab closes. Owner: nm-panel (bise fait mac; amb-win before).
import { useEffect } from 'react';
import { FRAME_KINDS } from '@/artifacts/frameNote';
import { useAboutWords, useAllArtifacts } from '@/artifacts';
import { centreOf } from '@/artifacts/gone';
import { useProjects, useStore } from '@/store';
import { ThreadScreen } from '@/thread/ThreadScreen';
import { MAIN, type Route } from './route';
import { openTab } from './panel/tabs';

/** The artifact this route opens in the window, if any: a project's artifact by id. Pure. */
export function artOf(route: Route): { project: string; id: string } | undefined {
  return route.screen === 'artifacts' && route.project !== '' && route.id !== undefined ? { project: route.project, id: route.id } : undefined;
}

/** The forwarder's message (main's guard.ts ESC_FORWARD). Pure. */
export function isFrameEsc(data: unknown): boolean {
  return typeof data === 'object' && data !== null && (data as { bise?: unknown }).bise === FRAME_KINDS.esc;
}

/** An artifact's route: the artifact in a panel tab (opened once per id), the maker's thread in the centre. */
export function ArtifactRoute({ project, id }: { project: string; id: string }) {
  useEffect(() => {
    // the route is the history's step already: its tab opens quietly (panel/steps.ts)
    openTab({ kind: 'artifact', project, id }, { quiet: true });
  }, [project, id]);
  return <ArtifactThread project={project} id={id} />;
}

/** The centre beside an open artifact: the maker's thread, its one composer about the artifact. */
export function ArtifactThread({ project, id }: { project: string; id: string }) {
  const it = useAllArtifacts(project).find((x) => x.a.id === id);
  const words = useAboutWords(project, id);
  // the project's name in a head, never its workspace id (designer m_20453)
  const name = useProjects().find((p) => p.project === project)?.name;
  // its list came without it: the project's main, never a blank centre (artifacts/gone.ts)
  const centre = centreOf(useStore().artifactsCame(project), it?.a);
  if (centre.c === 'main') return <ThreadScreen key={`${project}/${MAIN}`} project={project} agent={MAIN} {...(name === undefined ? {} : { projectName: name })} />;
  if (it === undefined || words === undefined) return <div className="app-art-wait" data-testid="artifact.thread.wait" />;
  // the maker's own thread, archived or not (dim then, '↺ restore' in its head); only the composer
  // goes to the project's main when the maker is archived (words.agent)
  const maker = it.a.agent;
  return (
    <ThreadScreen
      key={`${project}/${maker}/${id}`}
      project={project}
      agent={maker}
      {...(name === undefined ? {} : { projectName: name })}
      artifact={{ id, title: it.a.title, url: it.a.url, to: words.to, placeholder: words.placeholder, agent: words.agent, ...(words.pink === undefined ? {} : { pink: words.pink.tip }), ...(words.end === undefined ? {} : { end: words.end }) }}
    />
  );
}
