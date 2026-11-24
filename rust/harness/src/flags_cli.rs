//! `bise flags` (docs/flags.md): every feature flag of this build, what it
//! reads as on this Mac and where that comes from (BISE_FLAG, the
//! `[flags]` table of config.toml, the default). Read only: it never
//! writes config.toml (his file: `[flags]`, `name = true`).

use bise_home::flags::{self, Raw};
use bise_home::Home;
use bise_proto::flags::{resolve, Flag, Inputs};

const USAGE: &str = "bise flags: the feature flags of this bise (docs/flags.md)

  bise flags        each flag, on or off, and where that comes from

  Turn one on: BISE_FLAG=\"name=on\", or in ~/.bise/config.toml:
    [flags]
    name = true";

/// What `bise flags` prints for these reads and flags (pure: the tests' table).
pub fn render(raw: &Raw, all: &[Flag]) -> String {
    let i = Inputs::read(raw.env.as_deref(), raw.config.clone());
    let width = all.iter().map(|f| f.name().len()).max().unwrap_or(0);
    let mut out: Vec<String> = all
        .iter()
        .map(|f| {
            let (on, src) = resolve(*f, &i);
            format!("{:width$}  {:3}  {}  · {}", f.name(), if on { "on" } else { "off" }, src.word(), f.info().note)
        })
        .collect();
    if all.is_empty() {
        out.push("no flags: every feature of this bise is released".into());
    }
    out.extend(raw.config_bad.iter().cloned());
    if let Some((_, bad)) = raw.env.as_deref().map(bise_proto::flags::parse_env) {
        out.extend(bad.iter().map(|b| format!("BISE_FLAG: '{b}' is not name=on|off, skipped")));
    }
    out.join("\n")
}

pub fn main(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        None | Some("show") => {
            let raw = flags::read(&Home::from_env(), bise_home::env::test_setting(flags::ENV_VAR));
            println!("{}", render(&raw, Flag::ALL));
            0
        }
        Some("-h" | "--help" | "help") => {
            println!("{USAGE}");
            0
        }
        Some(other) => {
            eprintln!("bise flags: unknown '{other}'\n\n{USAGE}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_flag_today_says_so_and_still_names_a_bad_line() {
        let mut raw = Raw::default();
        assert_eq!(render(&raw, Flag::ALL), "no flags: every feature of this bise is released");
        raw.config_bad.push("config.toml: flags.odd = \"yes\" is not true or false, skipped".into());
        raw.env = Some("x=on,zz".into());
        let t = render(&raw, Flag::ALL);
        assert!(t.contains("flags.odd") && t.contains("BISE_FLAG: 'zz' is not name=on|off"), "{t}");
    }
}
