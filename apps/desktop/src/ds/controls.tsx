// src/ds · the controls a screen may not draw itself (designer's raw-controls call m_18343, via
// ambient-lead m_18391): IconButton, Act, TextArea, Sheet, Select. With prim.tsx's Button / Field /
// Toggle and window.tsx's Row, these are the only places a <button>, <input>, <select> or
// <textarea> is written; the controls lint (ds/controls-lint.mjs, in pnpm check) refuses one
// anywhere else. Radix (headless) is used here only, for the Sheet's dialog and the Select's
// listbox: focus trap, esc, typeahead and the a11y wiring are theirs, the look is the ds's.
// Pure renders of their props, like every ds component (index.ts's rules); css/controls.css.
import { useCoverWhile } from "./cover";
import * as Dialog from "@radix-ui/react-dialog";
import type { ButtonHTMLAttributes, InputHTMLAttributes, KeyboardEvent, MouseEventHandler, ReactNode, Ref, TextareaHTMLAttributes } from "react";
import { cx, st, type UiState } from "./cx";
import { Dropdown } from "./dropdown";
import { tipAttrs } from "./tip";

export interface PressProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "className" | "type"> {
  /** the row's own signed look (a class with its rule in the screen's css: the ds lint checks it) */
  look: string;
  ref?: Ref<HTMLButtonElement>;
}
/**
 * a real button that keeps a signed row's own look (the thread's part chips, the landed sha, the
 * brief line…): the ds resets the native button (no frame, no fill, the line's font and color) and
 * gives it the ds focus ring; the look class draws the rest. For a control nobody drew, use Button,
 * IconButton, Act or Chip instead (designer m_18343 / amb-win m_18625).
 */
export function Press({ look, ref, children, ...button }: PressProps) {
  return (
    <button ref={ref} type="button" className={cx("b-press", look)} {...button}>
      {children}
    </button>
  );
}

export interface InputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "className"> {
  /** the field's own signed look, when its frame isn't the ds Field's (the palette's, the spotlight's) */
  look?: string;
  ref?: Ref<HTMLInputElement>;
}
/**
 * an <input> inside a frame a screen drew and designer signed (the palette's query, the spotlight's
 * line, the log's find, an agent's inline rename): the line's font and color, the frame's look.
 * Anywhere else, use Field (one line) or TextArea (several).
 */
export function Input({ look, ref, ...input }: InputProps) {
  return <input ref={ref} className={cx("b-input", look)} {...input} />;
}

export interface InputAreaProps extends Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, "className" | "children"> {
  /** the field's own signed look (the spotlight's words, spot-shot): its wrap and its cap are there */
  look?: string;
  ref?: Ref<HTMLTextAreaElement>;
}
/**
 * Input's twin for words that wrap: a <textarea> inside a frame a screen drew and designer signed
 * (the spotlight's line, which grows with his words: spot-shot, designer m_32542). Anywhere else,
 * use TextArea.
 */
export function InputArea({ look, ref, ...area }: InputAreaProps) {
  return <textarea ref={ref} className={cx("b-input", look)} {...area} />;
}

export interface IconButtonProps {
  /** what it does, in words: its aria-label and its tooltip (with `keys` after it) */
  label: string;
  /** the glyph */
  children: ReactNode;
  /** its key, shown in the tooltip ('⌘\\') */
  keys?: string;
  /** "bar": 24 px, for the title bar and other bars; "row": 16 wide, a list row's own buttons (text-3); else 28 */
  size?: "bar" | "row";
  /** a toggle's state (aria-pressed): the panel button while the panel shows */
  pressed?: boolean;
  state?: UiState;
  disabled?: boolean;
  onClick?: MouseEventHandler<HTMLButtonElement>;
  testId?: string;
  ref?: Ref<HTMLButtonElement>;
}
/** a square button with a glyph: 28×28 (24 in bars, 16 wide in a row), r-2, glyph text-2, a 6% tint on hover */
export function IconButton({ label, children, keys, size, pressed, state, disabled, onClick, testId, ref }: IconButtonProps) {
  const off = disabled || state === "disabled";
  return (
    <button ref={ref} type="button" className={cx("b-ibtn", size, pressed && "on", st(state))} aria-label={label}
      {...tipAttrs(label, keys)} aria-pressed={pressed} disabled={off}
      onClick={onClick} data-testid={testId}>
      {children}
    </button>
  );
}

export interface SearchButtonProps {
  /** its words ('search or jump') */
  label: string;
  /** its key at the right ('⌘K') */
  keys?: string;
  onClick?: MouseEventHandler<HTMLButtonElement>;
  testId?: string;
}
/** the sidebar's search: a Field-like pill that opens the palette (⌕, the words, the key at the right) */
export function SearchButton({ label, keys, onClick, testId }: SearchButtonProps) {
  return (
    <button type="button" className="b-search" onClick={onClick} data-testid={testId}>
      <span aria-hidden="true">⌕</span>
      <span className="w">{label}</span>
      {keys !== undefined && <span className="b-kbd">{keys}</span>}
    </button>
  );
}

export interface ActProps {
  children: ReactNode;
  /** err: the words in the error tone; danger: text-2 at rest, the error tone on hover (a
   *  destructive act that waits in its row: remove; ui-review-app #12) */
  tone?: "err" | "danger";
  /** ref: a reference in a news line (the landed sha, 'PR #12'): ink, always underlined (designer m_18626) */
  kind?: "ref";
  state?: UiState;
  disabled?: boolean;
  onClick?: MouseEventHandler<HTMLButtonElement>;
  testId?: string;
  title?: string;
  ref?: Ref<HTMLButtonElement>;
}
/** an inline text action in a line ('edit · drop', 'give it back', 'show the report'): text-2, underlined on hover */
export function Act({ children, tone, kind, state, disabled, onClick, testId, title, ref }: ActProps) {
  const off = disabled || state === "disabled";
  return (
    <button ref={ref} type="button" className={cx("b-act", tone, kind, st(state))} disabled={off} onClick={onClick} data-testid={testId} title={title}>
      {children}
    </button>
  );
}

export interface TextAreaProps extends Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, "className" | "children"> {
  state?: "focus" | "error";
  testId?: string;
  ref?: Ref<HTMLTextAreaElement>;
}
/**
 * a field of several lines: the Field's look, grows with its words up to 40% of the window, then
 * scrolls; no resize grip. Uncontrolled-friendly (defaultValue + ref), like Field.
 */
export function TextArea({ state, testId, ref, ...area }: TextAreaProps) {
  return (
    <label className={cx("b-field", "b-area", st(state))}>
      <textarea ref={ref} {...area} aria-invalid={state === "error" || undefined} data-testid={testId} />
    </label>
  );
}

export interface SheetProps {
  open: boolean;
  /** esc, a click on the scrim, or the cancel: the sheet asks to close */
  onClose: () => void;
  title: ReactNode;
  sub?: ReactNode;
  children: ReactNode;
  /** the foot, right-aligned: a ghost cancel then the primary Button */
  foot?: ReactNode;
  /** ⏎ (outside a field of several lines) runs it, as the primary button would */
  onPrimary?: () => void;
  testId?: string;
}
/**
 * a modal over the window: the ground at 62% (no blur), a raised r-3 sheet 520 wide (480 under
 * 800) 12% from the top with --e-sheet, in with a fade and an 8 px rise (ds/css/controls.css), its head
 * (17/600) and sub (t-13, text-2), its body, its foot. Focus stays inside, esc closes, ⏎ runs the
 * primary, and focus goes back to what opened it (Radix Dialog).
 */
export function Sheet({ open, onClose, title, sub, children, foot, onPrimary, testId }: SheetProps) {
  // a native view over the page (a web tab) hides while it shows (cover.ts)
  useCoverWhile(open);
  const enter = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "Enter" || e.shiftKey || e.metaKey || e.altKey || e.ctrlKey || e.nativeEvent.isComposing) return;
    const t = e.target as HTMLElement;
    if (t.tagName === "TEXTAREA" || t.tagName === "BUTTON" || t.isContentEditable || onPrimary === undefined) return;
    e.preventDefault();
    onPrimary();
  };
  return (
    <Dialog.Root open={open} onOpenChange={(o) => { if (!o) onClose(); }}>
      <Dialog.Portal>
        <Dialog.Overlay className="b-scrim" data-testid={testId === undefined ? undefined : `${testId}.scrim`} />
        <Dialog.Content className="b-sheet" onKeyDown={enter} data-testid={testId} aria-describedby={undefined}>
          <div className="b-sheet-head">
            <Dialog.Title className="t">{title}</Dialog.Title>
            {sub != null && <Dialog.Description className="sub">{sub}</Dialog.Description>}
          </div>
          <div className="b-sheet-body">{children}</div>
          {foot != null && <div className="b-sheet-foot">{foot}</div>}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export interface SelectOption {
  value: string;
  label: ReactNode;
  /** dim words after the label ('default for agents') */
  hint?: ReactNode;
}
export interface SelectProps {
  value: string;
  options: readonly SelectOption[];
  onChange: (value: string) => void;
  /** its name in words (aria-label), when no visible label names it */
  label?: string;
  placeholder?: string;
  state?: "focus" | "error" | "disabled";
  testId?: string;
}
/**
 * the settings picker: a Field with the ds caret that opens a ds Picker under it, at least the
 * Field's width (dropdown.tsx's Dropdown, look "field"; designer m_24793): its inline hint and ✓,
 * typeahead, ↑↓ ⏎ esc. Each option's test id is `<testId>.opt.<value>`.
 */
export function Select({ value, options, onChange, label, placeholder, state, testId }: SelectProps) {
  const cur = options.find((o) => o.value === value);
  return (
    <Dropdown look="field" extra={st(state) || undefined} name={label} disabled={state === "disabled"} placeholder={cur === undefined}
      label={cur?.label ?? placeholder ?? ""} align="start" inline
      groups={[{ id: "opts", rows: options.map((o) => ({ id: o.value, name: o.label, secondary: o.hint ?? undefined, current: o.value === value, testId: testId === undefined ? undefined : `${testId}.opt.${o.value}` })) }]}
      onPick={(v) => { if (v !== value) onChange(v); }} testId={testId} />
  );
}
