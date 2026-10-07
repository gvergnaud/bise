//! The listener's tests: the batch path over voice's fake HTTP server
//! (the turn at Flush, Clear, the failures, the controller going). One
//! `#[ignore]` live test (by hand: a `say -o` file, nothing played).

use super::*;
use crate::voice::fakes::serve_once;
use crate::voicemode::Endpoint;
use std::time::Instant;

const WAIT: Duration = Duration::from_secs(3);
/// 100 ms of mic audio.
const BLOCK: usize = MIC_RATE as usize / 10;

struct Run {
    audio: Sender<ListenMsg>,
    heard: Receiver<Heard>,
    cancel: Arc<AtomicBool>,
}

impl Run {
    fn block(&self) {
        self.audio.send(ListenMsg::Audio(vec![3000; BLOCK])).unwrap();
    }
    fn next(&self) -> Heard {
        self.heard.recv_timeout(WAIT).expect("the listener said nothing")
    }
    fn quiet(&self, d: Duration) -> Option<Heard> {
        self.heard.recv_timeout(d).ok()
    }
}

fn start(job: ListenJob) -> Run {
    let (audio, rx) = mpsc::channel();
    let (tx, heard) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    listener_for(&job).start(job, rx, tx, cancel.clone());
    Run { audio, heard, cancel }
}

fn batch(base: &str) -> Run {
    start(ListenJob { realtime: None, batch: VoiceJob { base_url: base.into(), ..crate::voice::fakes::job() } })
}

#[test]
fn batch_transcribes_the_turn_at_flush() {
    let (base, request) = serve_once(200, r#"{"text":"  fix the tests "}"#);
    let run = batch(&base);
    for _ in 0..4 {
        run.block();
    }
    assert_eq!(run.quiet(Duration::from_millis(200)), None, "nothing before the flush");
    run.audio.send(ListenMsg::Flush).unwrap();
    assert_eq!(run.next(), Heard::Text(" fix the tests".into()));
    assert_eq!(run.next(), Heard::Flushed);
    let req = request.recv_timeout(WAIT).unwrap();
    // 4 blocks of 1600 samples, 16-bit, in a WAV: the whole turn
    assert!(req.len() > 4 * BLOCK * 2);
}

#[test]
fn a_realtime_endpoint_is_ignored_the_batch_model_transcribes() {
    // an old ListenJob with a realtime endpoint (nothing listens on port 9):
    // the batch model still hears the turn
    let (base, request) = serve_once(200, r#"{"text":"ok"}"#);
    let ep = Endpoint {
        name: "mistral/voxtral-mini-transcribe-realtime-2602".into(),
        provider_name: "Mistral".into(),
        base_url: "http://127.0.0.1:9/v1".into(),
        model: "voxtral-mini-transcribe-realtime-2602".into(),
        key: "sk-test".into(),
    };
    let run = start(ListenJob { realtime: Some(ep), batch: VoiceJob { base_url: base, ..crate::voice::fakes::job() } });
    for _ in 0..4 {
        run.block();
    }
    run.audio.send(ListenMsg::Flush).unwrap();
    assert_eq!(run.next(), Heard::Text(" ok".into()));
    assert_eq!(run.next(), Heard::Flushed);
    assert!(request.recv_timeout(WAIT).is_ok());
}

#[test]
fn batch_under_min_clip_is_flushed_at_once_without_a_request() {
    let (base, request) = serve_once(200, r#"{"text":"never"}"#);
    let run = batch(&base);
    run.audio.send(ListenMsg::Audio(vec![3000; 800])).unwrap();
    run.audio.send(ListenMsg::Flush).unwrap();
    assert_eq!(run.next(), Heard::Flushed);
    assert!(request.recv_timeout(Duration::from_millis(200)).is_err());
}

#[test]
fn batch_clear_drops_the_half_turn() {
    let (base, request) = serve_once(200, r#"{"text":"never"}"#);
    let run = batch(&base);
    for _ in 0..4 {
        run.block();
    }
    run.audio.send(ListenMsg::Clear).unwrap();
    run.audio.send(ListenMsg::Flush).unwrap();
    assert_eq!(run.next(), Heard::Flushed);
    assert!(request.recv_timeout(Duration::from_millis(200)).is_err());
}

#[test]
fn batch_failure_says_it_in_voice_wording() {
    let (base, _request) = serve_once(401, r#"{"message":"Unauthorized"}"#);
    let run = batch(&base);
    for _ in 0..4 {
        run.block();
    }
    run.audio.send(ListenMsg::Flush).unwrap();
    assert_eq!(run.next(), Heard::Failed("Mistral says the voice key is wrong. /provider fixes it.".into()));
}

#[test]
fn the_listener_stops_when_the_controller_goes_or_on_cancel() {
    let run = batch("http://127.0.0.1:9/v1");
    let Run { audio, heard, .. } = run;
    drop(audio);
    // the thread ends: its sender goes, the channel closes
    assert!(matches!(heard.recv_timeout(WAIT), Err(mpsc::RecvTimeoutError::Disconnected)));
    let run = batch("http://127.0.0.1:9/v1");
    run.cancel.store(true, Ordering::SeqCst);
    assert!(matches!(run.heard.recv_timeout(WAIT), Err(mpsc::RecvTimeoutError::Disconnected)));
}

// ---- live (by hand, no sound) ----

/// The 16 kHz mono PCM of a WAV file (its "data" chunk).
fn wav_pcm(bytes: &[u8]) -> Vec<i16> {
    let mut i = 12;
    while i + 8 <= bytes.len() {
        let len = u32::from_le_bytes(bytes[i + 4..i + 8].try_into().unwrap()) as usize;
        if &bytes[i..i + 4] == b"data" {
            let data = &bytes[i + 8..(i + 8 + len).min(bytes.len())];
            return data.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
        }
        i += 8 + len + (len & 1);
    }
    panic!("no data chunk")
}

/// One voice-mode turn of a `say -o` sentence (a file: nothing is played)
/// through the batch listener, with the voice role's Mistral key and
/// Transcribe 3, a language and a context bias set
/// (`cargo test -p bend-tui live_transcribe_3 -- --ignored --nocapture`).
#[test]
#[ignore]
fn live_transcribe_3_hears_a_spoken_sentence() {
    let mut job = crate::voice::resolve_job().expect("a voice key");
    assert_eq!(job.api, "mistral", "the voice role's provider is not Mistral");
    job.model = "voxtral-transcribe-3".into();
    job.name = "mistral/voxtral-transcribe-3".into();
    job.language = Some("en".into());
    job.vocabulary = vec!["clippy".into(), "bise".into()];
    let dir = std::env::temp_dir().join(format!("bise-listen-live-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (aiff, wav) = (dir.join("s.aiff"), dir.join("s.wav"));
    let said = "please run cargo clippy in bise and fix the failing tests";
    let ok = std::process::Command::new("say").arg("-o").arg(&aiff).arg(said).status().unwrap();
    assert!(ok.success());
    let ok = std::process::Command::new("afconvert").args(["-f", "WAVE", "-d", "LEI16@16000", "-c", "1"]).arg(&aiff).arg(&wav).status().unwrap();
    assert!(ok.success());
    let pcm = wav_pcm(&std::fs::read(&wav).unwrap());
    let _ = std::fs::remove_dir_all(&dir);
    let run = start(ListenJob { realtime: None, batch: job });
    for b in pcm.chunks(BLOCK) {
        run.audio.send(ListenMsg::Audio(b.to_vec())).unwrap();
    }
    let t = Instant::now();
    run.audio.send(ListenMsg::Flush).unwrap();
    let mut text = String::new();
    loop {
        match run.heard.recv_timeout(Duration::from_secs(30)).expect("no Flushed") {
            Heard::Text(t) => text.push_str(&t),
            Heard::Flushed => break,
            Heard::Failed(l) => panic!("{}", l),
        }
    }
    eprintln!("heard {:?} in {:?}", text, t.elapsed());
    let lower = text.to_lowercase();
    assert!(lower.contains("clippy") && lower.contains("tests") && lower.contains("fix"), "{}", text);
}

// ---- the language guard (voice-echo3: two quick French words came back
// as "Да, наверное.") ----

#[test]
fn a_transcript_in_a_script_you_never_use_is_redone_in_your_language() {
    let job = VoiceJob { language: None, ..crate::voice::fakes::job() };
    let (audio, rx) = mpsc::channel();
    let (tx, heard) = mpsc::channel();
    let cancel = AtomicBool::new(false);
    let langs_sent = std::sync::Mutex::new(Vec::<bool>::new());
    let send = |req: &http::Request| {
        // the second request carries the language: French words back
        let body = String::from_utf8_lossy(&req.body).to_string();
        let with_fr = body.contains("name=\"language\"") && body.contains("\r\n\r\nfr\r\n");
        langs_sent.lock().unwrap().push(with_fr);
        let text = if with_fr { "Oui, sans doute." } else { "Да, наверное." };
        Ok(http::Response { status: 200, body: format!(r#"{{"text":"{}"}}"#, text).into_bytes() })
    };
    audio.send(ListenMsg::Audio(vec![3000; MIC_RATE as usize])).unwrap();
    audio.send(ListenMsg::Flush).unwrap();
    drop(audio);
    let mut langs = Langs::new(vec!["fr-FR".into(), "en-FR".into()]);
    run_batch(&job, &rx, &tx, &cancel, &mut langs, &send);
    assert_eq!(heard.try_recv().unwrap(), Heard::Text(" Oui, sans doute.".into()));
    assert_eq!(*langs_sent.lock().unwrap(), [false, true], "auto first, then French");
}

/// Law (ambient-lead m_6275, dogfood: his first French ask came back as
/// 'You, I suppose that's what you said.'): with auto language, an
/// English transcript of the session's first words (or of a short clip)
/// is heard again in your language when it is not English, and kept when
/// it reads as yours; an English ask stays English; once you spoke
/// English, a long English clip is not checked again.
#[test]
fn first_words_read_as_english_are_heard_again_in_your_language() {
    let job = VoiceJob { language: None, ..crate::voice::fakes::job() };
    let turn = |langs: &mut Langs, secs: f32, said_fr: &'static str, said_en: &'static str| {
        let (audio, rx) = mpsc::channel();
        let (tx, heard) = mpsc::channel();
        let cancel = AtomicBool::new(false);
        let sent = std::sync::Mutex::new(Vec::<bool>::new());
        let send = |req: &http::Request| {
            let body = String::from_utf8_lossy(&req.body).to_string();
            let with_fr = body.contains("name=\"language\"") && body.contains("\r\n\r\nfr\r\n");
            sent.lock().unwrap().push(with_fr);
            // auto: the model's guess; in French: what it hears in French
            let text = if with_fr { said_fr } else { said_en };
            Ok(http::Response { status: 200, body: format!(r#"{{"text":"{}"}}"#, text).into_bytes() })
        };
        audio.send(ListenMsg::Audio(vec![3000; (secs * MIC_RATE as f32) as usize])).unwrap();
        audio.send(ListenMsg::Flush).unwrap();
        drop(audio);
        run_batch(&job, &rx, &tx, &cancel, langs, &send);
        let Heard::Text(t) = heard.try_recv().unwrap() else { panic!("no words") };
        let sent = sent.lock().unwrap().clone();
        (t.trim().to_string(), sent)
    };
    let mut langs = Langs::new(vec!["fr-FR".into(), "en-FR".into()]);
    // the first ask, French, guessed English: heard again in French
    let (t, sent) = turn(&mut langs, 6.0, "Est-ce que tu peux lancer les tests ?", "You, I suppose that's what you said.");
    assert_eq!((t.as_str(), sent), ("Est-ce que tu peux lancer les tests ?", vec![false, true]));
    // an English ask, short: checked in French, the English words come back, kept
    let (t, sent) = turn(&mut langs, 2.0, "run the tests now", "run the tests now");
    assert_eq!((t.as_str(), sent), ("run the tests now", vec![false, true]));
    // now English is the last spoken: a long English ask is not checked
    let (t, sent) = turn(&mut langs, 6.0, "-", "please run the tests and tell me what fails");
    assert_eq!((t.as_str(), sent), ("please run the tests and tell me what fails", vec![false]));
    // an English speaker's Mac: never checked
    let mut en = Langs::new(vec!["en-US".into()]);
    assert_eq!(en.recheck("you said it", 1.0), None);
    en.heard("you said it");
    assert_eq!(Langs::new(vec!["fr-FR".into()]).recheck("Tu m'entends ou pas ?", 1.0), None, "French words: kept");
}

#[test]
fn your_languages_keep_their_words_and_the_last_spoken_leads() {
    let mut l = Langs::new(vec!["fr-FR".into(), "en-FR".into(), "en_US.UTF-8".into()]);
    assert_eq!(l.redo("Да, наверное."), Some("fr".into()));
    assert_eq!(l.redo("yes, probably"), None, "Latin: one of yours");
    assert_eq!(l.redo("..."), None, "no letters");
    l.heard("yes, please run the tests now");
    assert_eq!(l.redo("Да, наверное."), Some("en".into()), "the last you spoke");
    // a Russian speaker's Russian stays
    assert_eq!(Langs::new(vec!["ru-RU".into()]).redo("Да, наверное."), None);
    // nothing known: kept
    assert_eq!(Langs::new(Vec::new()).redo("Да, наверное."), None);
    assert_eq!(script_of_text("Да, наверное."), Some(Script::Cyrillic));
    assert_eq!(script_of_text("Oui, ça marche."), Some(Script::Latin));
    assert_eq!(parse_apple_languages("(\n    \"fr-FR\",\n    \"en-FR\"\n)\n"), ["fr-FR", "en-FR"]);
}
