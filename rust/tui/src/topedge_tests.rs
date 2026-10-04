use super::*;
use crate::theme::text;

fn plain(t: String, st: Style) -> Span<'static> {
    Span::styled(t, st)
}

fn art(title: &str, agent: &str) -> Artifact {
    Artifact { id: title.replace(' ', "-"), title: title.into(), agent: agent.into(), by: agent.into(), ..Default::default() }
}

fn s(spans: &[Span]) -> String {
    spans.iter().map(|x| x.content.as_ref()).collect()
}

fn forms(new: u64, rows: &[Artifact]) -> Vec<String> {
    notice(new, rows, &plain).iter().map(|f| s(&f.spans)).collect()
}

/// The edge as text: what follows the logo, then what ends the edge.
fn edge(room: usize, paths: &[String], role: &str, counts: &str, notice: &[Form]) -> (String, String) {
    let role = if role.is_empty() { Vec::new() } else { vec![Span::raw(format!(" · {}", role))] };
    let c = counts.to_string();
    let counts = move |room: usize| {
        // a count shortens like the real ones: `# 1 in the inbox` → `# 1`
        let short = c.split(" in the").next().unwrap_or("").to_string();
        if c.is_empty() {
            Vec::new()
        } else if c.width() <= room {
            vec![Span::raw(c.clone())]
        } else if short.width() <= room {
            vec![Span::raw(short)]
        } else {
            Vec::new()
        }
    };
    let (l, r) = lay(room, paths, role, counts, notice);
    let (l, r) = (s(&l), s(&r));
    assert!(l.width() + r.width() <= room, "{l:?} {r:?} in {room}");
    (l, r)
}

fn acme() -> Vec<String> {
    paths("/w/lab/acme", "")
}

#[test]
fn the_notice_says_who_and_what() {
    assert!(forms(0, &[art("pricing page", "designer")]).is_empty(), "nothing new: no notice");
    assert_eq!(
        forms(1, &[art("features update (draft)", "designer")]),
        ["↗ designer · features update (draft)", "↗ 1 new artifact", "↗ 1 new", "↗ 1"]
    );
    // several: the makers, newest first, each once
    let rows = [art("a", "designer"), art("b", "art-demo"), art("c", "designer")];
    assert_eq!(
        forms(3, &rows),
        ["↗ 3 new artifacts · designer, art-demo", "↗ 3 new artifacts · designer +1", "↗ 3 new artifacts", "↗ 3 new", "↗ 3"]
    );
    // one maker: no `+0`
    assert_eq!(forms(2, &rows[..1].iter().chain(&rows[2..]).cloned().collect::<Vec<_>>())[..2], ["↗ 2 new artifacts · designer", "↗ 2 new artifacts"]);
    // no maker known (the list not there yet): the count in words
    assert_eq!(forms(2, &[]), ["↗ 2 new artifacts", "↗ 2 new", "↗ 2"]);
    let mine = Artifact { agent: String::new(), by: "page".into(), ..art("plan", "") };
    assert_eq!(forms(1, &[mine])[0], "↗ plan");
    // the arrow and the words in accent, the separator dim
    let f = &notice(1, &[art("pricing page", "designer")], &plain)[0];
    assert_eq!(f.spans.iter().map(|x| x.style.fg).collect::<Vec<_>>(), [Some(accent()), Some(accent()), Some(dim()), Some(accent())]);
}

#[test]
fn the_path_has_a_short_form() {
    assert_eq!(acme(), ["/w/lab/acme", "acme"]);
    assert_eq!(paths("/w/lab/acme", "lands via PRs"), ["/w/lab/acme · lands via PRs", "/w/lab/acme", "acme"]);
    assert!(paths("", "").is_empty());
    if let Ok(h) = std::env::var("HOME") {
        assert_eq!(paths(&format!("{h}/lab/acme"), "")[0], "~/lab/acme");
    }
}

#[test]
fn wide_the_edge_says_everything() {
    let n = notice(1, &[art("features update (draft)", "designer")], &plain);
    let (l, r) = edge(140, &acme(), "", "# 1 in the inbox", &n);
    assert_eq!(l, " · /w/lab/acme");
    assert_eq!(r, "# 1 in the inbox · ↗ designer · features update (draft)");
    // nothing new: the counts alone
    let (l, r) = edge(140, &acme(), "", "# 1 in the inbox", &[]);
    assert_eq!((l.as_str(), r.as_str()), (" · /w/lab/acme", "# 1 in the inbox"));
    // no counts: no separator before the notice
    let (_, r) = edge(140, &acme(), "", "", &n);
    assert_eq!(r, "↗ designer · features update (draft)");
}

/// Short on room: the path shortens before the notice loses its words,
/// the title is cut, then the notice keeps only the count in words; the
/// path goes before `1 new`, `1`; the counts last.
#[test]
fn short_on_room_the_edge_drops_in_order() {
    let n = notice(1, &[art("features update (draft)", "designer")], &plain);
    let at = |room| edge(room, &acme(), "", "# 1 in the inbox", &n);
    let full = " · /w/lab/acme".width() + "# 1 in the inbox · ↗ designer · features update (draft)".width();
    assert_eq!(at(full).0, " · /w/lab/acme");
    assert_eq!(at(full - 1), (" · acme".into(), "# 1 in the inbox · ↗ designer · features update (draft)".into()));
    // the title cut (with the long path back when it fits)
    let cut = at(full - 8);
    assert_eq!(cut, (" · /w/lab/acme".into(), "# 1 in the inbox · ↗ designer · features updat…".into()));
    // the title at its shortest, then the words
    let (l, r) = at(" · acme".width() + "# 1 in the inbox · ↗ designer · ".width() + 6);
    assert_eq!((l.as_str(), r.as_str()), (" · acme", "# 1 in the inbox · ↗ designer · featu…"));
    // who before the long path: a cut title beats `1 new artifact`
    let (l, r) = at(" · /w/lab/acme".width() + "# 1 in the inbox · ↗ 1 new artifact".width());
    assert_eq!((l.as_str(), r.as_str()), (" · acme", "# 1 in the inbox · ↗ designer · features …"));
    let (l, r) = at(" · acme".width() + "# 1 in the inbox · ↗ designer · ".width() + 5);
    assert_eq!((l.as_str(), r.as_str()), (" · acme", "# 1 in the inbox · ↗ 1 new artifact"));
    // the path goes, the notice shortens
    assert_eq!(at("# 1 in the inbox · ↗ 1 new artifact".width()), (String::new(), "# 1 in the inbox · ↗ 1 new artifact".into()));
    assert_eq!(at("# 1 in the inbox · ↗ 1 new".width()), (String::new(), "# 1 in the inbox · ↗ 1 new".into()));
    assert_eq!(at("# 1 in the inbox · ↗ 1".width()), (String::new(), "# 1 in the inbox · ↗ 1".into()));
    // then the counts shorten, the bare notice after them
    assert_eq!(at("# 1 · ↗ 1".width() + 3), (String::new(), "# 1 · ↗ 1".into()));
    assert_eq!(at(4).1, "# 1");
}

#[test]
fn several_new_ones_drop_their_names_one_by_one() {
    let rows = [art("a", "designer"), art("b", "art-demo"), art("c", "launch")];
    let n = notice(3, &rows, &plain);
    let at = |room| edge(room, &acme(), "", "", &n);
    let all = "↗ 3 new artifacts · designer, art-demo, launch";
    assert_eq!(at(" · /w/lab/acme".width() + all.width()).1, all);
    assert_eq!(at(" · acme".width() + all.width()), (" · acme".into(), all.into()));
    assert_eq!(at(" · /w/lab/acme".width() + "↗ 3 new artifacts · designer +2".width()).1, "↗ 3 new artifacts · designer +2");
    assert_eq!(at(" · acme".width() + "↗ 3 new artifacts".width()), (" · acme".into(), "↗ 3 new artifacts".into()));
    assert_eq!(at("↗ 3 new".width()), (String::new(), "↗ 3 new".into()));
}

/// The role line (a task's view) keeps up to ROLE_KEEP columns before the
/// path and the notice shorten for it, then ROLE_MIN; then it goes.
#[test]
fn the_role_line_follows_the_path() {
    let n = notice(1, &[art("pricing page", "designer")], &plain);
    let role = "fixing the safari login on the checkout page";
    let right = "# 1 in the inbox · ↗ designer · pricing page";
    let (l, r) = edge(200, &acme(), role, "# 1 in the inbox", &n);
    assert_eq!(l, format!(" · /w/lab/acme · {role}"));
    assert_eq!(r, right);
    // the role is cut to ROLE_KEEP while the rest stays whole
    let (l, r) = edge(" · /w/lab/acme".width() + ROLE_KEEP + right.width(), &acme(), role, "# 1 in the inbox", &n);
    assert_eq!(r, right);
    assert_eq!(l.width(), " · /w/lab/acme".width() + ROLE_KEEP);
    assert!(l.ends_with('…'), "{l}");
    // one less: the path shortens, the role keeps its room
    let (l, _) = edge(" · /w/lab/acme".width() + ROLE_KEEP + right.width() - 1, &acme(), role, "# 1 in the inbox", &n);
    assert!(l.starts_with(" · acme · fixing"), "{l}");
    // very short: no role, the counts and a notice
    let (l, r) = edge(30, &acme(), role, "# 1 in the inbox", &n);
    assert!(!l.contains("fixing"), "{l}");
    assert!(r.starts_with("# 1"), "{r}");
}

#[test]
fn a_full_screen_says_the_path_then_its_name() {
    let head = |room| s(&screen_head(room, &acme(), "artifacts"));
    assert_eq!(head(80), " · /w/lab/acme ── artifacts");
    assert_eq!(head(" · /w/lab/acme ── artifacts".width() - 1), " · acme ── artifacts");
    assert_eq!(head(10), " ── artifacts");
    let spans = screen_head(80, &acme(), "artifacts");
    assert_eq!(spans.last().unwrap().style.fg, Some(text()));
}
