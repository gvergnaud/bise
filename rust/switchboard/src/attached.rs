//! His attached files on an input (bise desktop item H, architect m_9650 H):
//! `HubCmd::Send.files` (absolute paths he attached or dropped) rendered once
//! after his words, next to the fn context (`crate::fn_context`), on the
//! input path only (daemon/fn_context.rs step_input), never on a slash
//! command. Pure: what a path is comes from the caller's `look`, so the
//! shell does the file system and the image store, and the tests don't.
//!
//! - an image: its image-store marker (`bend_images::marker`), one per line,
//!   so the model sees the image itself, never a raw path it must open;
//! - any other file: listed by its path under `[files he attached:]`, for
//!   main to read with its tools;
//! - a path that isn't absolute, or that isn't a file: left out (the shell
//!   logs it).

/// What a path turned out to be.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Look {
    /// an image now in the store: its marker
    Image(String),
    /// a file, not an image
    File,
    /// not absolute, gone, or not a file
    Skip,
}

/// The header of the list of his other files.
pub const HEADER: &str = "[files he attached:]";

/// His words with his files after them (the words alone when none is kept).
pub fn render(text: &str, files: &[String], look: &mut dyn FnMut(&str) -> Look) -> String {
    let mut markers = Vec::new();
    let mut listed = Vec::new();
    // every path once, whatever it turns out to be: the same image twice
    // is one marker and one store call (architect m_9875)
    let mut seen: Vec<&str> = Vec::new();
    for f in files {
        let f = f.trim();
        if f.is_empty() || !f.starts_with('/') || seen.contains(&f) {
            continue;
        }
        seen.push(f);
        match look(f) {
            Look::Image(m) => markers.push(m),
            Look::File => listed.push(f.to_string()),
            Look::Skip => {}
        }
    }
    let mut parts: Vec<String> = Vec::new();
    if !text.trim().is_empty() {
        parts.push(text.to_string());
    }
    if !markers.is_empty() {
        parts.push(markers.join("\n"));
    }
    if !listed.is_empty() {
        parts.push(format!("{HEADER}\n{}", listed.join("\n")));
    }
    parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look(p: &str) -> Look {
        match p {
            "/w/a.png" => Look::Image("<image name=\"a.png\">".into()),
            "/w/notes.md" | "/w/q3.pdf" => Look::File,
            _ => Look::Skip,
        }
    }

    #[test]
    fn images_become_markers_and_other_files_a_list_after_his_words() {
        let files = ["/w/notes.md", "/w/a.png", "/w/q3.pdf"].map(String::from);
        assert_eq!(
            render("summarize these", &files, &mut look),
            "summarize these\n\n<image name=\"a.png\">\n\n[files he attached:]\n/w/notes.md\n/w/q3.pdf"
        );
    }

    #[test]
    fn a_relative_a_missing_or_a_repeated_path_is_left_out() {
        let files = ["notes.md", "/w/gone.txt", "/w/notes.md", "/w/notes.md", " "].map(String::from);
        assert_eq!(render("look", &files, &mut look), "look\n\n[files he attached:]\n/w/notes.md");
        assert_eq!(render("look", &["/w/gone.txt".to_string()], &mut look), "look");
        assert_eq!(render("look", &[], &mut look), "look");
        // the same image twice: one look (one store call), one marker
        let mut asked = Vec::new();
        let twice = render("", &["/w/a.png".to_string(), " /w/a.png".to_string()], &mut |p| {
            asked.push(p.to_string());
            look(p)
        });
        assert_eq!((twice.as_str(), asked.len()), ("<image name=\"a.png\">", 1));
    }

    #[test]
    fn files_without_words_are_the_whole_text() {
        assert_eq!(render("  ", &["/w/a.png".to_string()], &mut look), "<image name=\"a.png\">");
        let mut asked = Vec::new();
        render("x", &["rel".to_string(), "/w/q3.pdf".to_string()], &mut |p| {
            asked.push(p.to_string());
            look(p)
        });
        assert_eq!(asked, ["/w/q3.pdf"], "a relative path is never looked at");
    }
}
