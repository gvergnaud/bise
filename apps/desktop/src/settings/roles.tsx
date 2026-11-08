// Which model does what (page models-settings, Gabriel's pick A): settings · Setup's one list, open
// and first. One row per role: its name and a plain line, its model by name (provider faint, the
// raw id in the tip) as a button that opens the picker in place, one right-aligned status; Voice
// mode as hears / thinks / speaks. On top, only when something stopped, one fix line (the core's
// Next); each row it stops says why in its status. A failed row opens its troubleshooting on his
// click. Nothing is tested at open: Test everything is his click, a role is tested when it changes
// (the core's role_set). The words are roleList.ts's; the TUI's /models says the same.
// Owner: setup-page.
import { useState, type ReactNode } from 'react';
import type { AppCmd, FeatureId, FeaturePart, FeatureRow, Model, RoleRow, SetupGauge } from '@/proto';
import { Act, Button, DropdownPair, Help, PickerSeg, Press, tipAttrs } from '@/ds';
import { cx } from '@/ds/cx';
import { settings } from '@/proto/words.gen';
import { Trouble, TryIt, fixButton, type GaugePorts } from './gauge';
import { modelRow } from './hold';
import { ABOUT, FALLBACK, ROW_NAME, actInPlace, fixLine, hasTrouble, listed, modelView, partName, partStatus, partsInOrder, pickSides, rowStatus, type Status } from './roleList';

export interface RoleListProps {
  g: SetupGauge;
  roles: readonly RoleRow[];
  models: readonly Model[];
  /** a role_set the core refused (its words) */
  error?: string | null;
  ports: GaugePorts & {
    /** set a role: the command, and the model it sets (a model whose provider isn't set up waits: hold.ts) */
    set: (cmd: AppCmd, model: string | null) => void;
  };
  /** the row the route lands on (/voice: dictation) */
  at?: FeatureId | null;
}

/** A status cell: its words in its tone, its tip. */
function StatusMark({ s, testId, toggle }: { s: Status; testId: string; toggle?: { open: boolean; flip: () => void } }) {
  // a stopped role's status: a link that opens its troubleshooting under the row (designer m_57293)
  if (toggle !== undefined) {
    return (
      <Press look={cx('st-mark', 'st-link', s.tone)} aria-expanded={toggle.open} onClick={toggle.flip} {...tipAttrs(s.tip)} data-testid={testId}>
        {s.text}
      </Press>
    );
  }
  return (
    <span className={cx('st-mark', s.tone)} {...tipAttrs(s.tip)} data-testid={testId}>
      {s.text}
    </span>
  );
}

/** A role's model: a button that opens its picker in place when the row owns the role, else its
 * name with why it isn't one (bise's model, an env var), the raw id or the step in the tip. */
function ModelCell({ r, p, props, testId }: { r: FeatureRow; p: FeaturePart; props: RoleListProps; testId: string }) {
  const { roles, models, ports } = props;
  const v = modelView(r, p, roles, models);
  // no model yet: a role he picks one for (the picker, its words say so), else nothing to show
  const own = roles.find((x) => x.role === p.role);
  const role = v === null ? (own?.kind === 'chat' && r.fix?.t === 'pick_model' ? own : undefined) : v.role === null ? undefined : own;
  const words: ReactNode =
    v === null ? (
      <span className="st-mp">{settings.part_pick}</span>
    ) : (
      <>
        <span className="st-mn">{v.name}</span>
        {v.faint !== '' && <span className="st-mp">{v.faint}</span>}
        {v.effort !== null && <span className="st-mp">· {v.effort}</span>}
      </>
    );
  if (role === undefined) {
    return v === null ? null : (
      <span className="st-model" {...tipAttrs(v.tip)} data-testid={testId}>
        {words}
      </span>
    );
  }
  const efforts = modelRow(role.model, models)?.efforts ?? [];
  const effort = role.effort ?? modelRow(role.model, models)?.default_effort ?? null;
  return (
    <span className="st-model-pick">
      <DropdownPair
        label={words}
        extra="st-pick"
        name={settings.role_model({ role: ROW_NAME[r.id] })}
        tip={v?.tip ?? settings.part_pick}
        back={settings.pick_back}
        {...pickSides(role, roles, models)}
        {...(efforts.length > 0
          ? {
              footer: (
                <PickerSeg
                  head={settings.roles_effort_title}
                  items={efforts}
                  on={effort}
                  onPick={(e) => {
                    if (e !== effort) ports.set({ cmd: 'role_set', role: role.role, effort: e }, null);
                  }}
                  testId={`${testId}.effort`}
                />
              ),
            }
          : {})}
        onPick={(id) => {
          const model = id === FALLBACK ? null : id;
          ports.set({ cmd: 'role_set', role: role.role, model }, model);
        }}
        testId={testId}
      />
    </span>
  );
}

/** One row on the list's three columns, role · model · status (designer m_57293), never more: a
 * stopped role's status is a link to its troubleshooting (its fix there, or on the fix line); a
 * role with no model shows its act in the model's place ('Turn on'); Voice mode's parts under it. */
function Row({ r, props, open, toggle }: { r: FeatureRow; props: RoleListProps; open: boolean; toggle: () => void }) {
  const { g, ports } = props;
  const one = r.parts.length === 1 ? r.parts[0] : undefined;
  const status = rowStatus(r, g);
  const act = actInPlace(r);
  const better = r.fix == null && r.better != null && r.better.t === 'allow_perm' ? r.better : null;
  const link = hasTrouble(r) ? { open, flip: toggle } : undefined;
  const mark = status === null ? null : <StatusMark s={status} testId={`settings.feature.${r.id}.mark`} {...(link !== undefined ? { toggle: link } : {})} />;
  return (
    <div className={cx('st-feature', props.at === r.id && 'is-at')} data-state={r.state} data-testid={`settings.feature.${r.id}`}>
      <div className="st-row">
        <span className="st-name">
          {ROW_NAME[r.id]}
          <span className="st-sub">{ABOUT[r.id]}</span>
        </span>
        <span className="st-cell">
          {act !== null ? (
            <Button size="sm" onClick={() => { ports.fix(act, r.id); }} testId={`settings.feature.${r.id}.fix`}>
              {fixButton(act, g)}
            </Button>
          ) : (
            one !== undefined && <ModelCell r={r} p={one} props={props} testId={modelTestId(r, one)} />
          )}
        </span>
        <span className="st-status">
          {better !== null && (
            <span className="st-try">
              <Act onClick={() => { ports.fix(better, r.id); }} testId={`settings.feature.${r.id}.better`}>
                {fixButton(better, g)}
              </Act>
            </span>
          )}
          {r.id === 'dictation' && r.state === 'on' && ports.tryIt !== undefined ? <TryIt port={ports.tryIt} mark={mark} /> : mark}
        </span>
      </div>
      {one === undefined &&
        partsInOrder(r).map((p) => (
          <div key={p.role} className="st-row st-part" data-testid={`settings.feature.${r.id}.${p.role}`}>
            <span className="st-name">{partName(p)}</span>
            <span className="st-cell">
              {/* a part with no model: the row's act in its model's place, like a row's (ui-review m_57328) */}
              {(p.name ?? '') === '' && r.fix != null && r.fix.t !== 'pick_model' ? (
                <Button size="sm" onClick={() => { if (r.fix != null) ports.fix(r.fix, r.id); }} testId={`settings.feature.${r.id}.${p.role}.fix`}>
                  {fixButton(r.fix, g)}
                </Button>
              ) : (
                <ModelCell r={r} p={p} props={props} testId={modelTestId(r, p)} />
              )}
            </span>
            <span className="st-status">
              <StatusMark s={partStatus(p, g)} testId={`settings.feature.${r.id}.${p.role}.mark`} {...(p.state === 'failed' && link !== undefined ? { toggle: link } : {})} />
            </span>
          </div>
        ))}
      {open && <Trouble r={r} g={g} ports={ports} />}
    </div>
  );
}

/** A model cell's test id: the role's own picker (settings.role.<role>.model), else the row's. */
function modelTestId(r: FeatureRow, p: FeaturePart): string {
  if (r.parts.length > 1) return `settings.feature.${r.id}.${p.role}.model`;
  return r.id === 'vision' ? `settings.feature.${r.id}.model` : `settings.role.${p.role}.model`;
}

export function RoleList(props: RoleListProps) {
  const { g, ports, error = null } = props;
  const [opened, setOpened] = useState<ReadonlySet<FeatureId>>(new Set());
  const line = fixLine(g);
  const testing = (g.testing?.length ?? 0) > 0;
  return (
    <section className="st-list" data-testid="settings.roles">
      <div className="st-list-head">
        <h3 className="st-list-title">{settings.roles_title}</h3>
        <span {...tipAttrs(settings.gauge_test_tip, undefined, 'below')}>
          <Act disabled={testing} onClick={() => { ports.test(); }} testId="settings.gauge.test">
            {testing ? settings.gauge_testing : settings.gauge_test_all}
          </Act>
        </span>
      </div>
      {line !== null && (
        <div className="st-fixline" data-testid="settings.fixline">
          <span className="w">
            <b>{settings.gauge_next}</b> {line.text}
          </span>
          <Button kind="pri" size="sm" onClick={() => { ports.fix(line.fix, null); }} testId="settings.fixline.fix">
            {fixButton(line.fix, g)}
          </Button>
        </div>
      )}
      <div className="st-rows" data-testid="settings.features">
        {listed(g).map((r) => (
          <Row
            key={r.id}
            r={r}
            props={props}
            open={opened.has(r.id)}
            toggle={() => {
              const next = new Set(opened);
              if (!next.delete(r.id)) next.add(r.id);
              setOpened(next);
            }}
          />
        ))}
      </div>
      {error !== null && (
        <Help err testId="settings.roles.error">
          {error}
        </Help>
      )}
      <p className="st-help">{settings.roles_help}</p>
    </section>
  );
}
