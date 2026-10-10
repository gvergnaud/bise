//! Voice mode on the whole screen (plan §4.6, §5): the app with a voice
//! mode over the fakes (no mic, no sound), drawn by the real draw path:
//! the header, the divider, the pane in the composer's place, the key
//! bar; the lanes under 30 rows; tab gives the composer back.
//!
//! `VOICE_CAPTURES=<dir> cargo test -p bend-tui voice_mode_captures --
//! --ignored` writes the designer's captures (text and ANSI): 150/95/80
//! columns × 40 and 29 rows, dark, light, NO_COLOR-like, every phase.

use super::*;
use crate::sb::bench::test_app;
use crate::theme::{self, Mode};
use crate::voicemode::config::VoiceModeConfig;
use crate::voicemode::fakes::Fakes;
use crate::voicemode::settings;
use crate::voicemode::turn::VoiceMode;
use crate::wire::{Ev, Mark};
use crate::App;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::time::{Duration, Instant};

fn app_in_voice_mode() -> (App, Fakes) {
    let mut app = test_app();
    let f = Fakes::new();
    let agent = app.sb.focus_name().to_string();
    let now = Instant::now();
    let vm = VoiceMode::start(&agent, f.ports(Route::Headphones), f.jobs(true), VoiceModeConfig::default(), true, now).unwrap();
    for ev in [
        Ev::You("morning. anything left from yesterday?".into(), Mark::Read, false, None),
        Ev::Assistant("the safari login. **auth-fix** finished it, it waits on your review: PR #412.".into()),
        Ev::Info("· voice mode · 14:02".into()),
    ] {
        crate::feed::push_event(&mut app.events, &mut app.cache, ev);
    }
    app.voice_mode = Some(vm);
    app.frame_at = now;
    (app, f)
}

fn draw(app: &mut App, w: u16, h: u16) -> (Vec<String>, Buffer) {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| crate::sb::draw_sb(app, f)).unwrap();
    let buf = term.backend().buffer().clone();
    let rows = buf.content.chunks(w as usize).map(|r| r.iter().map(|c| c.symbol()).collect::<String>().trim_end().to_string()).collect();
    (rows, buf)
}

fn has(rows: &[String], s: &str) -> bool {
    rows.iter().any(|r| r.contains(s))
}

#[test]
fn the_whole_screen_in_voice_mode() {
    let (mut app, _f) = app_in_voice_mode();
    for w in [150u16, 95, 80] {
        let (rows, _) = draw(&mut app, w, 40);
        assert!(has(&rows[..1], "● voice mode 0:00"), "the header at {w}: {:?}", rows[0]);
        assert!(has(&rows, "you ⇄ "), "the divider at {w}");
        assert!(has(&rows, "voice mode · headphones"), "{rows:#?}");
        assert!(has(&rows, "● listening"), "the pane at {w}: {rows:#?}");
        assert!(has(&rows, "esc leave"), "the keys at {w}");
        assert!(has(&rows, "▀▀▀▀"), "the kiss at {w}");
        // the thread stays above
        assert!(has(&rows, "auth-fix finished it"), "the thread at {w}");
        // the pane: half the screen, under the divider
        let div = rows.iter().position(|r| r.contains("you ⇄ ")).unwrap();
        assert!((18..=21).contains(&div), "the divider at row {div} of 40");
    }
}

#[test]
fn under_30_rows_the_lanes() {
    let (mut app, _f) = app_in_voice_mode();
    for w in [150u16, 95, 80] {
        let (rows, _) = draw(&mut app, w, 29);
        assert!(!has(&rows, "▀▀▀▀"), "no kiss at {w}");
        let you = rows.iter().position(|r| r.contains("you  ") && r.contains("● listening")).expect("your lane");
        assert!(rows[you + 1].contains(":*"), "the agent's lane under it: {:?}", rows[you + 1]);
        assert!(rows[you + 2].contains("esc leave"), "the keys under the lanes");
        assert!(has(&rows[..1], "● voice mode"));
    }
}

#[test]
fn tab_gives_the_composer_back_the_header_stays() {
    let (mut app, _f) = app_in_voice_mode();
    app.voice_mode.as_mut().unwrap().set_typing(true);
    let (rows, _) = draw(&mut app, 95, 40);
    assert!(has(&rows[..1], "● voice mode"));
    assert!(!has(&rows, "● listening"), "no pane while you type");
    assert!(!has(&rows, "▀▀▀▀"));
    assert!(has(&rows, "voice mode · headphones"), "the divider says it still");
}

// ---- the designer's captures ----

/// A buffer as ANSI (24-bit colors, bold/dim/underline); `color` off:
/// the attributes only, as a NO_COLOR terminal shows it.
fn ansi(buf: &Buffer, color: bool) -> String {
    use ratatui::style::Color;
    let mut out = String::new();
    let w = buf.area.width as usize;
    for row in buf.content.chunks(w) {
        let mut skip = 0;
        for c in row {
            if skip > 0 {
                skip -= 1;
                continue;
            }
            let mut sgr = vec!["0".to_string()];
            if let (true, Color::Rgb(r, g, b)) = (color, c.fg) {
                sgr.push(format!("38;2;{r};{g};{b}"));
            }
            if let (true, Color::Rgb(r, g, b)) = (color, c.bg) {
                sgr.push(format!("48;2;{r};{g};{b}"));
            }
            if c.modifier.contains(Modifier::BOLD) {
                sgr.push("1".into());
            }
            if c.modifier.contains(Modifier::DIM) {
                sgr.push("2".into());
            }
            if c.modifier.contains(Modifier::UNDERLINED) {
                sgr.push("4".into());
            }
            out.push_str(&format!("\x1b[{}m{}", sgr.join(";"), c.symbol()));
            skip = c.symbol().width().saturating_sub(1);
        }
        out.push_str("\x1b[0m\n");
    }
    out
}

fn sample(phase: Phase) -> PaneView {
    let words = |s: &str, st: WordState| s.split(' ').map(|w| (w.to_string(), st)).collect::<Vec<_>>();
    let (who, w) = match phase {
        Phase::Speaking | Phase::HoldToTalk => {
            let mut w = words("on it. perf takes the", WordState::Said);
            w.extend(words("signup, in its own worktree.", WordState::ToSay));
            (Who::Agent("main".into()), w)
        }
        Phase::CutIn => {
            let mut w = words("and the cookie banner,", WordState::Heard);
            w.extend(words("it hides", WordState::Partial));
            (Who::You, w)
        }
        _ => {
            let mut w = words("the signup is slow on mobile. can you", WordState::Heard);
            w.extend(words("have a", WordState::Partial));
            (Who::You, w)
        }
    };
    PaneView {
        muted: phase == Phase::Muted,
        phase,
        agent: "main".into(),
        who,
        words: w,
        you_level: 0.6,
        agent_level: 0.75,
        you_wave: (0..52).map(|i| (((i * 37) % 11) as f32 / 10.0).min(1.0) * if i > 30 { 1.0 } else { 0.3 }).collect(),
        agent_wave: (0..52).map(|i| (((i * 53) % 13) as f32 / 12.0).min(1.0)).collect(),
        elapsed: Duration::from_secs(134),
        route: Route::Headphones,
        heard_answer: None,
        work: sample_work(),
        kiss_ms: None,
        question: None,
    }
}

/// A turn's work (round 2): the column right of the captions.
fn sample_work() -> Vec<crate::voicemode::Work> {
    use crate::voicemode::{Work, WorkKind as K, WorkState as S};
    let w = |kind, text: &str, state| Work { kind, text: text.into(), state };
    vec![
        w(K::Thinking, "the signup is slow: the bundle or the images", S::Done),
        w(K::Tool, "read src/signup/Hero.tsx", S::Done),
        w(K::Tool, "bash npm run build -- --stats", S::Done),
        w(K::Tool, "bash npm test -- signup", S::Failed),
        w(K::Message, "the test needs its fixture; adding it", S::Done),
        w(K::Tool, "edit src/signup/fixture.ts", S::Done),
        w(K::Thinking, "", S::Done),
        w(K::Tool, "bash npm test -- signup", S::Running),
    ]
}

#[test]
#[ignore]
fn voice_mode_captures() {
    let Some(dir) = std::env::var_os("VOICE_CAPTURES") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let phases: Vec<(&str, Phase)> = vec![
        ("listening", Phase::Listening),
        ("about-to-answer", Phase::AboutToAnswer { fill: 0.6 }),
        ("holding", Phase::Holding),
        ("working", Phase::Working),
        ("speaking", Phase::Speaking),
        ("cut-in", Phase::CutIn),
        ("muted", Phase::Muted),
        ("hold-to-talk", Phase::HoldToTalk),
        ("failed", Phase::Failed("voice: no key for mistral · /voice".into())),
    ];
    let forms = [("dark", Mode::Dark, false), ("light", Mode::Light, false), ("nocolor", Mode::Dark, true)];
    let mut index = String::new();
    for (fname, mode, no_color) in forms {
        theme::set_mode(mode);
        let form = Form { still: true, no_color, ascii: false };
        // the whole screen, the real draw path (listening)
        for (w, h) in [(150u16, 40u16), (95, 40), (80, 40), (150, 29), (95, 29), (80, 29)] {
            let (mut app, _f) = app_in_voice_mode();
            let (rows, buf) = draw(&mut app, w, h);
            let buf = &buf;
            let name = format!("screen-{w}x{h}-{fname}");
            std::fs::write(dir.join(format!("{name}.txt")), rows.join("\n") + "\n").unwrap();
            std::fs::write(dir.join(format!("{name}.ans")), ansi(buf, !no_color)).unwrap();
            index.push_str(&format!("{name}\n"));
        }
        // voice-settings' screens: /voice and the first voice mode's
        for (sname, open) in [("settings", settings::Open::Settings), ("privacy", settings::Open::Privacy)] {
            let who = settings::Who { listen: "Mistral".into(), speak: Ok("Mistral".into()), ack: "Mistral".into() };
            let s = settings::Screen::new(open, VoiceModeConfig::default(), who, true);
            for (w, h) in [(150u16, 40u16), (95, 40), (80, 40), (80, 29)] {
                let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
                t.draw(|f| settings::draw_in(f, &s, Instant::now())).unwrap();
                let buf = t.backend().buffer().clone();
                let rows: Vec<String> = buf.content.chunks(w as usize).map(|r| r.iter().map(|c| c.symbol()).collect::<String>().trim_end().to_string()).collect();
                let name = format!("{sname}-{w}x{h}-{fname}");
                let mut txt = rows.join("\u{a}");
                txt.push('\u{a}');
                std::fs::write(dir.join(format!("{name}.txt")), txt).unwrap();
                std::fs::write(dir.join(format!("{name}.ans")), ansi(&buf, !no_color)).unwrap();
                index.push_str(&name);
                index.push('\u{a}');
            }
        }
        // every phase: the pane as drawn in the composer's place
        for (pname, p) in &phases {
            for (w, h) in [(148u16, 19u16), (93, 19), (78, 19), (148, LANES_H), (93, LANES_H), (78, LANES_H)] {
                let mut v = sample(p.clone());
                if *pname == "hold-to-talk" {
                    v.who = Who::You;
                }
                let area = Rect::new(0, 0, w, h + 1);
                let mut buf = Buffer::empty(area);
                draw_in(&mut buf, Rect { height: h, ..area }, &v, 0, form);
                let keys = keys(&v, w);
                buf.set_line(0, h, &keys, w);
                let rows: Vec<String> = buf.content.chunks(w as usize).map(|r| r.iter().map(|c| c.symbol()).collect::<String>().trim_end().to_string()).collect();
                let name = format!("pane-{pname}-{w}x{}-{fname}", h + 1);
                std::fs::write(dir.join(format!("{name}.txt")), rows.join("\n") + "\n").unwrap();
                std::fs::write(dir.join(format!("{name}.ans")), ansi(&buf, !no_color)).unwrap();
                index.push_str(&format!("{name}\n"));
            }
        }
        // voice-mute: your mic off while the agent speaks, works, rests
        for (mname, p) in [("speaking", Phase::Speaking), ("working", Phase::Working), ("rest", Phase::Muted)] {
            for (w, h) in [(148u16, 19u16), (93, 19), (78, 19), (148, LANES_H), (78, LANES_H)] {
                let mut v = sample(p.clone());
                v.muted = true;
                let area = Rect::new(0, 0, w, h + 1);
                let mut buf = Buffer::empty(area);
                draw_in(&mut buf, Rect { height: h, ..area }, &v, 0, form);
                buf.set_line(0, h, &keys(&v, w), w);
                let rows: Vec<String> = buf.content.chunks(w as usize).map(|r| r.iter().map(|c| c.symbol()).collect::<String>().trim_end().to_string()).collect();
                let name = format!("muted-{mname}-{w}x{}-{fname}", h + 1);
                std::fs::write(dir.join(format!("{name}.txt")), rows.join("\u{a}") + "\u{a}").unwrap();
                std::fs::write(dir.join(format!("{name}.ans")), ansi(&buf, !no_color)).unwrap();
                index.push_str(&name);
                index.push('\u{a}');
            }
        }
        // voice-lastq: your last question above the answer (40 and 30 rows)
        let long = "can you check why the signup is slow on mobile and whether the hero images are the cause";
        let asked: Vec<(&str, Phase, &str)> = vec![
            ("working", Phase::Working, "check the build and tell me what failed"),
            ("speaking", Phase::Speaking, "check the build and tell me what failed"),
            ("after", Phase::Listening, "check the build and tell me what failed"),
            ("long", Phase::Speaking, long),
        ];
        for (aname, p, q) in &asked {
            for (w, h) in [(148u16, 19u16), (93, 19), (78, 19), (148, 14), (93, 14), (78, 14)] {
                let mut v = sample(if *p == Phase::Listening { Phase::Speaking } else { p.clone() });
                v.phase = p.clone();
                v.question = Some(q.to_string());
                if *p == Phase::Working {
                    v.who = Who::You;
                    v.words.clear();
                }
                if *p == Phase::Listening {
                    v.words.iter_mut().for_each(|x| x.1 = WordState::Said);
                }
                let area = Rect::new(0, 0, w, h + 1);
                let mut buf = Buffer::empty(area);
                draw_in(&mut buf, Rect { height: h, ..area }, &v, 0, form);
                buf.set_line(0, h, &keys(&v, w), w);
                let rows: Vec<String> = buf.content.chunks(w as usize).map(|r| r.iter().map(|c| c.symbol()).collect::<String>().trim_end().to_string()).collect();
                let name = format!("asked-{aname}-{w}x{}-{fname}", h + 1);
                std::fs::write(dir.join(format!("{name}.txt")), rows.join("\u{a}") + "\u{a}").unwrap();
                std::fs::write(dir.join(format!("{name}.ans")), ansi(&buf, !no_color)).unwrap();
                index.push_str(&name);
                index.push('\u{a}');
            }
        }
    }
    theme::set_mode(Mode::Dark);
    std::fs::write(dir.join("index.txt"), index).unwrap();
}
