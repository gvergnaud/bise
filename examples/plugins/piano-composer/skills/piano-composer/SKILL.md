---
name: piano-composer
description: Compose or change a short piano melody the user can see, play and edit. Use when the user asks for a melody, a tune, a jingle, a chord line, or to change one ("make it major", "add a second phrase").
---

# Piano composer

1. Call `tools.piano_composer.compose` from run_typescript. Pass `key`
   ("D minor", "Bb major"), `bpm`, `title`, and either `bars` (1 to 16:
   the server makes a melody up) or `notes` to write it yourself:
   `["E4:0.5", "G4:0.5", "A4:1", ...]` (pitch:beats, one after the
   other) or `{p, t, d}` objects (t = start beat, d = beats).
2. The user sees a piano roll under your tool call: they can play it,
   click notes in or out, record from a MIDI keyboard and export a .mid.
   Their edits come back to you as model context ("The user edited the
   melody…" with the current notes): start from those notes, not from
   your first version.
3. A message from the view ("Make it major.", "Add a second phrase…")
   is a request about the current melody: call `compose` again with the
   same title and the changed notes (to change the mode, move each note
   to the new scale; keep the rhythm).
4. Answer in one line; the view shows the music. Do not paste the notes.
