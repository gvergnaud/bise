//! Where a boot from a photo spends its time (hub-loop, main m_61302: a
//! boot took 105 s at load 60 and failed the switch's wait). Run by hand
//! on a copy of a hub's journal and photo, never on a live state folder:
//! `BOOT_BENCH_DIR=<dir with journal.jsonl and <key>.json>
//! BOOT_BENCH_CORE=<the sb-core that wrote the photo> cargo test -p
//! switchboard --lib boot_bench -- --ignored --nocapture`.

use super::*;

#[test]
#[ignore]
fn boot_bench() {
    let (Ok(dir), Ok(core)) = (std::env::var("BOOT_BENCH_DIR"), std::env::var("BOOT_BENCH_CORE")) else { return };
    let dir = PathBuf::from(dir);
    let t = std::time::Instant::now();
    let lap = |what: &str, t: &mut std::time::Instant| {
        eprintln!("{what}: {:?}", t.elapsed());
        *t = std::time::Instant::now();
    };
    let mut t0 = t;
    let read = journal_file::read(&dir.join("journal.jsonl"));
    let events = read.events;
    lap(&format!("journal read ({} events)", events.len()), &mut t0);
    let key = crate::photo::key_of(Path::new(&core)).unwrap();
    let photo = crate::photo::read(&dir, &key);
    lap("photo read", &mut t0);
    let (head, slices) = photo.expect("a photo of this sb-core in the dir");
    let (mark, at) = crate::photo::Mark::of_and_at(&events, head.mark.events, &mut |_, _| {});
    lap(&format!("one pass mark ({})", mark.events), &mut t0);
    let k = match crate::photo::usable_at(Some(&head), &key, events.len(), &at, false) {
        crate::photo::Use::Photo { k } => k,
        crate::photo::Use::Full { why } => panic!("photo not usable: {why}"),
    };
    lap(&format!("usable (k {k})"), &mut t0);
    let mut hub = Hub::with_core(&dir.to_string_lossy(), PathBuf::from(&core)).unwrap();
    lap("sb-core start", &mut t0);
    let restored = hub.restore(&slices, &mut |_, _| {}).unwrap();
    lap(&format!("restore {} slices: {:?}", slices.len(), restored.err()), &mut t0);
    let skipped = hub.replay_with(&events[k..], &mut |_, _| {}).unwrap();
    lap(&format!("replay {} events ({} skipped)", events.len() - k, skipped.len()), &mut t0);
    eprintln!("total {:?}", t.elapsed());
}
