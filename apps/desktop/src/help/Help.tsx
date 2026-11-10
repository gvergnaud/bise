// ⌘/: the key list as an overlay (tui-parity V3; the
// TUI's /help and /shortcuts, rust/tui/src/help.rs). Its own field: typing
// filters, tab / shift+tab switch help and shortcuts, ↑↓ pgup pgdn scroll,
// esc clears the filter then closes, ctrl+c closes. The composer keeps its
// draft. Rows from settings/keys.ts (read only). Owner: amb-web.
import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { Field, Kbd, Seg } from '@/ds';
import { escOf, helpRows, otherPage, PAGES, type HelpPage } from './rows';
import { caretAtEnd, useKeptText } from '@/app/keep';
import './help.css';

const STEP = 40;

export function Help({ onClose, page: first = 'help' }: { onClose: () => void; page?: HelpPage }) {
  const [page, setPage] = useState<HelpPage>(first);
  // his filter survives a reload while help is open (keep.ts; closing it drops it)
  const [q, setQ, dropQ] = useKeptText('help');
  const field = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const sections = helpRows(page, q);
  // out like the ds sheet: a 120 ms fade, then the parent unmounts it (its animationend)
  const [closing, setClosing] = useState(false);
  const close = () => {
    setClosing(true);
  };

  useEffect(() => {
    field.current?.focus();
    caretAtEnd(field.current);
  }, []);
  useEffect(() => dropQ, [dropQ]);

  const scroll = (by: number | 'top' | 'end') => {
    const el = list.current;
    if (el === null) return;
    if (by === 'top') el.scrollTop = 0;
    else if (by === 'end') el.scrollTop = el.scrollHeight;
    else el.scrollTop += by;
  };

  const onKey = (e: KeyboardEvent) => {
    const page1 = Math.max(STEP, (list.current?.clientHeight ?? 0) - STEP);
    if (e.key === 'Tab') {
      setPage(otherPage);
      scroll('top');
    } else if (e.key === 'Escape') {
      if (escOf(q) === 'clear') setQ('');
      else close();
    } else if (e.ctrlKey && e.key === 'c') close();
    else if (e.key === 'ArrowDown') scroll(STEP);
    else if (e.key === 'ArrowUp') scroll(-STEP);
    else if (e.key === 'PageDown') scroll(page1);
    else if (e.key === 'PageUp') scroll(-page1);
    else return;
    e.preventDefault();
    e.stopPropagation();
  };

  return (
    <div
      className={closing ? 'help-over out' : 'help-over'}
      data-testid="help.overlay"
      data-page={page}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) close();
      }}
      onAnimationEnd={(e) => {
        if (closing && e.target === e.currentTarget) onClose();
      }}
    >
      <div className="help-box" onKeyDown={onKey} role="dialog" aria-label="keys">
        <div className="help-head">
          <Seg
            items={PAGES}
            on={PAGES.indexOf(page)}
            onPick={(i) => {
              setPage(PAGES[i] ?? 'help');
              field.current?.focus();
            }}
            testId="help.tabs"
          />
          <Field
            ref={field}
            value={q}
            placeholder="filter the keys"
            aria-label="filter the keys"
            onChange={(e) => {
              setQ(e.currentTarget.value);
              scroll('top');
            }}
            testId="help.field"
          />
        </div>
        <div className="help-list" ref={list} data-testid="help.list">
          {sections.length === 0 && <div className="help-none">no key matches “{q}”</div>}
          {sections.map((s) => (
            <section key={s.id} data-testid={`help.section.${s.id}`}>
              <h3>{s.title}</h3>
              {s.rows.map((r) => (
                <div className="help-row" key={r.keys} data-testid="help.row" data-keys={r.keys}>
                  <span className="k">
                    <Kbd>{r.keys}</Kbd>
                    {r.also !== undefined && (
                      <span className="also" data-testid="help.row.also">
                        {' · '}
                        <Kbd>{r.also}</Kbd>
                      </span>
                    )}
                  </span>
                  <span className="a">{r.action}</span>
                  {r.tui !== undefined && <span className="t">terminal: {r.tui}</span>}
                </div>
              ))}
            </section>
          ))}
        </div>
        <div className="help-keys">type to filter · tab {otherPage(page)} · esc {escOf(q) === 'clear' ? 'clear' : 'close'}</div>
      </div>
    </div>
  );
}
