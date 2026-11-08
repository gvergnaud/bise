# hub/forward.bend - msg-forward (architect m_41072): `sb send <agent>
# --forward m_<n> ["<note>"]` and `sb spawn --forward m_<n>`, a message
# forwarded by its id. Pure: the door, the text and the parts of the
# forward; core.bend sends it (laws in bend/laws/forward.bend).
#
# - the door: only a message the forwarder got or sent, under any of its
#   names (a renamed agent keeps its messages); an unknown id is refused;
# - the text: the forwarder's note (if any), a header line, then the
#   original's text whole (for a message with parts it is already their
#   render: its image markers reach the model). The header is for the
#   model only: every other reader takes the forwarded id from Msg.fwd,
#   never from this text;
# - the parts: the note's text part (if any), the `forwarded` part (it
#   renders to the header: bise_proto::parts::Part::Forwarded, architect
#   m_41547), then the original's parts, or its text as one text part, so
#   they render to the text (pinned end to end in core_forward_tests.rs)
#   and the clients draw the header as a line of its own;
# - `at`: the original's time as the shell formats it ("" leaves it out).
import Base
import ./model.bend as M
import ./gen/state.bend as S
import ./gen/state-codec.bend as SC
import ../vendor/json.bend as J

type Door is Data:
  FOk{m: S.Msg}
  FErr{e: String}

def mid(+n: Nat) -> String:
  String.append("m_", Nat.show(n))

def not_yours(+id: Nat) -> String:
  String.append(mid(id), " is not a message you got or sent: forward only your own thread's messages")

def unknown(+id: Nat) -> String:
  String.append(mid(id), " unknown: no such message to forward")

# the sender (`names`: all its names) sent or got `m`
def mine(+names: List<&2, String>, +m: S.Msg) -> Bool:
  M.str_in(names, S.Msg.from(m)) || M.str_in(names, S.Msg.to(m))

def door.pick(ok: Bool, +m: S.Msg) -> Door:
  match ok:
    case True{}:
      FOk{m}
    case False{}:
      FErr{not_yours(S.Msg.id(m))}

def door.of(m: Maybe<&2, S.Msg>, +names: List<&2, String>, +id: Nat) -> Door:
  match m:
    case None{}:
      FErr{unknown(id)}
    case Some{+x}:
      door.pick(mine(names, x), x)

# the message `id` for `sender`, or why not
def door(+st: S.St, +sender: String, +id: Nat) -> Door:
  door.of(M.msg_of(st, id), M.names_of(st, sender), id)

# who a party is, in the header
def who.pick(user: Bool, +n: String) -> String:
  match user:
    case True{}:
      "the user"
    case False{}:
      n

def who(+n: String) -> String:
  who.pick(String.eq(n, M.USER()), n)

def at_part(empty: Bool, +at: String) -> String:
  match empty:
    case True{}:
      ""
    case False{}:
      String.append(" · ", at)

# the header line (with its newline)
def header(+m: S.Msg, +by: String, +at: String) -> String:
  String.concat(["[forwarded ", mid(S.Msg.id(m)), " · from ", who(S.Msg.from(m)), " to ", who(S.Msg.to(m)),
    at_part(String.is_empty(at), at), " · forwarded by ", by, " · its place: sb show ", mid(S.Msg.id(m)), "]\n"])

def lead.pick(empty: Bool, +note: String, +h: String) -> String:
  match empty:
    case True{}:
      h
    case False{}:
      String.concat([note, "\n\n", h])

# what comes before the original's text: the note, then the header
def lead(+note: String, +m: S.Msg, +by: String, +at: String) -> String:
  lead.pick(M.trim_is_empty(note), note, header(m, by, at))

# the original's words: sb-core's state keeps no message's words (c6),
# the shell reads them from the journal and puts them on the request
# (`orig`, an S.Orig: its text, its parts' JSON or ""; switchboard
# forward.rs keys). Missing: no words; the shell's Missing text otherwise.
def orig_of(+q: J.Json) -> S.Orig:
  SC.Orig.journal_of(Maybe.default(&2, J.Json, J.get(q, "orig"), J.JNull{}))

# the forward's text: the lead, then the original's text whole
def text(+note: String, +m: S.Msg, +w: S.Orig, +by: String, +at: String) -> String:
  String.append(lead(note, m, by, at), S.Orig.text(w))

# a text part
def text_part(+t: String) -> J.Json:
  J.obj([J.kv("kind", J.JStr{"text"}), J.kv("text", J.JStr{t})])

# the forwarded part (architect m_41547): the original's id, who wrote it
# to whom, when ("" not said) and who forwards it; it renders to the
# header (bise_proto::parts::Part::Forwarded), and the clients draw it
# as its own line, never from the header's text
def fwd_part(+m: S.Msg, +by: String, +at: String) -> J.Json:
  J.obj([J.kv("kind", J.JStr{"forwarded"}), J.kv("id", J.JNum{S.Msg.id(m)}), J.kv("from", J.JStr{S.Msg.from(m)}),
    J.kv("to", J.JStr{S.Msg.to(m)}), J.kv("at", J.JStr{at}), J.kv("by", J.JStr{by})])

# the original's parts: its own, or its text as one text part
def orig.of(r: Result<&2, &2, String, J.Json>, +w: S.Orig) -> List<&2, J.Json>:
  match r:
    case Done{J.JArr{+xs}}:
      xs
    case _:
      [text_part(S.Orig.text(w))]

def orig(+w: S.Orig) -> List<&2, J.Json>:
  orig.of(J.parse(S.Orig.parts(w)), w)

# the note's part, with the blank line before the header (none: none)
def note_parts.pick(empty: Bool, +note: String) -> List<&2, J.Json>:
  match empty:
    case True{}:
      []
    case False{}:
      [text_part(String.append(note, "\n\n"))]

def note_parts(+note: String) -> List<&2, J.Json>:
  note_parts.pick(M.trim_is_empty(note), note)

# the forward's parts: the note's, the forwarded part, then the
# original's; never none, so every client draws it from its parts
def parts(+note: String, +m: S.Msg, +w: S.Orig, +by: String, +at: String) -> String:
  J.stringify(J.JArr{List.append(&2, J.Json, note_parts(note), fwd_part(m, by, at) <> orig(w))})

# `sb spawn --forward`: the brief, then its last section, the forwarded
# message (the header, then its text whole)
def brief_lead(+brief: String, +m: S.Msg, +by: String, +at: String) -> String:
  String.concat([brief, "\n\n## Forwarded message\n\n", header(m, by, at)])

def brief(+brief: String, +m: S.Msg, +w: S.Orig, +by: String, +at: String) -> String:
  String.append(brief_lead(brief, m, by, at), S.Orig.text(w))

# the brief's parts: the brief and its section's title, the forwarded
# part, then the original's
def brief_parts(+brief: String, +m: S.Msg, +w: S.Orig, +by: String, +at: String) -> String:
  J.stringify(J.JArr{text_part(String.concat([brief, "\n\n## Forwarded message\n\n"])) <> fwd_part(m, by, at) <> orig(w)})
