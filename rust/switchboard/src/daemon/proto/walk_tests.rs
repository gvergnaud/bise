//! The transcript walker (walk.rs): find forward, marks backward.
use super::*;
use std::io::Write;

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sb-walk-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A fold for the test: an entry per line starting 'E' (an agent's) or 'Y'
/// (his), the lines after it its text.
fn fold(lines: &[Line]) -> Vec<pthread::Entry> {
    let mut out: Vec<pthread::Entry> = Vec::new();
    for (pos, ts, l) in lines {
        let start = l.strip_prefix('E').map(|t| ("agent", t)).or_else(|| l.strip_prefix('Y').map(|t| ("you", t)));
        match (start, out.last_mut()) {
            (Some((kind, t)), _) => out.push(serde_json::from_value(serde_json::json!({"pos": pos, "at_ms": ts, "kind": kind, "text": t})).unwrap()),
            (None, Some(e)) => {
                e.text.push('\n');
                e.text.push_str(l);
            }
            (None, None) => {}
        }
    }
    out
}

#[test]
fn finds_every_entry_across_chunks_as_one_fold_would() {
    let d = scratch("chunks");
    let p = d.join("t.log");
    let mut f = std::fs::File::create(&p).unwrap();
    let mut want = Vec::new();
    let mut pos = 0;
    for e in 0..6000 {
        pos += 1;
        let start = pos;
        writeln!(f, "1\tE entry {e}").unwrap();
        // entries of 1 to 4 lines; the word in the last line of every 97th
        for k in 0..(e % 4) {
            pos += 1;
            writeln!(f, "1\tline {k}{}", if e % 97 == 0 && k + 1 == e % 4 { " kumquat" } else { "" }).unwrap();
        }
        if e % 97 == 0 && e % 4 > 0 {
            want.push(start as u64);
        }
    }
    drop(f);
    let ix = Indexes::default();
    let (got, upto) = search(&ix, &p, "KUMQUAT", &fold, &|| false).unwrap();
    assert_eq!(upto, pos as u64);
    assert_eq!(got.iter().map(|h| h.pos).collect::<Vec<_>>(), want);
    assert!(got.iter().all(|h| h.n == 1));
}

#[test]
fn a_newer_find_stops_it() {
    let d = scratch("stop");
    let p = d.join("t.log");
    std::fs::write(&p, "1\tE kumquat\n").unwrap();
    assert!(search(&Indexes::default(), &p, "kumquat", &fold, &|| true).is_none());
}

#[test]
fn a_single_long_entry_is_folded_whole() {
    let d = scratch("long");
    let p = d.join("t.log");
    let mut f = std::fs::File::create(&p).unwrap();
    writeln!(f, "1\tE first").unwrap();
    for _ in 0..(CHUNK * 2) {
        writeln!(f, "1\tmore").unwrap();
    }
    writeln!(f, "1\tthe kumquat at its end").unwrap();
    writeln!(f, "1\tE second").unwrap();
    drop(f);
    let (got, _) = search(&Indexes::default(), &p, "kumquat", &fold, &|| false).unwrap();
    assert_eq!(got, vec![FindHit { pos: 1, n: 1 }]);
}

/// A transcript of `n` entries of 1 to 4 lines, his every `every`th; his
/// entries' (pos, first words), oldest first.
fn his_transcript(p: &Path, n: usize, every: usize) -> Vec<(u64, String)> {
    let mut f = std::fs::File::create(p).unwrap();
    let mut his = Vec::new();
    let mut pos = 0u64;
    for e in 0..n {
        pos += 1;
        let mine = e % every == 0;
        writeln!(f, "{pos}\t{}ask {e}", if mine { "Y" } else { "E" }).unwrap();
        if mine {
            his.push((pos, format!("ask {e}")));
        }
        for k in 0..(e % 4) {
            pos += 1;
            writeln!(f, "{pos}\tline {k}").unwrap();
        }
    }
    his
}

#[test]
fn marks_are_his_last_ones_oldest_first_each_whole() {
    let d = scratch("marks");
    let p = d.join("t.log");
    let his = his_transcript(&p, 20_000, 113);
    let (got, upto) = marks(&Indexes::default(), &p, 20, &fold);
    assert!(upto > 0);
    let want = &his[his.len() - 20..];
    assert_eq!(got.iter().map(|m| m.pos).collect::<Vec<_>>(), want.iter().map(|h| h.0).collect::<Vec<_>>());
    // each read whole across the chunk edges: its lines folded on one line, its time its first line's
    for (m, (pos, words)) in got.iter().zip(want) {
        assert!(m.text.starts_with(words.as_str()), "{} vs {words}", m.text);
        assert_eq!(m.at_ms, *pos);
        let lines = words.trim_start_matches("ask ").parse::<usize>().unwrap() % 4;
        assert_eq!(m.text.matches("line").count(), lines, "{}", m.text);
    }
}

#[test]
fn marks_fewer_than_asked_reads_to_the_start_and_caps_the_limit() {
    let d = scratch("few");
    let p = d.join("t.log");
    let his = his_transcript(&p, 9000, 4000);
    let (got, _) = marks(&Indexes::default(), &p, 20, &fold);
    assert_eq!(got.iter().map(|m| m.pos).collect::<Vec<_>>(), his.iter().map(|h| h.0).collect::<Vec<_>>());
    let all = his_transcript(&p, 500, 1);
    let (got, _) = marks(&Indexes::default(), &p, 10_000, &fold);
    assert_eq!(got.len(), MAX_MARKS);
    assert_eq!(got.last().map(|m| m.pos), all.last().map(|h| h.0));
    let (none, upto) = marks(&Indexes::default(), &d.join("none.log"), 20, &fold);
    assert!(none.is_empty() && upto == 0);
}

#[test]
fn marks_stop_once_they_have_them() {
    // his last 3 in the last chunk: the walk never folds the chunks before it
    let d = scratch("stops");
    let p = d.join("t.log");
    his_transcript(&p, 50_000, 7);
    let folded = std::cell::Cell::new(0);
    let counting = |lines: &[Line]| {
        folded.set(folded.get() + lines.len());
        fold(lines)
    };
    let (got, _) = marks(&Indexes::default(), &p, 3, &counting);
    assert_eq!(got.len(), 3);
    assert_eq!(folded.get(), CHUNK);
}

/// By hand, on a copy of a real transcript (msg-dots' measure):
/// `BISE_MARKS_BENCH=<file> cargo test -p switchboard --release --lib marks_bench -- --ignored --nocapture`.
#[test]
#[ignore]
fn marks_bench() {
    let Ok(p) = std::env::var("BISE_MARKS_BENCH") else { return };
    let p = PathBuf::from(p);
    let page = |_: &str| None;
    let fold = |lines: &[Line]| {
        let ctx = Ctx { open_cards: &[], page: &page, provider: &|i: &str, _: &str| i.to_string(), width: &width, offset: &offset, attached: &crate::attached::split };
        pthread::fold(lines, &ctx)
    };
    let ix = Indexes::default();
    let t0 = std::time::Instant::now();
    ix.build(&p);
    let built = t0.elapsed();
    for limit in [20, 50] {
        let t = std::time::Instant::now();
        let (m, upto) = marks(&ix, &p, limit, &fold);
        let first = m.first().map_or(0, |m| m.pos);
        eprintln!("marks {limit}: {} in {:?} (index {:?}); lines {upto}, oldest mark at {first}", m.len(), t.elapsed(), built);
    }
}
