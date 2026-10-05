//! voice.rs: no microphone, no network — fake ports.

use super::fakes::*;
use super::*;

fn voice(rec: &FakeRecorder, tr: &FakeTranscriber) -> Voice {
    Voice::new(true, Box::new(rec.clone()), Box::new(tr.clone()))
}

fn key() -> Result<VoiceJob, String> {
    Ok(job())
}

// ---- keys ----

#[test]
fn ctrl_r_starts_when_enabled_and_hints_when_off() {
    let ctrl = KeyModifiers::CONTROL;
    assert_eq!(key_action(VoiceState::Idle, true, KeyCode::Char('r'), ctrl), KeyAction::Start);
    assert_eq!(key_action(VoiceState::Idle, false, KeyCode::Char('r'), ctrl), KeyAction::OffHint);
    assert_eq!(key_action(VoiceState::Idle, true, KeyCode::Char('r'), KeyModifiers::NONE), KeyAction::Pass);
    assert_eq!(key_action(VoiceState::Idle, true, KeyCode::Esc, KeyModifiers::NONE), KeyAction::Pass);
    assert_eq!(key_action(VoiceState::Idle, true, KeyCode::Char('c'), ctrl), KeyAction::Pass);
}

#[test]
fn while_recording_any_key_stops_and_ctrl_c_or_esc_cancel() {
    let r = VoiceState::Recording;
    assert_eq!(key_action(r, true, KeyCode::Char('a'), KeyModifiers::NONE), KeyAction::Stop);
    assert_eq!(key_action(r, true, KeyCode::Enter, KeyModifiers::NONE), KeyAction::Stop);
    assert_eq!(key_action(r, true, KeyCode::Char('r'), KeyModifiers::CONTROL), KeyAction::Stop);
    assert_eq!(key_action(r, true, KeyCode::Char('c'), KeyModifiers::CONTROL), KeyAction::Cancel);
    assert_eq!(key_action(r, true, KeyCode::Esc, KeyModifiers::NONE), KeyAction::Cancel);
    let f = VoiceState::Flushing;
    // BISE-222: while transcribing you keep typing; what would send the
    // text or record again is eaten
    assert_eq!(key_action(f, true, KeyCode::Char('a'), KeyModifiers::NONE), KeyAction::Pass);
    assert_eq!(key_action(f, true, KeyCode::Backspace, KeyModifiers::NONE), KeyAction::Pass);
    assert_eq!(key_action(f, true, KeyCode::Enter, KeyModifiers::SHIFT), KeyAction::Pass);
    assert_eq!(key_action(f, true, KeyCode::Enter, KeyModifiers::NONE), KeyAction::Swallow);
    assert_eq!(key_action(f, true, KeyCode::Tab, KeyModifiers::NONE), KeyAction::Swallow);
    assert_eq!(key_action(f, true, KeyCode::Char('r'), KeyModifiers::CONTROL), KeyAction::Swallow);
    assert_eq!(key_action(f, true, KeyCode::Esc, KeyModifiers::NONE), KeyAction::Cancel);
    assert_eq!(key_action(f, true, KeyCode::Char('c'), KeyModifiers::CONTROL), KeyAction::Cancel);
    // voice mode switched off mid-recording: the keys still end it
    assert_eq!(key_action(r, false, KeyCode::Char('a'), KeyModifiers::NONE), KeyAction::Stop);
}

// ---- audio ----

#[test]
fn mono_averages_the_channels() {
    assert_eq!(to_mono(&[0.2, 0.4, -1.0, 1.0], 2), vec![0.3f32, 0.0]);
    assert_eq!(to_mono(&[0.1, 0.2], 1), vec![0.1, 0.2]);
}

#[test]
fn resampler_48k_to_16k_keeps_one_sample_in_three_across_blocks() {
    let mut r = Resampler::new(48_000, 16_000);
    let input: Vec<f32> = (0..960).map(|i| (i as f32) / 1000.0).collect();
    let mut out = Vec::new();
    // odd block sizes: the phase carries over
    for block in input.chunks(97) {
        out.extend(r.process(block));
    }
    assert_eq!(out.len(), 320);
    for (k, s) in out.iter().enumerate() {
        assert_eq!(*s, to_i16(input[k * 3]), "sample {}", k);
    }
}

#[test]
fn resampler_upsamples_by_interpolating() {
    let mut r = Resampler::new(8_000, 16_000);
    let out = r.process(&[0.0, 0.5]);
    assert_eq!(out, vec![0, to_i16(0.25), to_i16(0.5)]);
    // the next block interpolates from the previous last sample
    let out = r.process(&[1.0]);
    assert_eq!(out, vec![to_i16(0.75), to_i16(1.0)]);
}

#[test]
fn resampler_44100_output_count_is_stable() {
    let mut r = Resampler::new(44_100, 16_000);
    let n: usize = (0..100).map(|_| r.process(&[0.0; 441]).len()).sum();
    assert!((15_999..=16_001).contains(&n), "{}", n);
}

#[test]
fn samples_clip_and_peak_is_normalized() {
    assert_eq!(to_i16(2.0), i16::MAX);
    assert_eq!(to_i16(-2.0), -i16::MAX);
    assert_eq!(peak(&[0, -16384, 100]), 16384.0 / 32767.0);
    assert_eq!(peak(&[i16::MIN]), 1.0);
    assert_eq!(peak(&[]), 0.0);
    assert_eq!(peak_glyph(0.0), '▁');
    assert_eq!(peak_glyph(1.0), '█');
    assert_eq!(peak_glyph(0.5), '▅');
}

/// BISE-246: the meter is in decibels: a quiet room stays on the lowest
/// bar, speech (peaks 0.02-0.3 of full scale on a laptop mic) fills the
/// bars; linear, it all sat on `▁`/`▂` and the chip looked dead.
#[test]
fn the_meter_reads_speech_in_decibels() {
    let bar = |p: f32| peak_glyph(loudness(p));
    assert_eq!(loudness(0.0), 0.0);
    assert_eq!(loudness(1.0), 1.0);
    assert_eq!(loudness(2.0), 1.0);
    // a room's hiss (measured: 0.001-0.0036): flat
    assert_eq!(bar(0.001), '▁');
    assert_eq!(bar(0.0036), '▁');
    // quiet, normal, loud speech
    assert_eq!(bar(0.02), '▃');
    assert_eq!(bar(0.1), '▅');
    assert_eq!(bar(0.3), '▇');
    assert!(loudness(0.05) < loudness(0.06), "it grows with the peak");
}

// ---- settings ----

#[test]
fn voice_mode_is_off_by_default_and_the_env_overrides() {
    assert!(!voice_enabled_from(None, None));
    assert!(!voice_enabled_from(Some("garbage"), None));
    assert!(voice_enabled_from(Some(r#"{"voice_mode_enabled": true}"#), None));
    assert!(!voice_enabled_from(Some(r#"{"voice_mode_enabled": true}"#), Some("0")));
    assert!(voice_enabled_from(None, Some("1")));
    assert!(voice_enabled_from(Some(r#"{"voice_mode_enabled": true}"#), Some("")));
}

#[test]
fn saving_keeps_the_other_settings() {
    // the old layout: ~/.bend-harness/tui.json, the theme's key kept
    let d = std::env::temp_dir().join(format!("bise-voice-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let hs = d.to_string_lossy().to_string();
    let h = bise_home::Home::from_lookup(&|k: &str| (k == "HOME").then(|| hs.clone()));
    let tui = d.join(".bend-harness/tui.json");
    std::fs::create_dir_all(tui.parent().unwrap()).unwrap();
    std::fs::write(&tui, r#"{"theme": "dark"}"#).unwrap();
    h.pref(bise_home::Pref::Voice).set(true.into()).unwrap();
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&tui).unwrap()).unwrap();
    assert_eq!(v, serde_json::json!({"theme": "dark", "voice_mode_enabled": true}));
    let _ = std::fs::remove_dir_all(&d);
}

// ---- the controller ----

#[test]
fn start_without_a_model_or_key_warns_and_stays_idle() {
    let (rec, tr) = (FakeRecorder::ok(true), FakeTranscriber::default());
    let mut v = voice(&rec, &tr);
    let e = "voice transcription needs an API key: set MISTRAL_API_KEY or run 'bise login mistral'";
    assert_eq!(v.start(Err(e.into()), Instant::now()).unwrap_err(), e);
    assert_eq!(v.state(), VoiceState::Idle);
    assert!(tr.session.lock().unwrap().is_none(), "no transcription started");
}

#[test]
fn start_errors_say_what_to_do() {
    let tr = FakeTranscriber::default();
    let mut rec = FakeRecorder::ok(true);
    rec.result = Err(StartError::NoInputDevice);
    let e = voice(&rec, &tr).start(key(), Instant::now()).unwrap_err();
    assert!(e.starts_with("no audio input device found."), "{}", e);
    rec.result = Err(StartError::Backend("boom".into()));
    let e = voice(&rec, &tr).start(key(), Instant::now()).unwrap_err();
    assert_eq!(e, "audio backend is unavailable: boom");
}

#[test]
fn deltas_are_inserted_live_then_stop_flushes_and_done_ends() {
    let (rec, tr) = (FakeRecorder::ok(true), FakeTranscriber::default());
    let mut v = voice(&rec, &tr);
    let t0 = Instant::now();
    v.start(key(), t0).unwrap();
    assert_eq!(v.state(), VoiceState::Recording);
    // no audio yet: a flat meter; a block of speech rises on the right
    assert_eq!(v.levels(), [0.0; chip::BARS]);
    rec.speak(0.1, 1);
    assert!((v.levels()[chip::BARS - 1] - loudness(0.1)).abs() < 1e-3);
    // the block went on to the transcriber too
    assert!(matches!(tr.audio().as_slice(), [AudioMsg::Chunk(c)] if c.len() == chip::METER_BLOCK));
    assert_eq!(tr.session.lock().unwrap().as_ref().unwrap().3, job());
    tr.send(TranscribeEvent::Delta("Hello".into()));
    tr.send(TranscribeEvent::Delta(" world".into()));
    assert_eq!(
        v.poll(t0),
        vec![VoiceOutput::Insert("Hello".into()), VoiceOutput::Insert(" world".into())]
    );
    assert_eq!(v.state(), VoiceState::Recording);
    assert_eq!(v.clip_len(t0 + Duration::from_secs(1)), Duration::from_secs(1));
    let t1 = t0 + Duration::from_secs(2);
    v.stop(t1);
    assert_eq!(v.state(), VoiceState::Flushing);
    assert!(*rec.stopped.borrow(), "the microphone is released at stop");
    // the timer holds the clip's length once stopped
    assert_eq!(v.clip_len(t1 + Duration::from_secs(9)), Duration::from_secs(2));
    assert_eq!(tr.audio(), vec![AudioMsg::End]);
    tr.send(TranscribeEvent::Delta("!".into()));
    tr.send(TranscribeEvent::Done);
    assert_eq!(v.poll(t1), vec![VoiceOutput::Insert("!".into()), VoiceOutput::Utterance]);
    assert_eq!(v.state(), VoiceState::Idle);
    assert!(v.poll(t1).is_empty());
}

#[test]
fn cancel_releases_everything_and_keeps_quiet() {
    let (rec, tr) = (FakeRecorder::ok(true), FakeTranscriber::default());
    let mut v = voice(&rec, &tr);
    v.start(key(), Instant::now()).unwrap();
    v.cancel();
    assert_eq!(v.state(), VoiceState::Idle);
    assert!(*rec.stopped.borrow());
    assert!(tr.cancelled());
    // the cancelled session's events go nowhere
    let s = tr.session.lock().unwrap();
    assert!(s.as_ref().unwrap().1.send(TranscribeEvent::Delta("late".into())).is_err());
    drop(s);
    assert!(v.poll(Instant::now()).is_empty());
}

#[test]
fn a_server_error_stops_the_recording() {
    let (rec, tr) = (FakeRecorder::ok(true), FakeTranscriber::default());
    let mut v = voice(&rec, &tr);
    v.start(key(), Instant::now()).unwrap();
    tr.send(TranscribeEvent::Failed(Failure { kind: FailKind::WrongKey, said: String::new() }, Vec::new()));
    assert_eq!(
        v.poll(Instant::now()),
        vec![VoiceOutput::Failed(FailLines {
            glyph: "✗",
            head: "Mistral says the voice key is wrong. /provider fixes it.".into(),
            dim: Vec::new()
        })]
    );
    assert_eq!(v.state(), VoiceState::Idle);
    assert!(*rec.stopped.borrow());
    assert!(tr.cancelled());
}

#[test]
fn no_text_and_silence_blames_the_microphone() {
    let (rec, tr) = (FakeRecorder::ok(false), FakeTranscriber::default());
    let mut v = voice(&rec, &tr);
    let t0 = Instant::now();
    v.start(key(), t0).unwrap();
    v.stop(t0 + Duration::from_millis(800));
    tr.send(TranscribeEvent::Done);
    let out = v.poll(t0 + Duration::from_millis(900));
    assert_eq!(out.len(), 1);
    match &out[0] {
        VoiceOutput::Error(m) => assert!(
            m.starts_with("voice transcription failed: i can't hear you. "),
            "{}",
            m
        ),
        other => panic!("{:?}", other),
    }
}

#[test]
fn no_text_but_a_signal_or_a_short_press_is_no_speech() {
    for (signal, ms) in [(true, 2000), (false, 200)] {
        let (rec, tr) = (FakeRecorder::ok(signal), FakeTranscriber::default());
        let mut v = voice(&rec, &tr);
        let t0 = Instant::now();
        v.start(key(), t0).unwrap();
        v.stop(t0 + Duration::from_millis(ms));
        tr.close();
        assert_eq!(
            v.poll(t0 + Duration::from_millis(ms + 10)),
            vec![VoiceOutput::Notice("no speech detected".into())]
        );
        assert_eq!(v.state(), VoiceState::Idle);
    }
}

#[test]
fn the_transcription_times_out_after_two_minutes() {
    let (rec, tr) = (FakeRecorder::ok(true), FakeTranscriber::default());
    let mut v = voice(&rec, &tr);
    let t0 = Instant::now();
    v.start(key(), t0).unwrap();
    v.stop(t0);
    assert!(v.poll(t0 + Duration::from_secs(119)).is_empty());
    assert_eq!(
        v.poll(t0 + Duration::from_secs(120)),
        vec![VoiceOutput::Error("voice transcription failed: the transcription timed out".into())]
    );
    assert_eq!(v.state(), VoiceState::Idle);
    assert!(tr.cancelled());
}

#[test]
fn recording_stops_itself_after_five_minutes() {
    let (rec, tr) = (FakeRecorder::ok(true), FakeTranscriber::default());
    let mut v = voice(&rec, &tr);
    let t0 = Instant::now();
    v.start(key(), t0).unwrap();
    assert!(v.poll(t0 + Duration::from_secs(299)).is_empty());
    assert_eq!(v.state(), VoiceState::Recording);
    v.poll(t0 + Duration::from_secs(300));
    assert_eq!(v.state(), VoiceState::Flushing);
    assert_eq!(tr.audio(), vec![AudioMsg::End]);
}

#[test]
fn start_while_active_is_a_no_op() {
    let (rec, tr) = (FakeRecorder::ok(true), FakeTranscriber::default());
    let mut v = voice(&rec, &tr);
    v.start(key(), Instant::now()).unwrap();
    assert_eq!(v.start(Err("x".into()), Instant::now()), Ok(()));
    assert_eq!(v.state(), VoiceState::Recording);
}

// ---- the batch transcription ----

#[test]
fn a_wav_header_says_16k_mono_s16() {
    let w = wav_bytes(&[1, -2], 16_000);
    assert_eq!(w.len(), 48);
    assert_eq!(&w[..4], b"RIFF");
    assert_eq!(u32::from_le_bytes(w[4..8].try_into().unwrap()), 40);
    assert_eq!(&w[8..16], b"WAVEfmt ");
    assert_eq!(u16::from_le_bytes([w[22], w[23]]), 1, "mono");
    assert_eq!(u32::from_le_bytes(w[24..28].try_into().unwrap()), 16_000);
    assert_eq!(&w[36..40], b"data");
    assert_eq!(&w[44..], &[1, 0, 0xfe, 0xff]);
}

#[test]
fn the_clip_is_every_chunk_until_end_even_queued_behind_them() {
    let (tx, rx) = mpsc::channel();
    tx.send(AudioMsg::Chunk(vec![1, 2])).unwrap();
    tx.send(AudioMsg::Chunk(vec![3])).unwrap();
    tx.send(AudioMsg::End).unwrap();
    tx.send(AudioMsg::Chunk(vec![9])).unwrap();
    assert_eq!(collect_clip(&rx, &AtomicBool::new(false)), Some(vec![1, 2, 3]));
    // the recorder gone is the end too; cancel drops the clip
    let (tx, rx) = mpsc::channel();
    tx.send(AudioMsg::Chunk(vec![4])).unwrap();
    drop(tx);
    assert_eq!(collect_clip(&rx, &AtomicBool::new(false)), Some(vec![4]));
    let (_tx, rx) = mpsc::channel::<AudioMsg>();
    assert_eq!(collect_clip(&rx, &AtomicBool::new(true)), None);
}

fn speech() -> Vec<i16> {
    (0..16_000).map(|i| ((i as f32 / 8.0).sin() * 8000.0) as i16).collect()
}

#[test]
fn silence_or_a_slip_sends_nothing() {
    let never = |_: &http::Request| -> Result<http::Response, String> { panic!("no request") };
    let no = AtomicBool::new(false);
    assert_eq!(transcribe_clip(&job(), &[0; 16_000], &no, &never), Ok(String::new()));
    assert_eq!(transcribe_clip(&job(), &speech()[..1000], &no, &never), Ok(String::new()));
    assert_eq!(transcribe_clip(&job(), &speech(), &AtomicBool::new(true), &never), Ok(String::new()));
}

#[test]
fn one_request_per_clip_and_the_text_trimmed() {
    let seen = std::cell::RefCell::new(Vec::new());
    let send = |r: &http::Request| {
        seen.borrow_mut().push(r.clone());
        Ok(http::Response { status: 200, body: br#"{"model":"voxtral-mini-latest","text":" Salut, lance cargo clippy. "}"#.to_vec() })
    };
    let t = transcribe_clip(&job(), &speech(), &AtomicBool::new(false), &send);
    assert_eq!(t, Ok("Salut, lance cargo clippy.".into()));
    let seen = seen.borrow();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].url, "https://api.mistral.ai/v1/audio/transcriptions");
    // the WAV is in the body: 44-byte header + 2 bytes a sample
    assert!(seen[0].body.windows(4).any(|w| w == b"RIFF"));
    assert!(seen[0].body.len() > 32_044);
    let fail = |_: &http::Request| Ok(http::Response { status: 401, body: br#"{"message":"Unauthorized"}"#.to_vec() });
    assert_eq!(
        transcribe_clip(&job(), &speech(), &AtomicBool::new(false), &fail),
        Err(Failure { kind: FailKind::WrongKey, said: "Unauthorized".into() })
    );
}

/// The real thread end to end: BatchTranscriber against a local fake
/// server (plain HTTP), the Voice controller on top.
#[test]
fn the_batch_transcriber_talks_to_a_fake_server() {
    let (url, got) = fakes::serve_once(200, r#"{"text":"bonjour le crate bise-catalog"}"#);
    let job = VoiceJob { base_url: url, ..job() };
    let (atx, arx) = mpsc::channel();
    let (etx, erx) = mpsc::channel();
    BatchTranscriber.start(job, arx, etx, Arc::new(AtomicBool::new(false)));
    atx.send(AudioMsg::Chunk(speech())).unwrap();
    atx.send(AudioMsg::End).unwrap();
    let t = Duration::from_secs(10);
    assert_eq!(erx.recv_timeout(t), Ok(TranscribeEvent::Delta("bonjour le crate bise-catalog".into())));
    assert_eq!(erx.recv_timeout(t), Ok(TranscribeEvent::Done));
    let req = got.recv_timeout(t).unwrap();
    let head = String::from_utf8_lossy(&req[..req.windows(4).position(|w| w == b"\r\n\r\n").unwrap()]).to_string();
    assert!(head.starts_with("POST /v1/audio/transcriptions HTTP/1.1\r\n"), "{head}");
    assert!(head.contains("Authorization: Bearer sk-test\r\n"), "{head}");
    assert!(head.contains("Content-Type: multipart/form-data; boundary="), "{head}");
    let body = String::from_utf8_lossy(&req);
    assert!(body.contains("name=\"model\"\r\n\r\nvoxtral-mini-latest\r\n"), "{body}");
    // an error comes back in one line
    let (url, _got) = fakes::serve_once(400, "{\"detail\": \"invalid model:\\n  nope\"}");
    let job = VoiceJob { base_url: url, ..fakes::job() };
    let (atx, arx) = mpsc::channel();
    let (etx, erx) = mpsc::channel();
    BatchTranscriber.start(job, arx, etx, Arc::new(AtomicBool::new(false)));
    atx.send(AudioMsg::Chunk(speech())).unwrap();
    atx.send(AudioMsg::End).unwrap();
    assert_eq!(
        erx.recv_timeout(t),
        Ok(TranscribeEvent::Failed(Failure { kind: FailKind::Model, said: "invalid model: nope".into() }, speech()))
    );
}

/// The real API, by hand only (network + key: the chat keys'
/// resolution, `[voice]` from config.toml): a WAV
/// (`say -v Thomas -o x.wav --data-format=LEI16@16000 "…"`).
/// `SB_STT_WAV=x.wav cargo test -p bend-tui real_api -- --ignored --nocapture`
#[test]
#[ignore]
fn real_api_transcribes_a_wav() {
    let path = bise_home::env::test_setting("SB_STT_WAV").expect("SB_STT_WAV");
    let bytes = std::fs::read(path).unwrap();
    let data = bytes.windows(4).position(|w| w == b"data").unwrap() + 8;
    let samples: Vec<i16> = bytes[data..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
    let job = resolve_job().expect("a voice model and its key");
    let t0 = Instant::now();
    let text = transcribe_clip(&job, &samples, &AtomicBool::new(false), &|r| http::send(r, TRANSCRIBE_TIMEOUT));
    eprintln!("{} in {:?}: {:?}", job.name, t0.elapsed(), text);
    assert!(!text.unwrap().is_empty());
}

// ---- the providers' wire formats (stt.rs) and the HTTP client ----

fn job_for(api: &str, base: &str, model: &str) -> VoiceJob {
    VoiceJob {
        name: format!("x/{}", model),
        provider_name: "X".into(),
        billing_url: String::new(),
        api: api.into(),
        base_url: base.into(),
        model: model.into(),
        key: "k-1".into(),
        language: Some("fr".into()),
        vocabulary: vec!["bise-catalog".into(), "GitHub".into()],
    }
}

fn header<'a>(r: &'a http::Request, k: &str) -> Option<&'a str> {
    r.headers.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str())
}

fn field(body: &[u8], name: &str) -> Vec<String> {
    let b = String::from_utf8_lossy(body);
    let tag = format!("name=\"{}\"", name);
    b.split("--bise-voice-")
        .filter(|p| p.contains(&format!("{}\r\n", tag)))
        .map(|p| p.split("\r\n\r\n").nth(1).unwrap_or("").trim_end_matches("\r\n").to_string())
        .collect()
}

#[test]
fn each_family_builds_its_request() {
    let wav = wav_bytes(&[1, 2, 3], 16_000);
    // mistral: multipart, context_bias per word, Bearer
    let r = stt::request(&job_for("mistral", "https://api.mistral.ai/v1/", "voxtral-mini-latest"), &wav);
    assert_eq!(r.url, "https://api.mistral.ai/v1/audio/transcriptions");
    assert_eq!(header(&r, "Authorization"), Some("Bearer k-1"));
    assert_eq!(field(&r.body, "model"), vec!["voxtral-mini-latest"]);
    assert_eq!(field(&r.body, "language"), vec!["fr"]);
    assert_eq!(field(&r.body, "context_bias"), vec!["bise-catalog", "GitHub"]);
    assert!(field(&r.body, "prompt").is_empty());
    assert!(String::from_utf8_lossy(&r.body).contains("name=\"file\"; filename=\"audio.wav\"\r\nContent-Type: audio/wav"));
    assert!(r.body.ends_with(b"--bise-voice-7d1f3c9a2e5b--\r\n"));
    // openai / groq: the vocabulary is the prompt
    let r = stt::request(&job_for("openai", "https://api.groq.com/openai/v1", "whisper-large-v3-turbo"), &wav);
    assert_eq!(r.url, "https://api.groq.com/openai/v1/audio/transcriptions");
    assert_eq!(field(&r.body, "prompt"), vec!["bise-catalog, GitHub"]);
    assert_eq!(field(&r.body, "response_format"), vec!["json"]);
    // elevenlabs: model_id, language_code, keyterms, xi-api-key
    let r = stt::request(&job_for("elevenlabs", "https://api.elevenlabs.io/v1", "scribe_v2"), &wav);
    assert_eq!(r.url, "https://api.elevenlabs.io/v1/speech-to-text");
    assert_eq!((header(&r, "xi-api-key"), header(&r, "Authorization")), (Some("k-1"), None));
    assert_eq!(field(&r.body, "model_id"), vec!["scribe_v2"]);
    assert_eq!(field(&r.body, "language_code"), vec!["fr"]);
    assert_eq!(field(&r.body, "keyterms"), vec!["bise-catalog", "GitHub"]);
    // deepgram: the WAV as the body, the options in the query
    let mut j = job_for("deepgram", "https://api.deepgram.com/v1", "nova-3");
    j.language = None;
    j.vocabulary = vec!["config.toml".into(), "pull request".into()];
    let r = stt::request(&j, &wav);
    assert_eq!(
        r.url,
        "https://api.deepgram.com/v1/listen?model=nova-3&smart_format=true&punctuate=true&language=multi&keyterm=config.toml&keyterm=pull%20request"
    );
    assert_eq!((header(&r, "Authorization"), header(&r, "Content-Type")), (Some("Token k-1"), Some("audio/wav")));
    assert_eq!(r.body, wav);
    // no language: the field is not sent (detected)
    let mut j = job_for("mistral", "https://m/v1", "m");
    j.language = None;
    assert!(field(&stt::request(&j, &wav).body, "language").is_empty());
    // the key never shows in Debug
    assert!(!format!("{:?}", stt::request(&j, &wav)).contains("k-1"));
}

#[test]
fn responses_parse_and_errors_are_one_line() {
    let ok = |b: &str| http::Response { status: 200, body: b.as_bytes().to_vec() };
    let err = |s: u16, b: &str| http::Response { status: s, body: b.as_bytes().to_vec() };
    assert_eq!(stt::parse("mistral", &ok(r#"{"text":"salut"}"#)), Ok("salut".into()));
    assert_eq!(stt::parse("elevenlabs", &ok(r#"{"language_code":"fra","text":"x"}"#)), Ok("x".into()));
    let dg = r#"{"results":{"channels":[{"alternatives":[{"transcript":"hello","confidence":0.9}]}]}}"#;
    assert_eq!(stt::parse("deepgram", &ok(dg)), Ok("hello".into()));
    assert_eq!(stt::parse("openai", &ok("{}")), Err("the response has no text".into()));
    assert_eq!(stt::parse("openai", &ok("<html>")), Err("the response is not JSON".into()));
    for (body, want) in [
        (r#"{"error":{"message":"Incorrect API key","type":"x"}}"#, "HTTP 401: Incorrect API key"),
        (r#"{"message":"Unauthorized"}"#, "HTTP 401: Unauthorized"),
        (r#"{"detail":{"status":"invalid_api_key","message":"Invalid API key"}}"#, "HTTP 401: Invalid API key"),
        (r#"{"detail":[{"loc":["body"],"msg":"field required"}]}"#, "HTTP 401: field required"),
        (r#"{"err_code":"INVALID_AUTH","err_msg":"Invalid credentials."}"#, "HTTP 401: Invalid credentials."),
        ("Bad\n  gateway\n", "HTTP 401: Bad gateway"),
        ("", "HTTP 401"),
    ] {
        assert_eq!(stt::parse("mistral", &err(401, body)), Err(want.into()), "{body}");
    }
    let long = "x ".repeat(300);
    assert_eq!(stt::one_line(&long).chars().count(), 201);
}

#[test]
fn http_responses_frame_by_length_chunks_or_close() {
    let r = http::parse_response(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhel", false).unwrap();
    assert_eq!(r, None, "more bytes to come");
    let r = http::parse_response(b"HTTP/1.1 200 OK\r\ncontent-length: 5\r\n\r\nhello", false).unwrap();
    assert_eq!(r, Some(http::Response { status: 200, body: b"hello".to_vec() }));
    let chunked = b"HTTP/1.1 400 Bad\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2;x=1\r\nde\r\n0\r\n\r\n";
    assert_eq!(http::parse_response(chunked, false).unwrap().unwrap().body, b"abcde");
    assert_eq!(http::parse_response(&chunked[..chunked.len() - 7], false).unwrap(), None);
    let close = b"HTTP/1.0 200 OK\r\n\r\n{}";
    assert_eq!(http::parse_response(close, false).unwrap(), None);
    assert_eq!(http::parse_response(close, true).unwrap().unwrap().body, b"{}");
    let cont = b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 201 Created\r\nContent-Length: 0\r\n\r\n";
    assert_eq!(http::parse_response(cont, false).unwrap().unwrap().status, 201);
    assert!(http::parse_response(b"garbage\r\n\r\n", false).is_err());
    assert_eq!(
        http::split_url("https://api.groq.com/openai/v1/audio/transcriptions").unwrap(),
        (true, "api.groq.com".into(), 443, "/openai/v1/audio/transcriptions".into())
    );
    assert_eq!(http::split_url("http://127.0.0.1:8080").unwrap(), (false, "127.0.0.1".into(), 8080, "/".into()));
    assert!(http::split_url("wss://x").is_err());
}

#[test]
fn a_dead_server_is_an_error_not_a_hang() {
    // a port nothing listens on
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1/x", l.local_addr().unwrap());
    drop(l);
    let req = http::Request { url, headers: Vec::new(), body: Vec::new() };
    let e = http::send(&req, Duration::from_secs(2)).unwrap_err();
    assert!(e.starts_with("cannot connect to 127.0.0.1"), "{e}");
}
