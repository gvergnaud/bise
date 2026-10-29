---
name: voice-memo
description: Let the user record a voice memo. Use when the user wants to record, dictate or leave a voice note, or when speaking is easier for them than typing.
---

# Voice memo

1. Call `tools.voice_memo.record_memo` from run_typescript, with a short
   `prompt` when you want something precise ("Describe the bug in your
   own words"). The user records in the view under your call.
2. When they press "Send to the agent", a message arrives: "Here is my
   memo, 14 s: <path>". The file is an audio recording (webm/ogg/m4a)
   at that path. You cannot hear it: say what you can do with it (keep
   it, attach it, transcribe it with a tool the user has) and ask.
3. `tools.voice_memo.list_memos` lists the saved memos with their paths.
