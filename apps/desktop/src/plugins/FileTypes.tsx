// File types, the Plugins place's third group (contract docs/plugin-apps.md A3, A6b; designer's
// page plugin-apps-mocks block 5, m_60036): the one place where every pick is seen and reset. One
// row per type the hub knows: the type ('.xlsx'), its app as a button that opens the candidates
// like the model picker ('Sheets ▾'; none: 'your Mac', no menu), a faint status ('default', 'your
// pick', 'no app'; never pink: nothing here waits on him) and Reset on his picks. A pick and Reset
// are the core's opener_set; the plugins event brings the rows back. The words are typerows.ts's.
// Owner: apps-gallery.
import { Button, Dropdown, GroupLabel, sentenceCase } from '@/ds';
import type { AppCmd, FileType } from '@/proto';
import { settings } from '@/proto/words.gen';
import { openerSet, typeLines, type TypeLine } from './typerows';

export function FileTypes({ rows, send, project }: { rows: readonly FileType[]; send: (cmd: AppCmd) => void; project?: string }) {
  const at = project === undefined ? {} : { project };
  const lines = typeLines(rows.map((r) => ({ ...r, others: r.others ?? [] })));
  const set = (type: string, id?: string) => { send({ ...openerSet(type, id), ...at }); };
  if (lines.length === 0) return <div className="plg-empty" data-testid="plugins.types.empty">{settings.file_types_empty}</div>;
  return (
    <div className="plg-group" data-testid="plugins.types">
      <GroupLabel count={lines.length}>{sentenceCase(settings.file_types_head)}</GroupLabel>
      <div className="plg-list">
        {lines.map((l) => <TypeRow key={l.type} l={l} onPick={(id) => { set(l.type, id); }} onReset={() => { set(l.type); }} />)}
      </div>
    </div>
  );
}

function TypeRow({ l, onPick, onReset }: { l: TypeLine; onPick: (id: string) => void; onReset: () => void }) {
  const testId = `plugins.types.row.${l.type}`;
  return (
    <div className="plg-type" data-testid={testId}>
      <span className={`plg-type-name${l.ending ? ' end' : ''}`} data-testid={`${testId}.name`}>{l.name}</span>
      <span className="plg-type-app">
        {l.picks.length === 0 ? (
          <span className="plg-type-one" data-testid={`${testId}.app`}>{l.app}</span>
        ) : (
          <Dropdown label={l.app} testId={`${testId}.app`}
            groups={[{ id: 'apps', rows: l.picks.map((p) => ({ id: p.id, name: p.app, current: p.current, testId: `${testId}.pick.${p.id}` })) }]}
            onPick={onPick} />
        )}
      </span>
      <span className="plg-type-st" data-testid={`${testId}.status`}>{l.status}</span>
      <span className="plg-type-end">
        {l.reset && <Button size="sm" kind="ghost" onClick={onReset} testId={`${testId}.reset`}>{settings.file_type_reset}</Button>}
      </span>
    </div>
  );
}
