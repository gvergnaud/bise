# bend/laws/forward.bend: msg-forward (architect m_41072): `sb send
# --forward m_<n>` and `sb spawn --forward m_<n>` (bend/hub/forward.bend).
# Closed by bend/proofs/forward.bend; the gate checks every
# bend/proofs/*.bend: a law here with no proof there fails it.
import Base
import ../hub/gen/state.bend as HS
import ../hub/core.bend as HC
import ../hub/forward.bend as HFW
import ../vendor/json.bend as J

# LAW: the forward's message carries the forwarded id as a typed field
# (Msg.fwd): no reader ever finds it in the header's text.
law forward_records_source:
  for +s: HC.Snd
  for +m: HS.Msg
  for +w: HS.Orig
  for +at: String
  {HC.Snd.fwd(HC.fwd_snd(s, m, w, at)) == Some{HS.Msg.id(m)} : Maybe<&2, Nat>}

# LAW: the brief of `sb spawn --forward` carries it too.
law forward_brief_records_source:
  for t: HC.NewTask
  for +m: HS.Msg
  for +w: HS.Orig
  for +at: String
  {HC.NewTask.fwd(HC.new_task.fwd(t, m, w, at)) == Some{HS.Msg.id(m)} : Maybe<&2, Nat>}

# LAW: a message the forwarder neither sent nor got (under none of its
# names) is refused, and nothing is sent.
law forward_only_own:
  for +names: List<&2, String>
  for +m: HS.Msg
  for +id: Nat
  for e: {HFW.mine(names, m) == False{} : Bool}
  {HFW.door.of(Some{m}, names, id) == HFW.FErr{HFW.not_yours(HS.Msg.id(m))} : HFW.Door}

# LAW: one it sent or got goes through, whole.
law forward_own_passes:
  for +names: List<&2, String>
  for +m: HS.Msg
  for +id: Nat
  for e: {HFW.mine(names, m) == True{} : Bool}
  {HFW.door.of(Some{m}, names, id) == HFW.FOk{m} : HFW.Door}

# LAW: an unknown id is refused.
law forward_unknown:
  for +names: List<&2, String>
  for +id: Nat
  {HFW.door.of(None{}, names, id) == HFW.FErr{HFW.unknown(id)} : HFW.Door}

# LAW: the forward's text is its lead (note, header), then the
# original's text whole (its words, as the shell read them: c6).
law forward_keeps_text:
  for +note: String
  for +m: HS.Msg
  for +w: HS.Orig
  for +by: String
  for +at: String
  {HFW.text(note, m, w, by, at) == String.append(HFW.lead(note, m, by, at), HS.Orig.text(w)) : String}

# LAW: no note: the lead is the header alone.
law forward_no_note:
  for +m: HS.Msg
  for +by: String
  for +at: String
  {HFW.lead("", m, by, at) == HFW.header(m, by, at) : String}

# LAW: the header names the original's id, its sender, its recipient,
# the forwarder and where to open it; no time given: none written.
law forward_names_source:
  for +m: HS.Msg
  for +by: String
  {HFW.header(m, by, "") == String.concat(["[forwarded ", HFW.mid(HS.Msg.id(m)), " · from ", HFW.who(HS.Msg.from(m)), " to ", HFW.who(HS.Msg.to(m)),
    " · forwarded by ", by, " · its place: sb show ", HFW.mid(HS.Msg.id(m)), "]\n"]) : String}

# LAW: no note: the forward's parts open on the forwarded part (the
# header's typed twin, architect m_41547), then the original's. With a
# note, its text part comes first (note_parts). That they render to the
# forward's text is pinned across Bend and Rust by
# rust/switchboard/src/core_forward_tests.rs (Bend has no parts render).
law forward_no_note_parts:
  for +m: HS.Msg
  for +w: HS.Orig
  for +by: String
  for +at: String
  {HFW.parts("", m, w, by, at) == J.stringify(J.JArr{HFW.fwd_part(m, by, at) <> HFW.orig(w)}) : String}

# LAW: an original without parts is forwarded as its text, one text
# part: a forward always has parts, so every client draws it from them.
law forward_plain_original:
  for +w: HS.Orig
  for e: {HS.Orig.parts(w) == "" : String}
  {HFW.orig(w) == [HFW.text_part(HS.Orig.text(w))] : List<&2, J.Json>}
