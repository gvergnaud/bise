//! The settings area's UI words (see [`super`]: the rules, how to read
//! them): settings, setup and onboarding, models and providers, plugins,
//! updates. Split in parts (architect m_27109), each a `words! { settings: }`
//! block, so a key is `settings.<name>` whatever its part.

use super::Word;

mod accounts;
mod first;
mod gauge;
mod keys;
mod pages;
mod plugins;
mod projects;
mod providers;
mod roles;
mod setup;
mod tui;
mod update;

pub use accounts::*;
pub use first::*;
pub use gauge::*;
pub use keys::*;
pub use pages::*;
pub use plugins::*;
pub use projects::*;
pub use providers::*;
pub use roles::*;
pub use setup::*;
pub use tui::*;
pub use update::*;

const PARTS: &[&[Word]] = &[pages::WORDS, accounts::WORDS, projects::WORDS, update::WORDS, keys::WORDS, setup::WORDS, first::WORDS, gauge::WORDS, providers::WORDS, roles::WORDS, tui::WORDS, plugins::WORDS];

/// This area's words, in order (bise_proto::words::ALL).
pub const WORDS: &[Word] = &super::concat::<{ super::count(PARTS) }>(PARTS);
