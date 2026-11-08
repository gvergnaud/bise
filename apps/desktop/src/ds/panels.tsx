// src/ds · panels and the rest (desktop-ds v5: components/board.html, sideviews.html,
// settings.html, onboarding.html, capsule.html; css/board.css, sideviews.css, settings.css,
// onboarding.css, capsule.css): Board + Job + Thumbs, SideView, Group + Check, Onb + FnKey,
// Capsule + Wave + Shot + Edge.
import { memo, useId, type MouseEventHandler, type ReactNode, type Ref } from "react";
import { cx, sub } from "./cx";
import { Bar, Button, Chip, type ChipProps, Dot, Empty, Help, Kbd, Pearl, Skel, Toggle, type DotState, type ToggleProps } from "./prim";

// ---- the task board

export interface JobProps {
  /** wk working, nd needs you, ok done, er error, "" idle, sched scheduled, loading */
  state: DotState | "" | "sched" | "loading";
  name: ReactNode;
  /** "⌥1" */
  hotkey?: ReactNode;
  /** it runs in its own worktree (ψ) */
  psi?: boolean;
  /** "12 min", "10:40", "idle", or a chip */
  end?: ReactNode;
  endChip?: ChipProps;
  /** one line: what it does */
  what?: ReactNode;
  /** a known end (only while it works) */
  pct?: number;
  /** no known end: the running bar (only while it works) */
  ind?: boolean;
  on?: boolean;
  hover?: boolean;
  onClick?: MouseEventHandler<HTMLDivElement>;
  testId?: string;
  ref?: Ref<HTMLDivElement>;
}
/** a job's card on the board */
export const Job = memo(function Job({ state, name, hotkey, psi, end, endChip, what, pct, ind, on, hover, onClick, testId, ref }: JobProps) {
  const lead = state === "loading" ? <Skel /> : state === "sched" ? <Kbd>◷</Kbd> : <Dot state={state || undefined} />;
  return (
    <div ref={ref} className={cx("b-task", state === "nd" && "nd", state === "ok" && "done", state === "er" && "er", on && "on", hover && "is-hover")}
      onClick={onClick} role={onClick ? "button" : undefined} aria-current={on || undefined} data-testid={testId}>
      <div className="a">
        {lead} <b>{name}</b> {hotkey != null && <Kbd>{hotkey}</Kbd>}{psi && " ψ"}
        <span className="end">{endChip ? <Chip {...endChip} /> : end}</span>
      </div>
      <div className="w">{what}</div>
      {state === "wk" && (pct != null ? <Bar pct={pct} /> : ind ? <Bar /> : null)}
    </div>
  );
});

/** the board: Job children */
export function Board({ children, testId }: { children: ReactNode; testId?: string }) {
  return <div className="b-board" data-testid={testId}>{children}</div>;
}

export interface Thumb {
  id: string;
  title: ReactNode;
  /** an svg or an img */
  thumb?: ReactNode;
  hover?: boolean;
}
/** what was made today, two per row; each one's test id is `<testId>.<id>` */
export function Thumbs({ items, onOpen, testId }: { items: readonly Thumb[]; onOpen?: (id: string) => void; testId?: string }) {
  return (
    <div className="b-thumbs" data-testid={testId}>
      {items.map((t) => (
        <div key={t.id} className={cx("b-tn", t.hover && "is-hover")} onClick={onOpen ? () => { onOpen(t.id); } : undefined} role={onOpen ? "button" : undefined}
          data-testid={sub(testId, t.id)}>
          {t.thumb ?? <div className="ph" />}<span>{t.title}</span>
        </div>
      ))}
    </div>
  );
}

// ---- side views

export interface SideViewProps {
  title: ReactNode;
  /** ghost buttons and a chip on the head's right */
  end?: ReactNode;
  loading?: boolean;
  empty?: { title: ReactNode; text?: ReactNode };
  /** Row children (a diff's files, an artifact's versions), a Diff, a pane */
  children?: ReactNode;
  testId?: string;
}
/** a panel on the window's right: a head and its body */
export function SideView({ title, end, loading, empty, children, testId }: SideViewProps) {
  return (
    <div className="b-sideview" data-testid={testId}>
      <div className="sh2">{title}{end != null && <span className="end">{end}</span>}</div>
      <div className="b-files">
        {loading && <><Skel /><br /><Skel /><br /><Skel /></>}
        {empty && <Empty title={empty.title}>{empty.text}</Empty>}
        {children}
      </div>
    </div>
  );
}

// ---- settings

/** a checkbox */
export function Check({ on, onChange, label, testId }: { on: boolean; onChange?: (on: boolean) => void; label?: string; testId?: string }) {
  return (
    <span className={cx("b-check", on && "on")} role="checkbox" aria-checked={on} aria-label={label} tabIndex={0} data-testid={testId}
      onClick={onChange ? () => { onChange(!on); } : undefined}
      onKeyDown={(e) => { if (onChange && (e.key === " " || e.key === "Enter")) { e.preventDefault(); onChange(!on); } }}>
      {on ? "✓" : ""}
    </span>
  );
}

/** a checkbox with its label, the whole row clickable (designer m_56751): the box is the Check's look,
 * the row is the checkbox (its focus ring drawn on the box) */
export function CheckRow({ on, onChange, children, testId }: { on: boolean; onChange: (on: boolean) => void; children: ReactNode; testId?: string }) {
  return (
    <span className="b-checkrow" role="checkbox" aria-checked={on} tabIndex={0} data-testid={testId}
      onClick={() => { onChange(!on); }}
      onKeyDown={(e) => { if (e.key === " " || e.key === "Enter") { e.preventDefault(); onChange(!on); } }}>
      <span className={cx("b-check", on && "on")} aria-hidden>{on ? "✓" : ""}</span>
      <span>{children}</span>
    </span>
  );
}

export interface GroupItem {
  id: string;
  label: ReactNode;
  sub?: ReactNode;
  /** a checkbox before the label */
  check?: ReactNode;
  /** the control on the right: a Button, a Chip, a Field, or a value */
  control?: ReactNode;
  /** a switch row (designer m_44968): the Toggle on the right, and the whole row is its <label>, so a
   * click anywhere on it flips the switch; disabled, the label goes text-3 */
  toggle?: ToggleProps;
  value?: ReactNode;
  /** a list's row (Group list): the keys' selection is on it (the 9% tint, past hover's 6%) */
  sel?: boolean;
  /** a list's row: a click picks it (the mouse never moves the selection otherwise) */
  onPick?: () => void;
}
/** a settings group: a small head, its rows, a help line. `list`: its rows are a list the keys walk
 * (useListKeys): each row carries data-row, hover tints 6%, the selected row 9% (designer m_22280). */
export function Group({ title, items, help, list, testId }: { title?: ReactNode; items: readonly GroupItem[]; help?: ReactNode; list?: boolean; testId?: string }) {
  const uid = useId();
  return (
    <div data-testid={testId}>
      {title != null && <div className="b-gt">{title}</div>}
      <div className={cx("b-group", list && "is-list")}>
        {items.map((it) => {
          const body = <span className="l">{it.check}{it.check != null && " "}{it.label}{it.sub != null && <span>{it.sub}</span>}</span>;
          if (it.toggle !== undefined) {
            const id = `${uid}-${it.id}`;
            return (
              <label key={it.id} htmlFor={id} className={cx("b-gi", "has-switch", it.toggle.state === "disabled" && "is-disabled")} data-testid={sub(testId, it.id)}>
                {body}<Toggle {...it.toggle} id={id} />
              </label>
            );
          }
          return (
            <div key={it.id} className={cx("b-gi", it.sel && "is-sel")} data-testid={sub(testId, it.id)}
              role={it.onPick === undefined ? undefined : list ? "option" : "button"}
              data-row={list ? it.id : undefined} aria-selected={list ? it.sel === true : undefined} onClick={it.onPick}>
              {body}{it.control}{it.value != null && <span className="v">{it.value}</span>}
            </div>
          );
        })}
      </div>
      {help != null && <Help>{help}</Help>}
    </div>
  );
}

// ---- onboarding

/** the fn key, drawn as a key */
export function FnKey({ down }: { down?: boolean }) {
  return <span className={cx("b-fn", down && "down")}>fn</span>;
}

export interface OnbProps {
  /** over the title: bise's typed ':*' (mark.tsx Kiss; never the mist pearl, desktop-onboarding v4) */
  mark?: ReactNode;
  title: ReactNode;
  sub?: ReactNode;
  children?: ReactNode;
  fn?: "up" | "down";
  cta?: { text: ReactNode; onClick?: MouseEventHandler<HTMLButtonElement>; state?: "loading" | "disabled"; testId?: string };
  steps?: number;
  step?: number;
  testId?: string;
}
/** an onboarding step */
export function Onb({ mark, title, sub: subtitle, children, fn, cta, steps, step, testId }: OnbProps) {
  return (
    <div className="b-onb" data-testid={testId}>
      {mark != null && <div className="mk">{mark}</div>}
      <div className="t">{title}</div>
      {subtitle != null && <div className="s">{subtitle}</div>}
      {children}
      {fn && <FnKey down={fn === "down"} />}
      {cta && <Button kind="pri" state={cta.state} onClick={cta.onClick} testId={cta.testId}>{cta.text}</Button>}
      {steps != null && steps > 0 && (
        <div className="steps">{Array.from({ length: steps }, (_, i) => <i key={i} className={i === step ? "on" : undefined} />)}</div>
      )}
    </div>
  );
}

// ---- the fn capsule

/** the voice's five bars */
export function Wave({ off }: { off?: boolean }) {
  return <span className={cx("b-wave", off && "off")} aria-hidden="true"><i /><i /><i /><i /><i /></span>;
}

/** a screen capture's thumb */
export function Shot({ children }: { children?: ReactNode }) {
  return <span className="b-shot">{children}</span>;
}

export interface CapsuleProps {
  /** "off": no network, a dim dot */
  lead?: "pearl" | "off";
  mood?: string;
  /** the live pearl, else the reference's */
  pearl?: ReactNode;
  /** his words; <span className="lo"> quieter, <b> a name */
  words: ReactNode;
  typing?: boolean;
  wave?: "on" | "off";
  /** the capture it attaches */
  shot?: ReactNode;
  shotBelow?: boolean;
  chips?: ReactNode;
  /** the route's drain bar */
  countdown?: boolean;
  /** "release fn to send   fn esc cancel" */
  hint?: ReactNode;
  testId?: string;
}
/** the capsule */
export function Capsule({ lead, mood, pearl, words, typing, wave, shot, shotBelow, chips, countdown, hint, testId }: CapsuleProps) {
  const shotEl = shot != null ? <Shot>{shot}</Shot> : null;
  return (
    <div className="b-capsule" data-testid={testId}>
      <div className="r1">
        {lead === "off" ? <Dot state="off" lg /> : pearl ?? <Pearl size={56} mood={mood} />}
        <div className="wd">{words}{typing && <span className="cur" />}</div>
        {wave && <Wave off={wave === "off"} />}
        {!shotBelow && shotEl}
      </div>
      {chips != null && <div className="r2">{shotBelow && shotEl}{chips}</div>}
      {countdown && <div className="cd"><i /></div>}
      {hint != null && <div className="hint">{hint}</div>}
    </div>
  );
}

/** the screen's edge while fn listens */
export function Edge({ children }: { children?: ReactNode }) {
  return <div className="b-edge">{children}</div>;
}

