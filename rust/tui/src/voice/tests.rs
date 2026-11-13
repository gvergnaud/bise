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
    // prefs.json, the theme's key kept
    let d = std::env::temp_dir().join(format!("bise-voice-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let hs = d.to_string_lossy().to_string();
    let h = bise_home::Home::from_lookup(&|k: &str| (k == "HOME").then(|| hs.clone()));
    let prefs = d.join(".bise/prefs.json");
    std::fs::create_dir_all(prefs.parent().unwrap()).unwrap();
    std::fs::write(&prefs, r#"{"theme": "dark"}"#).unwrap();
    h.pref(bise_home::Pref::Voice).set(true.into()).unwrap();
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&prefs).unwrap()).unwrap();
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

/// No time limit (dictate-nocap): an hour in, it still records.
#[test]
fn recording_has_no_time_limit() {
    let (rec, tr) = (FakeRecorder::ok(true), FakeTranscriber::default());
    let mut v = voice(&rec, &tr);
    let t0 = Instant::now();
    v.start(key(), t0).unwrap();
    assert!(v.poll(t0 + Duration::from_secs(3600)).is_empty());
    assert_eq!(v.state(), VoiceState::Recording);
    assert!(tr.audio().is_empty());
}

/// The wait for a stopped clip's words counts from the transcriber's
/// last sign: a long clip's segments each get their time.
#[test]
fn the_flush_wait_starts_again_at_each_segment() {
    let (rec, tr) = (FakeRecorder::ok(true), FakeTranscriber::default());
    let mut v = voice(&rec, &tr);
    let t0 = Instant::now();
    v.start(key(), t0).unwrap();
    v.stop(t0);
    let later = t0 + TRANSCRIBE_TIMEOUT - Duration::from_secs(1);
    tr.send(TranscribeEvent::Delta("w1".into()));
    assert_eq!(v.poll(later), vec![VoiceOutput::Insert("w1".into())]);
    // past the stop's wait, within the segment's: still waiting
    assert!(v.poll(t0 + TRANSCRIBE_TIMEOUT + Duration::from_secs(5)).is_empty());
    assert_eq!(v.state(), VoiceState::Flushing);
    let out = v.poll(later + TRANSCRIBE_TIMEOUT);
    assert_eq!(out, vec![VoiceOutput::Error("voice transcription failed: the transcription timed out".into())]);
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

/// The fake HTTP of [`run_segments`]: each request's sample count, and
/// its words ("w<n>", n its order) or a failure while `down` says so.
struct Seg {
    sizes: std::cell::RefCell<Vec<usize>>,
    down: std::cell::Cell<bool>,
}

impl Seg {
    fn new() -> Seg {
        Seg { sizes: Default::default(), down: Default::default() }
    }
    fn send(&self, r: &http::Request) -> Result<http::Response, String> {
        if self.down.get() {
            return Ok(http::Response { status: 503, body: b"{}".to_vec() });
        }
        // the WAV's data chunk: its length is the sample count x 2
        let riff = r.body.windows(4).position(|w| w == b"RIFF").expect("a WAV");
        let at = riff + 36;
        assert_eq!(&r.body[at..at + 4], b"data");
        let len = u32::from_le_bytes(r.body[at + 4..at + 8].try_into().unwrap()) as usize / 2;
        let mut sizes = self.sizes.borrow_mut();
        sizes.push(len);
        Ok(http::Response { status: 200, body: format!(r#"{{"text":"w{}"}}"#, sizes.len()).into_bytes() })
    }
}

fn events_of(rx: &mpsc::Receiver<TranscribeEvent>) -> Vec<TranscribeEvent> {
    rx.try_iter().collect()
}

/// Every chunk until End (even queued behind it: not after), the recorder
/// gone is the end too; cancel sends nothing.
#[test]
fn the_clip_is_every_chunk_until_end_one_request_when_short() {
    let s = Seg::new();
    let (tx, rx) = mpsc::channel();
    let (etx, erx) = mpsc::channel();
    tx.send(AudioMsg::Chunk(speech())).unwrap();
    tx.send(AudioMsg::Chunk(speech())).unwrap();
    tx.send(AudioMsg::End).unwrap();
    tx.send(AudioMsg::Chunk(speech())).unwrap();
    run_segments(&job(), &rx, &etx, &AtomicBool::new(false), &|r| s.send(r));
    assert_eq!(*s.sizes.borrow(), vec![32_000]);
    assert_eq!(events_of(&erx), vec![TranscribeEvent::Delta("w1".into()), TranscribeEvent::Done]);
    let (tx, rx) = mpsc::channel();
    tx.send(AudioMsg::Chunk(speech())).unwrap();
    drop(tx);
    run_segments(&job(), &rx, &etx, &AtomicBool::new(false), &|r| s.send(r));
    assert_eq!(events_of(&erx).last(), Some(&TranscribeEvent::Done));
    let (_tx, rx) = mpsc::channel::<AudioMsg>();
    run_segments(&job(), &rx, &etx, &AtomicBool::new(true), &|r| s.send(r));
    assert!(events_of(&erx).is_empty());
}

/// No time limit (dictate-nocap): a 10-minute clip is heard in segments
/// of 3 min at most, every sample once, its words in order, one space
/// between two segments'.
#[test]
fn a_ten_minute_clip_keeps_every_word() {
    let s = Seg::new();
    let (tx, rx) = mpsc::channel();
    let (etx, erx) = mpsc::channel();
    for _ in 0..600 {
        tx.send(AudioMsg::Chunk(speech())).unwrap();
    }
    tx.send(AudioMsg::End).unwrap();
    run_segments(&job(), &rx, &etx, &AtomicBool::new(false), &|r| s.send(r));
    let sizes = s.sizes.borrow().clone();
    assert_eq!(sizes.iter().sum::<usize>(), 600 * 16_000, "every sample heard once");
    assert!(sizes.iter().all(|&n| n <= segments::SEGMENT_MAX), "{sizes:?}");
    assert_eq!(sizes.len(), 4);
    let words: String = events_of(&erx)
        .into_iter()
        .filter_map(|e| match e {
            TranscribeEvent::Delta(t) => Some(t),
            _ => None,
        })
        .collect();
    assert_eq!(words, "w1 w2 w3 w4");
}

/// A long clip whose provider is down at the end: the words heard so far
/// came, the failure keeps only the sound not yet heard (for ctrl+r).
#[test]
fn a_failure_keeps_only_the_sound_not_yet_heard() {
    let s = Seg::new();
    let (tx, rx) = mpsc::channel();
    let (etx, erx) = mpsc::channel();
    for _ in 0..200 {
        tx.send(AudioMsg::Chunk(speech())).unwrap();
    }
    tx.send(AudioMsg::End).unwrap();
    // down once the first segment is heard
    let send = |r: &http::Request| {
        let out = s.send(r);
        s.down.set(true);
        out
    };
    run_segments(&job(), &rx, &etx, &AtomicBool::new(false), &send);
    let evs = events_of(&erx);
    assert_eq!(evs[0], TranscribeEvent::Delta("w1".into()));
    let Some(TranscribeEvent::Failed(_, kept)) = evs.last() else { panic!("{evs:?}") };
    assert_eq!(kept.len(), 200 * 16_000 - segments::SEGMENT_MAX);
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

/// Mistral took voxtral-transcribe-3 away (2026-10-19: a 404, "Mistral
/// can't transcribe with voxtral-transcribe-3" on every take): the take
/// goes once to the provider's voice pick, and its words land.
#[test]
fn a_model_the_provider_took_away_falls_back_to_its_voice_pick_once() {
    let gone = br#"{"object":"error","message":"Invalid model: voxtral-transcribe-3","type":"invalid_model","code":"1500"}"#;
    let models = std::cell::RefCell::new(Vec::new());
    let send = |r: &http::Request| {
        let body = String::from_utf8_lossy(&r.body).to_string();
        let model = ["voxtral-transcribe-3", "voxtral-mini-transcribe-3"]
            .into_iter()
            .find(|m| body.contains(&format!("\r\n\r\n{}\r\n", m)))
            .unwrap_or("?");
        models.borrow_mut().push(model);
        Ok(match model {
            "voxtral-mini-transcribe-3" => http::Response { status: 200, body: br#"{"text":" the words come back "}"#.to_vec() },
            _ => http::Response { status: 404, body: gone.to_vec() },
        })
    };
    let job = VoiceJob {
        name: "mistral/voxtral-transcribe-3".into(),
        model: "voxtral-transcribe-3".into(),
        fallback: Some("voxtral-mini-transcribe-3".into()),
        ..job()
    };
    let no = AtomicBool::new(false);
    assert_eq!(transcribe_clip(&job, &speech(), &no, &send), Ok("the words come back".into()));
    assert_eq!(*models.borrow(), vec!["voxtral-transcribe-3", "voxtral-mini-transcribe-3"]);
    // no fallback: the user's model's failure, one request
    models.borrow_mut().clear();
    let alone = VoiceJob { fallback: None, ..job.clone() };
    assert_eq!(transcribe_clip(&alone, &speech(), &no, &send).map_err(|f| f.kind), Err(FailKind::Model));
    assert_eq!(models.borrow().len(), 1);
    // the fallback gone too: the user's model's failure, two requests
    models.borrow_mut().clear();
    let both = VoiceJob { fallback: Some("voxtral-transcribe-3".into()), ..job.clone() };
    let f = transcribe_clip(&both, &speech(), &no, &send).unwrap_err();
    assert_eq!((f.kind, models.borrow().len()), (FailKind::Model, 2));
    // a wrong key does not fall back
    models.borrow_mut().clear();
    let fail = |_: &http::Request| Ok(http::Response { status: 401, body: br#"{"message":"Unauthorized"}"#.to_vec() });
    assert_eq!(transcribe_clip(&job, &speech(), &no, &fail).map_err(|f| f.kind), Err(FailKind::WrongKey));
}

/// Every user, off-screen: a fake provider whose model list lacks the
/// catalog's default. Left to the default, dictation takes one it lists
/// (one request); a model he wrote and it took away falls back once and
/// the take says so in one line (the BatchTranscriber, as ctrl+r runs it).
#[test]
fn dictation_works_when_the_providers_list_lacks_the_default() {
    let (url, got) = fakes::serve_until(|body: &str| {
        if body.contains("\r\n\r\nvoxtral-next\r\n") {
            (200, r#"{"text":"hello"}"#.to_string())
        } else {
            (404, r#"{"message":"Invalid model","type":"invalid_model"}"#.to_string())
        }
    });
    let lists = vec![("mistral".to_string(), vec!["voxtral-next".to_string()])];
    let env = |k: &str| (k == "MISTRAL_API_KEY").then(|| "sk-fake".to_string());
    let store = bise_catalog::auth::Store::default();
    let keys = bise_catalog::auth::Keys { env: &env, store: &store, files: &[] };
    let no_env = |_: &str| None;
    let job_of = |cfg: &str| VoiceJob { base_url: url.clone(), ..bise_catalog::Setup::from_text(Some(cfg), &no_env).with_live_stt(&lists).voice_job(&keys).unwrap() };
    let t = Duration::from_secs(10);
    for (cfg, requests, note) in [("", 1, false), ("[roles]\nvoice = \"mistral/voxtral-transcribe-3\"\n", 2, true)] {
        let (atx, arx) = mpsc::channel();
        let (etx, erx) = mpsc::channel();
        BatchTranscriber.start(job_of(cfg), arx, etx, Arc::new(AtomicBool::new(false)));
        atx.send(AudioMsg::Chunk(speech())).unwrap();
        atx.send(AudioMsg::End).unwrap();
        assert_eq!(erx.recv_timeout(t), Ok(TranscribeEvent::Delta("hello".into())), "config {cfg:?}");
        if note {
            assert_eq!(
                erx.recv_timeout(t),
                Ok(TranscribeEvent::Note("voxtral-transcribe-3 is gone from Mistral: voxtral-next heard this one · /voice to pick another".into()))
            );
        }
        assert_eq!(erx.recv_timeout(t), Ok(TranscribeEvent::Done), "config {cfg:?}");
        assert_eq!((0..requests).filter_map(|_| got.recv_timeout(t).ok()).count(), requests, "config {cfg:?}");
    }
}

/// Gabriel's dictation on 2026-10-19, off-screen: Mistral answers 404 to
/// voxtral-transcribe-3 and knows the Transcribe 3 ids it has now. His
/// config (it names the old id) and the default both get their words.
#[test]
fn dictation_survives_mistral_taking_voxtral_transcribe_3_away() {
    let mistral_now = |r: &http::Request| {
        let body = String::from_utf8_lossy(&r.body).to_string();
        let known = ["voxtral-mini-transcribe-3", "voxtral-small-transcribe-3"]
            .iter()
            .any(|m| body.contains(&format!("\r\n\r\n{}\r\n", m)));
        Ok(if known {
            http::Response { status: 200, body: br#"{"text":"bonjour"}"#.to_vec() }
        } else {
            http::Response { status: 404, body: br#"{"message":"Invalid model","type":"invalid_model"}"#.to_vec() }
        })
    };
    let env = |k: &str| (k == "MISTRAL_API_KEY").then(|| "sk-fake".to_string());
    let store = bise_catalog::auth::Store::default();
    let keys = bise_catalog::auth::Keys { env: &env, store: &store, files: &[] };
    let no_env = |_: &str| None;
    for cfg in ["[roles]\nvoice = \"mistral/voxtral-transcribe-3\"\n", ""] {
        let job = bise_catalog::Setup::from_text(Some(cfg), &no_env).voice_job(&keys).unwrap();
        let got = transcribe_clip(&job, &speech(), &AtomicBool::new(false), &mistral_now);
        assert_eq!(got.map_err(|f| f.kind), Ok("bonjour".to_string()), "config {cfg:?}");
    }
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
        fallback: None,
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
