// A .diff or .patch artifact in the panel (try-diff-open, architect m_52963): its files and hunks,
// + and − lines in their colours, drawn by the review's own DiffFiles from the hub's typed `diff`
// (the hub reads bise's copy of that version and parses it with diff::parse); never a blank frame.
// A quote goes to its maker's thread. Owner: nm-pages-b.
import { useEffect } from 'react';
import { useArtifactDiff, useStore } from '@/store';
import { DiffFiles } from '@/review/DiffFiles';
import { pages } from '@/proto/words_pages.gen';
import type { Artifact } from '@/proto';

export function DiffArtifact({ project, a, version, onEscape }: { project: string; a: Artifact; version: number; onEscape: () => void }) {
  const store = useStore();
  const diff = useArtifactDiff(project, a.id, version);
  // asked once per version shown (the hub answers `diff` with its artifact and version)
  useEffect(() => {
    store.command({ cmd: 'diff', project, artifact: a.id, version });
  }, [store, project, a.id, version]);
  return (
    <div className="rv-screen" data-testid="artifact.diff">
      <DiffFiles
        project={project}
        diff={diff}
        loading={pages.review_loading_diff({ title: a.title })}
        none={pages.review_no_changes}
        to={a.agent}
        from={a.title}
        onEscape={onEscape}
        testId="artifact.diff"
      />
    </div>
  );
}
