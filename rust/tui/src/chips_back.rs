//! His message back in the composer (issue 23: ↑ takes a queued message
//! back from the hub): its typed parts (msg-parts) become the composer's
//! text with its chips again (quotes, pastes, images, files), numbered
//! from `first` on the images' one sequence. The reverse of
//! `attach::expand`: expanding what this gives sends the same parts.
//! Pure: the caller puts the text in the composer and the attachments
//! beside it. A message with no parts (an older one) is read from its
//! text once, as the fold does (`bise_proto::parts::parse`).

use crate::attach::{Attachment, Info};
use bise_proto::parts::{self, Part, NO_SPACE_BEFORE};

/// The composer's text and its attachments for `parts` (or `text` when
/// there are none), chips numbered from `first`.
pub(crate) fn of(parts: &[Part], text: &str, first: usize) -> (String, Vec<Attachment>) {
    let parsed;
    let parts = match parts.is_empty() {
        false => parts,
        true => {
            parsed = parts::parse(text);
            &parsed
        }
    };
    let mut out = String::new();
    let mut atts = Vec::new();
    let mut n = first;
    let mut after_chip = false;
    for p in parts {
        let chip = match p {
            Part::Text { text } => {
                // the chip's own space, dropped before a mark (expand)
                if after_chip && text.starts_with(NO_SPACE_BEFORE) {
                    out.push(' ');
                }
                out.push_str(text);
                after_chip = false;
                continue;
            }
            Part::Quote { from, text, file, new, old, .. } => {
                (crate::quote::label(n), parts::quote_tag(from, file, new, old, None, text), Info::default())
            }
            Part::Paste { text, .. } => (crate::pasted::label(n), crate::pasted::tag(n, text), Info::default()),
            Part::Image { source, mime, b64_file, caption, .. } => {
                let label = crate::attach::label(n);
                let image = Part::Image { name: label.clone(), source: source.clone(), mime: mime.clone(), b64_file: b64_file.clone(), caption: caption.clone() };
                (label, parts::render(&[image]), Info { source: source.clone(), ..Info::default() })
            }
            Part::File { path, missing, .. } => {
                let label = parts::file_label(n);
                let marker = parts::file_marker(path, *missing);
                crate::attach_file::name_chip(&label, path);
                (label, marker, Info { source: path.clone(), ..Info::default() })
            }
            // his notes on a page: their words; the hub's hints and a
            // newer client's parts are not his to edit
            Part::Page { text, .. } => {
                out.push_str(text);
                after_chip = false;
                continue;
            }
            // a part of an artifact he selected in its viewer: its tag,
            // as the model read it (only the desktop's panel makes one)
            Part::Anchor { .. } => {
                out.push_str(&parts::render(std::slice::from_ref(p)));
                after_chip = false;
                continue;
            }
            Part::Hint { .. } | Part::Forwarded { .. } | Part::ViewContext { .. } | Part::Unknown(_) => continue,
        };
        let (label, marker, info) = chip;
        out.push_str(&label);
        atts.push(Attachment { label, marker, info });
        n += 1;
        after_chip = true;
    }
    (out, atts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quote(text: &str, n: Option<u32>) -> Part {
        Part::Quote { from: "main".into(), text: text.into(), file: String::new(), new: String::new(), old: String::new(), n }
    }

    /// Law: what comes back expands to the same parts (the chips' numbers
    /// aside), so a message taken back and queued again says the same.
    #[test]
    fn the_chips_come_back_and_expand_to_the_same_parts() {
        let sent = vec![
            Part::Text { text: "about ".into() },
            quote("a\nb", Some(1)),
            Part::Text { text: " and ".into() },
            Part::Paste { n: 2, text: "long\npaste".into() },
            Part::Text { text: ", see ".into() },
            Part::Image { name: "[Image #3]".into(), source: "clipboard".into(), mime: "image/png".into(), b64_file: "/s/x.b64".into(), caption: String::new() },
        ];
        let (text, atts) = of(&sent, "", 1);
        assert_eq!(text, "about [Quote #1] and [Paste #2] , see [Image #3]");
        let mut app = crate::sb::bench::test_app();
        app.attachments = atts;
        let said = crate::attach::expand(&mut app, &text);
        assert_eq!(said.parts, parts::canonical(&sent));
    }

    /// Numbers start at `first` (the composer's own chips keep theirs).
    #[test]
    fn numbers_start_at_first() {
        let (text, atts) = of(&[quote("q", None), Part::Text { text: " why?".into() }], "", 3);
        assert_eq!(text, "[Quote #3] why?");
        assert_eq!(atts[0].label, "[Quote #3]");
    }

    /// An older message with no parts: read from its text, as the fold.
    #[test]
    fn an_old_message_is_read_from_its_text() {
        let text = format!("{}\nwhy?", crate::quote::tag("main", "q"));
        let (back, atts) = of(&[], &text, 1);
        assert!(back.starts_with("[Quote #1]") && back.ends_with("why?"), "{back}");
        assert_eq!(atts.len(), 1);
        assert_eq!(of(&[], "plain words", 1), ("plain words".to_string(), Vec::new()));
    }
}
