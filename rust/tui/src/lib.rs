//! bend-tui — the Switchboard terminal UI (ratatui): the client of a
//! workspace's hub (`sb::run_switchboard`), one feed per agent.
//!
//! Born as a Rust port of repl-tui (Ink). The presentation is
//! modeled on the REAL OpenCode TUI (packages/tui in the opencode repo):
//! no header bar — the screen is the conversation. Blocks breathe: a
//! blank line at every content transition, one column of margin on
//! each edge of the feed, blank rows separating the history from the
//! composer, and the user block paints its panel background the full
//! column. Status speaks in glyphs, not words: ✦ reasoning, ✓ ok,
//! ✗ fail, ▲ warning, ⟳ compaction, ≡ summary, ↳ preview. User
//! messages are
//! blocks with a colored left bar and a panel background; assistant
//! markdown renders in the OpenCode markdown colors; tool calls are
//! OpenCode inline tools (braille spinner while running, muted ✓ once
//! done, red ✗ on failure); the prompt is an OpenCode prompt (left
//! border, element background, agent/model meta row); commands filter
//! in an OpenCode autocomplete popup (split border, primary selection).
//! The status row carries the spinner + ctrl+c-to-interrupt hints.
//!
//! When stdin/stdout is not a TTY (piped), it falls back to line mode
//! (`sb/client.rs`) so the UI stays scriptable.


// the tests run on a temp HOME, never the user's (bise_home::test_home)
bise_home::test_home!();

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::io::{self, IsTerminal, Write};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

mod theme;
use theme::*;
mod theme_detect;
mod when;
mod wire;
use wire::*;
mod markdown;
use markdown::*;
mod code;
mod codeblock;
mod syntax;
mod mdlive;
mod sanitize;
mod render;
mod answered;
mod toolbox;
mod toolrow;
mod release_row;
use render::*;
mod feed;
use feed::*;
mod app;
use app::*;
mod commands;
use commands::*;
mod ui;
use ui::*;
mod input;
use input::*;
mod run;
use run::*;
mod sb;
mod skills;
mod files;
mod plugins;
mod emoji;
mod editor;
mod undo;
mod clipboard;
mod attach;
mod quote;
mod pasted;
mod usage;
mod models;
mod term;
mod feedsel;
mod find;
mod find_bar;
mod links;
mod textlayer;
mod pointer;
mod file_links;
mod keyprobe;
pub mod timing;
mod crash;
mod termtitle;
mod resign;
mod help;
mod approvals_screen;
mod logview;
mod artifacts;
mod artifacts_screen;
mod scheduled;
mod scheduled_screen;
mod diffbranches;
mod diffquote;
mod diffview;
mod computer_use;
mod keybar;
mod keycheck;
pub(crate) mod scan;
mod onboarding;
mod hints;
mod tour;
mod ctrlhint;
mod reach;
mod queue;
mod layout;
mod chrome;
mod topedge;
mod gust;
mod anim;
mod zen;
mod voice;
mod voicemode;
#[cfg(test)]
mod voice_ui_tests;
#[cfg(test)]
mod composer_wrap_tests;
#[cfg(test)]
mod feed_render_tests;
#[cfg(test)]
mod quiet_send_tests;
pub use keyprobe::keyprobe;
pub use sb::{run_switchboard, setup_main, take_reexec, take_refused};
pub use keycheck::check_model;
pub use crash::install as install_crash_hook;

#[cfg(test)]
mod at_popup_tests;
#[cfg(test)]
mod fuzz_tests;
#[cfg(test)]
mod editor_undo_tests;
#[cfg(test)]
mod links_tests;
#[cfg(test)]
mod file_links_tests;
#[cfg(test)]
mod cmd_a_tests;
