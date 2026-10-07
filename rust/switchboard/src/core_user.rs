//! The user's commands (the TUI's typed line, the window's typed
//! commands): `user_input` parses a line (router::parse) for the agent in
//! view, `user_cmd` runs a parsed command. The window's typed `new`,
//! `rename`, `model` and `effort` (bise-proto HubCmd, bar A.1 and A.5)
//! come as `Input::UserCmd` straight to `user_cmd` with their fields, so
//! nothing is rebuilt as text and parsed again (architect m_10331): the
//! TUI and the window run the same handlers.

use super::*;

impl Hub {
    pub(super) fn user_input(
        &mut self,
        fx: &mut Fx,
        env: &mut dyn Env,
        client: ClientId,
        focus: &str,
        text: &str,
        queued: bool,
    ) {
        let focus = self.focus_of(client, focus);
        // only words wait for the turn's end: a command runs now
        let cmd = router::parse(text, &focus);
        if queued && !matches!(cmd, UserCmd::Say(_) | UserCmd::To { .. }) {
            fx.push(notice(client, "commands run now, not queued"));
        }
        self.user_cmd(fx, env, client, focus, cmd, queued);
    }

    /// The agent `focus` names (main when none), now `client`'s focus.
    pub(super) fn focus_of(&mut self, client: ClientId, focus: &str) -> String {
        let focus = self.st.resolve(focus).unwrap_or_else(|| MAIN.to_string());
        if let Some(v) = self.clients.get_mut(&client) {
            if v.focus != focus {
                v.focus = focus.clone();
            }
        }
        focus
    }

    /// One command of `client`, for the agent `focus` (resolved).
    pub(super) fn user_cmd(&mut self, fx: &mut Fx, env: &mut dyn Env, client: ClientId, focus: String, cmd: UserCmd, queued: bool) {
        let c = Some(client);
        match cmd {
            UserCmd::Say(t) => {
                if !t.is_empty() {
                    self.core(fx, env, c, json!({"t": "say", "focus": focus, "text": t, "queued": queued}));
                }
            }
            UserCmd::To { target, text } => self.core(
                fx,
                env,
                c,
                json!({"t": "route", "target": target, "text": text, "focus": focus, "queued": queued}),
            ),
            UserCmd::New {
                name,
                brief,
                worktree,
                with_changes,
            } => {
                // the TUI's router refuses it early too; the typed `new` has
                // only this check
                if with_changes && !worktree {
                    return fx.push(notice(client, "--with-changes only works with -w"));
                }
                let b = Brief {
                    objective: brief,
                    ..Brief::default()
                };
                match new_task(name.as_deref(), &b, worktree, with_changes) {
                    Ok(mut v) => {
                        v["t"] = json!("new");
                        self.core(fx, env, c, v)
                    }
                    Err(e) => fx.push(notice(client, &e)),
                }
            }
            UserCmd::Drop { name, force } => match name {
                None => fx.push(notice(
                    client,
                    "usage: /archive <agent> (or /archive from the agent's view)",
                )),
                Some(name) => self.core(
                    fx,
                    env,
                    c,
                    json!({"t": "drop", "name": name, "force": force}),
                ),
            },
            UserCmd::Restore { name } => {
                self.core(fx, env, c, json!({"t": "restore", "name": name}))
            }
            UserCmd::Isolate { name } => {
                self.core(fx, env, c, json!({"t": "isolate", "name": name}))
            }
            UserCmd::Rename { name, new_name } => {
                let valid = router::valid_name(&new_name);
                self.core(
                    fx,
                    env,
                    c,
                    json!({"t": "rename", "name": name, "new_name": new_name, "valid": valid}),
                )
            }
            UserCmd::Answer { card, text } => {
                self.core(fx, env, c, json!({"t": "answer", "card": card, "text": text}))
            }
            UserCmd::Close { card } => {
                self.core(fx, env, c, json!({"t": "close", "card": card}))
            }
            UserCmd::Tasks => fx.push(notice(client, &board::user_board(&self.st, env.now()))),
            UserCmd::Prs => {
                // `prs`: the typed rows (bise-proto rows::Pr), the TUI
                // draws them like the PR lines; `text` for a client that
                // does not know them (sb's CLI prints it)
                let places = crate::place::places(&self.st, &self.prs);
                let rows = crate::forge::news::pr_rows(&places);
                let text = crate::forge::news::prs_list(&rows);
                if rows.is_empty() {
                    fx.push(notice(client, &text));
                } else {
                    let head = crate::forge::news::prs_head(&rows);
                    fx.push(Effect::ToClient { client, body: json!({"ev": "prs", "head": head, "rows": rows, "text": text}) })
                }
            }
            UserCmd::Interrupt => {
                self.interrupt_by = Some("user".into());
                self.core(fx, env, c, json!({"t": "interrupt", "agent": focus}));
                self.interrupt_by = None;
            }
            UserCmd::Passthrough(l) => {
                let first = l.split_whitespace().next().unwrap_or("");
                if first != "/compact" {
                    fx.push(notice(
                        client,
                        &format!("unknown command: {} (see /help)", first),
                    ));
                    return;
                }
                self.core(
                    fx,
                    env,
                    c,
                    json!({"t": "passthrough", "focus": focus, "line": l}),
                )
            }
            UserCmd::Model { model, default } => fx.push(Effect::Choose {
                client,
                agent: focus,
                model,
                effort: None,
                default,
            }),
            UserCmd::Reasoning { effort } => fx.push(Effect::Choose {
                client,
                agent: focus,
                model: None,
                effort,
                default: false,
            }),
            UserCmd::Flow { set } => fx.push(Effect::Flow { client: Some(client), token: None, set }),
            UserCmd::Help => fx.push(notice(client, HELP)),
            UserCmd::Invalid(e) => fx.push(notice(client, &e)),
        }
    }
}
