use super::*;

fn said<'a>(from: &'a str, to: &'a str, text: &'a str, parts: &'a str) -> Option<Said<'a>> {
    Some(Said { from, to, text, parts })
}

#[test]
fn an_attach_is_a_message_id_or_an_absolute_path() {
    assert_eq!(want("m_12"), Ok(Want::Msg(12)));
    assert_eq!(want("/tmp/a.png"), Ok(Want::Path("/tmp/a.png".into())));
    assert!(want("a.png").unwrap_err().contains("absolute"));
}

#[test]
fn a_path_is_a_store_image_an_image_or_a_file() {
    let store = Path::new("/h/images");
    assert_eq!(kind("/h/images/abc.b64", store), Kind::StoreB64);
    // a .b64 elsewhere is just a file
    assert_eq!(kind("/x/abc.b64", store), Kind::File);
    assert_eq!(kind("/x/shot.PNG", store), Kind::Image);
    assert_eq!(kind("/x/deck.pptx", store), Kind::File);
    assert_eq!(kind("/x/main.rs", store), Kind::File);
}

#[test]
fn m_n_forwards_only_a_message_the_sender_got_or_sent() {
    let parts = r#"[{"kind":"text","text":"look "},{"kind":"image","name":"[Image #1]","source":"/u/s.png","mime":"image/png","b64_file":"/h/images/1.b64"},{"kind":"file","path":"/u/deck.pptx","name":"[File #2]"}]"#;
    let got = forwarded("main", 7, said("user", "main", "", parts)).unwrap();
    assert_eq!(
        got,
        vec![
            Item::Image { source: "/u/s.png".into(), mime: "image/png".into(), b64: "/h/images/1.b64".into() },
            Item::File { path: "/u/deck.pptx".into() },
        ]
    );
    // its sender may forward it too
    assert!(forwarded("user", 7, said("user", "main", "", parts)).is_ok());
    // another agent's message: refused
    assert!(forwarded("docs", 7, said("user", "main", "", parts)).unwrap_err().contains("not a message you got"));
    // unknown or no longer kept: refused with words, never its text alone
    assert!(forwarded("main", 7, None).unwrap_err().contains("too old to forward"));
    // nothing to forward
    assert!(forwarded("main", 7, said("user", "main", "hi", "")).unwrap_err().contains("no image or file"));
}

#[test]
fn m_n_without_parts_reads_them_from_its_text() {
    let marker = parts::image_marker("[Image #1]", "/u/s.png", "image/png", "/h/images/1.b64", "");
    let text = format!("see {marker}");
    let got = forwarded("main", 3, said("user", "main", &text, "")).unwrap();
    assert_eq!(got, vec![Item::Image { source: "/u/s.png".into(), mime: "image/png".into(), b64: "/h/images/1.b64".into() }]);
}

#[test]
fn a_file_in_the_senders_tmp_goes_to_the_receivers() {
    let (a, b) = (Path::new("/h/agents/a/tmp"), Path::new("/h/agents/b/tmp"));
    assert_eq!(tmp_copy("/h/agents/a/tmp/x/r.csv", a, b), Some(PathBuf::from("/h/agents/b/tmp/attached/r.csv")));
    assert_eq!(tmp_copy("/w/r.csv", a, b), None);
    assert_eq!(tmp_copy("/h/agents/a/tmp/../../c/r.csv", a, b), None);
}

#[test]
fn parts_are_the_text_then_each_attachment_numbered() {
    let items = vec![
        Item::Image { source: "/u/s.png".into(), mime: "image/png".into(), b64: "/h/images/1.b64".into() },
        Item::File { path: "/w/deck.pptx".into() },
    ];
    let p = parts("here it is  ", &items);
    assert_eq!(p[0], Part::Text { text: "here it is\n\n".into() });
    assert!(matches!(&p[1], Part::Image { name, .. } if name == "[Image #1]"));
    assert!(matches!(&p[2], Part::File { name, .. } if name == "[File #2]"));
    // the render: his words, then the markers the runtime reads
    let text = parts::render(&p);
    assert!(text.starts_with("here it is\n\n<image name=\"[Image #1]\""));
    assert!(text.ends_with("[deck.pptx](/w/deck.pptx)"), "{text}");
    // and it reads back as the same parts (the fold's fallback), the file
    // named by its name (its label is not in the text)
    let mut back = p.clone();
    back[2] = Part::File { path: "/w/deck.pptx".into(), name: "deck.pptx".into(), missing: false };
    assert_eq!(parts::parse(&text), back);
}

#[test]
fn ten_at_most() {
    assert!(capped(MAX).is_ok());
    assert!(capped(MAX + 1).unwrap_err().contains("10 at most"));
}
