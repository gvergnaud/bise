use super::*;

fn vals(kv: &[(&str, bool)]) -> Values {
    kv.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

/// The precedence, one row per level: env > config > default.
#[test]
fn resolve_takes_the_first_source_that_names_the_flag() {
    let full = Inputs { env: vals(&[("test", false)]), config: vals(&[("test", true)]) };
    assert_eq!(resolve(Flag::Test, &full), (false, Source::Env));
    let no_env = Inputs { env: Values::new(), ..full.clone() };
    assert_eq!(resolve(Flag::Test, &no_env), (true, Source::Config));
    let other = Inputs { config: vals(&[("other", true)]), ..Inputs::default() };
    assert_eq!(resolve(Flag::Test, &other), (false, Source::Default), "another name never counts");
    assert_eq!(on(&no_env), vec![Flag::Test]);
    assert_eq!(on(&Inputs::default()), Vec::<Flag>::new());
}

#[test]
fn an_unknown_name_is_no_flag() {
    assert_eq!(Flag::of_name("later_thing"), None);
    assert_eq!(Flag::of_name("test"), Some(Flag::Test));
}

#[test]
fn bise_flag_reads_a_list_of_name_value() {
    let (v, bad) = parse_env(" test=on, other=OFF,x=1 ,y=0,,z, w=maybe ,=on");
    assert_eq!(v, vals(&[("test", true), ("other", false), ("x", true), ("y", false)]));
    assert_eq!(bad, vec!["z", "w=maybe", "=on"]);
    assert_eq!(parse_env(""), (Values::new(), vec![]));
    assert_eq!(Inputs::read(Some("test=on"), Values::new()).env, vals(&[("test", true)]));
}
