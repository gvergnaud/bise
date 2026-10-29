"""piano-composer: a dependency-free stdio MCP server with an MCP Apps view.

Tools (SEP-1865, MCP Apps 2026-01-26):

- `compose` (model and app): writes a short melody in a key and answers
  the score in `structuredContent`; the view `ui://piano-composer/piano`
  draws it as a piano roll and plays it.
- `set_notes` (app only): the view's editor saves its notes; answers the
  normalized score. The model never sees it.
- `export_midi` (app only): the score as a real MIDI file, an embedded
  resource (`audio/midi`, base64) the view hands to `ui/download-file`.

The server keeps no state: every call carries the whole score, so a
restarted server (or the hub's own views bridge) answers the same.
"""
import base64
import json
import os
import random
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
MIME = "text/html;profile=mcp-app"
URI = "ui://piano-composer/piano"

NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"]
FLATS = {"Db": "C#", "Eb": "D#", "Gb": "F#", "Ab": "G#", "Bb": "A#"}
MAJOR = [0, 2, 4, 5, 7, 9, 11]
MINOR = [0, 2, 3, 5, 7, 8, 10]


def midi_of(name):
    """'C#4' -> 61; None when it is not a note name."""
    name = name.strip()
    for i in (2, 1):
        pitch, octave = name[:i], name[i:]
        pitch = FLATS.get(pitch, pitch)
        if pitch in NAMES and octave.lstrip("-").isdigit():
            return NAMES.index(pitch) + 12 * (int(octave) + 1)
    return None


def name_of(midi):
    return NAMES[midi % 12] + str(midi // 12 - 1)


def parse_key(key):
    """'D minor' -> (root pitch class, 'minor', 'D minor')."""
    words = (key or "C major").replace("-", " ").split()
    root = FLATS.get(words[0].capitalize(), words[0].capitalize()) if words else "C"
    if root not in NAMES:
        root = "C"
    mode = "minor" if len(words) > 1 and words[1].lower().startswith("min") else "major"
    return NAMES.index(root), mode, f"{root} {mode}"


def melody(key, bars, seed):
    """A singable line: a random walk on the scale, phrases end on the tonic."""
    root, mode, _ = parse_key(key)
    scale = [60 + root + s for s in (MAJOR if mode == "major" else MINOR)]
    scale = [n - 12 for n in scale] + scale + [scale[0] + 12]
    rng = random.Random(seed)
    notes, t, step = [], 0.0, 7
    rhythms = [[1, 1, 1, 1], [1, 0.5, 0.5, 1, 1], [1.5, 0.5, 1, 1], [0.5, 0.5, 1, 2], [2, 1, 1]]
    for bar in range(bars):
        rhythm = rng.choice(rhythms) if bar < bars - 1 else [1, 1, 2]
        for i, d in enumerate(rhythm):
            last = bar == bars - 1 and i == len(rhythm) - 1
            step = 7 if last else max(2, min(len(scale) - 2, step + rng.choice([-2, -1, -1, 1, 1, 2, 0, 3, -3])))
            notes.append({"p": name_of(scale[step]), "t": t, "d": d})
            t += d
    return notes


def clean_notes(raw):
    """Keep the notes that make sense: a known pitch, t >= 0, 0 < d <= 16."""
    out = []
    for n in raw or []:
        if isinstance(n, str):  # "E4" or "E4:0.5", one after another
            p, _, d = n.partition(":")
            n = {"p": p, "t": out[-1]["t"] + out[-1]["d"] if out else 0, "d": float(d or 1)}
        try:
            m, t, d = midi_of(str(n["p"])), float(n.get("t", 0)), float(n.get("d", 1))
        except (KeyError, TypeError, ValueError):
            continue
        if m is not None and 21 <= m <= 108 and 0 <= t <= 256 and 0 < d <= 16:
            out.append({"p": name_of(m), "t": round(t * 4) / 4, "d": max(0.25, round(d * 4) / 4)})
    return sorted(out, key=lambda n: (n["t"], midi_of(n["p"])))[:512]


def score_of(args):
    _, _, key = parse_key(args.get("key"))
    bpm = max(40, min(220, int(args.get("bpm") or 96)))
    return {"title": str(args.get("title") or "untitled")[:80], "key": key, "bpm": bpm,
            "notes": clean_notes(args.get("notes"))}


def describe(s):
    beats = max((n["t"] + n["d"] for n in s["notes"]), default=0)
    line = " ".join(f"{n['p']}:{n['d']:g}" for n in s["notes"][:48])
    more = " …" if len(s["notes"]) > 48 else ""
    return (f"\"{s['title']}\" in {s['key']}, {s['bpm']} bpm, {len(s['notes'])} notes over "
            f"{beats:g} beats. Notes (pitch:beats): {line}{more}. The piano view shows it.")


def vlq(n):
    out = [n & 0x7F]
    n >>= 7
    while n:
        out.insert(0, (n & 0x7F) | 0x80)
        n >>= 7
    return bytes(out)


def midi_file(s):
    """A format-0 Standard MIDI File: tempo, track name, the notes on channel 1."""
    ppq = 480
    events = []
    for n in s["notes"]:
        m = midi_of(n["p"])
        events.append((int(n["t"] * ppq), 1, bytes([0x90, m, 88])))
        events.append((int((n["t"] + n["d"]) * ppq), 0, bytes([0x80, m, 0])))
    events.sort(key=lambda e: (e[0], e[1]))
    title = s["title"].encode()[:120]
    tempo = 60_000_000 // s["bpm"]
    track = vlq(0) + b"\xff\x03" + vlq(len(title)) + title
    track += vlq(0) + b"\xff\x51\x03" + tempo.to_bytes(3, "big")
    now = 0
    for tick, _, data in events:
        track += vlq(tick - now) + data
        now = tick
    track += vlq(0) + b"\xff\x2f\x00"
    return b"MThd" + struct.pack(">IHHH", 6, 0, 1, ppq) + b"MTrk" + struct.pack(">I", len(track)) + track


def tools():
    s, n = {"type": "string"}, {"type": "integer"}
    note = {"type": "object", "properties": {"p": s, "t": {"type": "number"}, "d": {"type": "number"}},
            "required": ["p"]}
    score = {"title": s, "key": s, "bpm": n,
             "notes": {"type": "array", "items": {"anyOf": [note, s]}}}
    return [
        {"name": "compose",
         "description": ("Compose a short piano melody and show it in an editable piano roll the user can "
                         "play. key: like 'D minor' or 'Bb major'. bars: 1 to 16. Give `notes` to write the "
                         "melody yourself, as 'E4:0.5' strings (pitch:beats, one after the other) or "
                         "{p, t, d} objects (t = start beat); without notes a melody is made up."),
         "inputSchema": {"type": "object", "properties": dict(score, bars=n)},
         "_meta": {"ui": {"resourceUri": URI}}},
        {"name": "set_notes", "description": "Save the score the user edited in the piano view.",
         "inputSchema": {"type": "object", "properties": score, "required": ["notes"]},
         "_meta": {"ui": {"resourceUri": URI, "visibility": ["app"]}}},
        {"name": "export_midi", "description": "The score as a MIDI file (for the view's download).",
         "inputSchema": {"type": "object", "properties": score, "required": ["notes"]},
         "_meta": {"ui": {"visibility": ["app"]}}},
    ]


def result(s):
    return {"content": [{"type": "text", "text": describe(s)}], "structuredContent": s}


def call(name, args):
    if name == "compose":
        s = score_of(args)
        if not s["notes"]:
            bars = max(1, min(16, int(args.get("bars") or 4)))
            s["notes"] = melody(s["key"], bars, f"{s['title']}|{s['key']}|{bars}")
            if s["title"] == "untitled":
                s["title"] = "Little tune in " + s["key"]
        return result(s)
    if name == "set_notes":
        return result(score_of(args))
    if name == "export_midi":
        s = score_of(args)
        slug = "".join(c if c.isalnum() else "-" for c in s["title"].lower()).strip("-") or "melody"
        return {"content": [{"type": "resource", "resource": {
            "uri": f"file:///{slug}.mid", "mimeType": "audio/midi",
            "blob": base64.b64encode(midi_file(s)).decode()}}]}
    return None


VIEW_META = {"prefersBorder": True}
# MIDI is not in the spec's permissions: bise reads its own key next to
# `ui` (docs/mcp-apps.md C6); other hosts ignore it and the view falls
# back to its on-screen keyboard.
BISE_META = {"permissions": {"midi": {}}}


def res_meta():
    return {"ui": VIEW_META, "dev.bise/ui": BISE_META}


def answer(method, params):
    if method == "initialize":
        return {"protocolVersion": params.get("protocolVersion", "2025-06-18"),
                "capabilities": {"tools": {}, "resources": {}},
                "serverInfo": {"name": "piano-composer", "version": "1.0.0"}}
    if method == "tools/list":
        return {"tools": tools()}
    if method == "tools/call":
        r = call(params.get("name"), params.get("arguments") or {})
        if r is None:
            raise LookupError("unknown tool: " + str(params.get("name")))
        return r
    if method == "resources/list":
        return {"resources": [{"uri": URI, "name": "piano", "mimeType": MIME,
                               "description": "Piano roll: edit, play, export", "_meta": res_meta()}]}
    if method == "resources/read":
        if params.get("uri") != URI:
            raise LookupError("unknown resource: " + str(params.get("uri")))
        with open(os.path.join(HERE, "views", "piano.html")) as f:
            return {"contents": [{"uri": URI, "mimeType": MIME, "text": f.read(), "_meta": res_meta()}]}
    if method == "ping":
        return {}
    raise NotImplementedError(method)


def main():
    for line in sys.stdin:
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        if msg.get("id") is None:
            continue  # notifications: nothing to answer
        try:
            reply = {"result": answer(msg.get("method"), msg.get("params") or {})}
        except LookupError as e:
            reply = {"error": {"code": -32602, "message": str(e)}}
        except NotImplementedError as e:
            reply = {"error": {"code": -32601, "message": "method not found: " + str(e)}}
        except Exception as e:  # a bad argument is an error answer, never a dead server
            reply = {"error": {"code": -32603, "message": f"{type(e).__name__}: {e}"}}
        sys.stdout.write(json.dumps(dict(jsonrpc="2.0", id=msg["id"], **reply)) + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
