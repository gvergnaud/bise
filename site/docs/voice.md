---
title: voice
description: ctrl+r dictates into the composer. ctrl+r twice starts voice mode, a spoken conversation with the agent in view.
---

## dictation

`ctrl+r` records; any key stops and keeps the text in the composer, ready to edit and send. `esc` or `ctrl+c` stops and drops it.

dictation needs a speech-to-text model, the **voice** role. with a Mistral or an OpenAI key, bise picks one for you. `/voice` shows what is set and changes it: the model, the language, the voice.

| provider | models | key |
|---|---|---|
| Mistral | `mistral/voxtral-transcribe-3`, `mistral/voxtral-small-transcribe-3` | `MISTRAL_API_KEY` |
| OpenAI | `openai/gpt-transcribe`, `openai/gpt-4o-mini-transcribe`, `openai/gpt-4o-transcribe`, `openai/whisper-1` | `OPENAI_API_KEY` |
| Groq | `groq/whisper-large-v3-turbo`, `groq/whisper-large-v3` | `GROQ_API_KEY` |
| ElevenLabs | `elevenlabs/scribe_v2`, `elevenlabs/scribe_v1` | `ELEVENLABS_API_KEY` |
| Deepgram | `deepgram/nova-3` | `DEEPGRAM_API_KEY` |

`bise models voice` lists them with the state of each key.

## voice mode

`ctrl+r` twice starts voice mode: you talk with the agent in view, and it answers out loud.

| key | in voice mode |
|---|---|
| `space` | send now |
| `hold space` | keep the floor, or cut in when it talks on speakers |
| `m` | mute |
| `tab` | type instead |
| `esc` | leave voice mode (`ctrl+c` too) |

with headphones it listens hands-free. on speakers you hold `space` while you talk, so it doesn't hear itself. today only Mistral's voice model speaks, so voice mode needs a Mistral key.

by default it reads aloud what needs you and what you asked. the first time, voice mode says who hears you: your audio goes to the speech-to-text provider, and the text it speaks goes to Mistral.

## settings

`/voice` changes all of these. they live in `[voice]`:

```toml title="~/.bise/config.toml"
[roles]
voice = "mistral/voxtral-transcribe-3"   # dictation's model

[voice]
language = "fr"                          # unset: detected
vocabulary = ["bise", "config.toml"]     # words to spell right
listen = "auto"                          # auto (hands-free with headphones, hold on speakers), hands-free, hold
tts_voice = "…"                          # the voice it speaks with; unset: the default voice
speed = 1.0                              # 0.8 to 1.6
read_aloud = "needs"                     # needs (what needs you + what you asked), all, nothing
sounds = true
```

`BISE_VOICE_MODEL` sets the dictation model for one session.
