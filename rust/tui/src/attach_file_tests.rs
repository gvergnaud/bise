//! File chips in the composer and the history (file-chips).

use super::*;
use bise_proto::parts::Part;

fn composer(text: &str, at: usize) -> App {
    let mut app = crate::sb::bench::test_app();
    app.ed.paste(text);
    app.ed.cursor = at;
    app
}

/// A folder with the five documents of the brief and a code file.
fn docs(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("file-chips-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for f in ["q3.pdf", "data.csv", "plan.xlsx", "brief.docx", "deck.pptx", "main.rs"] {
        std::fs::write(dir.join(f), "x".repeat(2100)).unwrap();
    }
    dir
}

/// A drop of document paths: one file chip each, in the images' number
/// sequence, at the cursor; the strip names each with its size and
/// folder; a paste that holds a word that is not a file stays text.
#[test]
fn a_drop_of_files_makes_file_chips() {
    let dir = docs("drop");
    let mut app = composer("", 0);
    let paths: Vec<String> = ["q3.pdf", "data.csv", "plan.xlsx", "brief.docx", "deck.pptx"].iter().map(|f| dir.join(f).to_string_lossy().to_string()).collect();
    let labels = on_paste(&mut app, &paths.join(" ")).unwrap().unwrap();
    // each chip says its file's name (designer m_27881), the box's row its number
    assert_eq!(labels, ["▯ q3.pdf", "▯ data.csv", "▯ plan.xlsx", "▯ brief.docx", "▯ deck.pptx"]);
    assert_eq!(app.ed.text, "[File #1] [File #2] [File #3] [File #4] [File #5] ");
    assert_eq!(shown(&app).len(), 5);
    assert_eq!(attach::strip_height(&app), 7);
    let src = strip_source(&app.attachments[4]);
    assert!(src.starts_with("2 kB  "), "{src}");
    assert_eq!(path_of(&app.attachments[4]).as_deref(), Some(paths[4].as_str()));
    // a word that isn't a file: text
    assert!(on_paste(&mut app, &format!("{} hello", paths[0])).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The brief's example: an image chip and a file chip in one sentence
/// reach the parts each at its place.
#[test]
fn the_sentence_keeps_each_chip_at_its_place() {
    let dir = docs("send");
    let mut app = composer("ajouter l'image ", 16);
    app.attachments.push(Attachment {
        label: "[Image #1]".into(),
        marker: parts::image_marker("[Image #1]", "/u/logo.png", "image/png", "/h/images/1.b64", ""),
        info: Info::default(),
    });
    attach::insert_chip(&mut app.ed, "[Image #1]");
    let at = app.ed.text.chars().count();
    app.ed.set(&format!("{}dans la presentation powerpoint ", app.ed.text), at + 32);
    let deck = dir.join("deck.pptx");
    attach_path(&mut app, &deck);
    let text = app.ed.text.clone();
    let said = attach::expand(&mut app, &text);
    let deck = deck.to_string_lossy().to_string();
    assert_eq!(
        said.parts,
        vec![
            Part::Text { text: "ajouter l'image ".into() },
            Part::Image { name: "[Image #1]".into(), source: "/u/logo.png".into(), mime: "image/png".into(), b64_file: "/h/images/1.b64".into(), caption: String::new() },
            Part::Text { text: " dans la presentation powerpoint ".into() },
            Part::File { path: deck.clone(), name: "[File #2]".into(), missing: false },
            Part::Text { text: " ".into() },
        ]
    );
    assert!(said.text.contains(&format!("powerpoint [deck.pptx]({deck})")), "{}", said.text);
    let _ = std::fs::remove_dir_all(&dir);
}

/// at-chips: an `@` pick of code is a file chip where the `@token` was,
/// the cursor right after it, the words after it kept; the model reads
/// the file's absolute path at that place. A path that is no file stays
/// the pick's text.
#[test]
fn an_at_pick_of_code_is_a_chip_at_the_at() {
    let dir = docs("pick");
    let mut app = composer("", 0);
    crate::sb::bench::set_workspace(&mut app, &dir.to_string_lossy());
    app.ed.set("look @mai then fix it", 9);
    assert_eq!(pick(&mut app, "main.rs").as_deref(), Some("▯ main.rs"));
    assert_eq!(attach::chip_text("[File #1]").trim(), "▯ main.rs");
    assert_eq!(app.ed.text, "look [File #1] then fix it");
    assert_eq!(app.ed.cursor, "look [File #1] ".chars().count());
    let text = app.ed.text.clone();
    let said = attach::expand(&mut app, &text);
    let main = dir.join("main.rs").to_string_lossy().to_string();
    assert_eq!(
        said.parts,
        vec![Part::Text { text: "look ".into() }, Part::File { path: main.clone(), name: "[File #1]".into(), missing: false }, Part::Text { text: " then fix it".into() }]
    );
    assert!(said.text.contains(&format!("look [main.rs]({main}) then fix it")), "{}", said.text);
    app.ed.set("and @gone", 9);
    assert!(pick(&mut app, "gone.rs").is_none(), "no such file: stays text");
    let _ = std::fs::remove_dir_all(&dir);
}

/// In the history: `▯ deck.pptx`, a link to its file; a missing one says
/// so after it, only `missing` in the error tone.
#[test]
fn the_history_draws_a_file_chip_and_says_missing() {
    let text = format!("see {} now", parts::file_marker("/u/Desk top/deck.pptx", false));
    let spans = attach::chip_spans(&text, Style::default());
    let words: String = spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(words, "see ▯\u{a0}deck.pptx now");
    let (_, urls) = crate::links::collect(|| attach::chip_spans(&text, Style::default()));
    assert_eq!(urls, vec!["bise-file:/u/Desk top/deck.pptx".to_string()]);
    let gone = parts::file_marker("/u/q3.csv", true);
    let spans = attach::chip_spans(&gone, Style::default());
    let words: String = spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(words, "▯\u{a0}q3.csv · missing");
    assert_eq!(spans.last().unwrap().style.fg, Some(crate::theme::error()));
}
