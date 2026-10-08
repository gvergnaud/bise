//! The keys of the switchboard mode: the panel navigation, the drop and
//! not-delivered questions, the cards, esc (moved out of sb.rs as is).

use super::*;

/// Agent navigation from the keyboard.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Nav {
    Next,
    Prev,
    /// agent number N of the list (0 = main)
    Goto(usize),
}

/// Alt+↓ next, Alt+↑ previous, Alt+N agent N (0 = main). BISE-302:
/// ctrl+k / ctrl+j are the editor's again (line end, newline).
pub(super) fn nav_key(k: &crossterm::event::KeyEvent) -> Option<Nav> {
    match (k.code, k.modifiers) {
        (KeyCode::Down, KeyModifiers::ALT) => Some(Nav::Next),
        (KeyCode::Up, KeyModifiers::ALT) => Some(Nav::Prev),
        (KeyCode::Char(c), KeyModifiers::ALT) if c.is_ascii_digit() => {
            c.to_digit(10).map(|d| Nav::Goto(d as usize))
        }
        _ => None,
    }
}

/// What the switchboard keys change outside the composer: the agent in
/// view, the panel's selection, preview, archived section and drop
/// question, the confirm, the card box. A key that changes it went
/// there, not to the composer (zen, BISE-124).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Scene {
    focus: String,
    selected: Option<usize>,
    preview: bool,
    archived_open: bool,
    drop_ask: bool,
    confirm: bool,
    card: (bool, Option<u64>, usize),
}

pub(crate) fn scene(app: &App) -> Scene {
    let sb = &app.sb;
    Scene {
        focus: sb.focus.clone(),
        selected: sb.selected,
        preview: sb.preview,
        archived_open: sb.archived_open,
        drop_ask: sb.drop_ask.is_some(),
        confirm: sb.confirm.is_some(),
        card: (sb.card.open, sb.card.sel, sb.card.scroll),
    }
}

/// Keys of the switchboard mode; `true` when handled.
pub(crate) fn key(app: &mut App, k: &crossterm::event::KeyEvent, popup_open: bool) -> bool {
    let empty = app.ed.text.is_empty();
    let pending = app.pending;
    let interrupt_requested = app.interrupt_requested;
    let sb = &mut app.sb;
    // `D` asked "archive {name}?": y archives, n or esc keeps it; any other
    // key drops the question and does its usual job
    if let Some(name) = sb.drop_ask.take() {
        let plain = !k.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
        match k.code {
            KeyCode::Char('y') | KeyCode::Char('Y') if plain => {
                sb.send_input(format!("/archive {}", name));
                return true;
            }
            KeyCode::Char('n') | KeyCode::Char('N') if plain => return true,
            KeyCode::Esc => return true,
            _ => {}
        }
    }
    // the cards (cards v2): ctrl+1-9 from the thread, the rest in the
    // card view
    if super::cards::key(app, k, popup_open) {
        return true;
    }
    let sb = &mut app.sb;
    // BISE-86: `✗ not delivered: {name} stopped. ⏎ send again · esc drop`
    // is the last line and the composer is empty: ⏎ sends it again, esc
    // drops it; the question goes away either way
    if empty && matches!(k.code, KeyCode::Enter | KeyCode::Esc) && k.modifiers == KeyModifiers::NONE {
        let last = app.events.iter().rposition(|e| crate::feed::ev_visible(e, false));
        if let Some(i) = last {
            if let Ev::Undelivered { name, text, open: open @ true } = &mut app.events[i] {
                *open = false;
                app.cache[i] = None;
                let (name, text) = (name.clone(), text.clone());
                if k.code == KeyCode::Enter {
                    let v = if sb.focus == name { text.clone() } else { format!("@{} {}", name, text) };
                    sb.send_input(v.clone());
                    push_event(&mut app.events, &mut app.cache, Ev::You(v, Mark::Sent, false));
                }
                return true;
            }
        }
    }
    let n = sb.nav().len();
    let nav = nav_key(k);
    match (k.code, k.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) if pending && !interrupt_requested => {
            let f = sb.focus.clone();
            sb.call("turn/interrupt", json!({"agent": f}), super::rpc::Then::Shown);
            // computer-use-design.md §7.3: it lets go of Chrome and its apps too
            if crate::computer_use::driving(&f).is_some() {
                crate::computer_use::stop(&sb.dir_of(&f));
            }
            app.interrupt_requested = true;
            push_event(
                &mut app.events,
                &mut app.cache,
                Ev::Info("interrupted — the turn stops at the next safe point · ctrl+c again to quit".into()),
            );
            true
        }
        // computer use: `? you took the wheel · ⏎ give it back`
        (KeyCode::Enter, KeyModifiers::NONE) if empty && sb.selected.is_none() && crate::computer_use::paused(&sb.focus) => {
            crate::computer_use::give_back(&sb.focus.clone());
            true
        }
        _ if empty && n > 0 && nav == Some(Nav::Next) => {
            sb.selected = Some(match sb.selected {
                None => 0,
                Some(i) => (i + 1) % n,
            });
            true
        }
        _ if empty && n > 0 && nav == Some(Nav::Prev) => {
            sb.selected = Some(match sb.selected {
                None | Some(0) => n - 1,
                Some(i) => i - 1,
            });
            true
        }
        (KeyCode::Enter, KeyModifiers::NONE) if empty && sb.selected.is_some() => {
            if let Some(name) = sb.selected_agent().map(|a| a.name.clone()) {
                focus(app, &name);
            }
            true
        }
        (KeyCode::Char(' '), _) if empty && sb.selected.is_some() => {
            sb.preview = !sb.preview;
            true
        }
        (KeyCode::Char('A'), _) if empty && sb.selected.is_some() => {
            sb.toggle_archived();
            true
        }
        (KeyCode::Char('D'), _) if empty && sb.selected.is_some() => {
            let live = |a: &&Agent| !a.main && !a.archived();
            if let Some(name) = sb.selected_agent().filter(live).map(|a| a.name.clone()) {
                sb.drop_ask = Some(name);
            }
            true
        }
        (KeyCode::Esc, _) if !popup_open => {
            if let Some((id, _)) = sb.confirm.take() {
                sb.send(json!({"op": "confirm", "id": id, "yes": false}));
                return true;
            }
            if sb.selected.is_some() || sb.preview {
                sb.selected = None;
                sb.preview = false;
                return true;
            }
            if app.ed.selection().is_some() {
                // a selection in the composer goes first, the text stays
                // (BISE-284: the first run's `show me what you can do`)
                app.ed.anchor = None;
                return true;
            }
            if !empty {
                // the draft goes to the history (Up brings it back)
                let d = app.ed.take();
                app.history.insert(0, d);
                return true;
            }
            if sb.focus != "main" {
                focus(app, "main");
                return true;
            }
            false
        }
        _ if matches!(nav, Some(Nav::Goto(_))) => {
            // the number shown in the panel (it stays while the agent lives)
            if let Some(t) = nav.and_then(|n| if let Nav::Goto(i) = n { sb.agent_numbered(i) } else { None }) {
                focus(app, &t);
            }
            true
        }
        // ctrl+z undoes the composer's edits (editor.rs); with nothing
        // left to undo there, no undo of what was sent (book §13): say
        // it, and how to change course
        (KeyCode::Char('z'), KeyModifiers::CONTROL) if !app.ed.can_undo() => {
            push_event(&mut app.events, &mut app.cache, Ev::Info(NO_UNDO.into()));
            true
        }
        _ => false,
    }
}
