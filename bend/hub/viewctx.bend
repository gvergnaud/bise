# hub/viewctx.bend — an MCP App view's context for its agent (docs/mcp-apps.md,
# C5; mcp-apps review d5): `ui/update-model-context` keeps the view's last
# context (S.Vctx in X.R.ctx, runtime only, never journaled: lost at a restart, as
# architect m_49840 decided), and his next message to that agent carries
# them all once, before his words. Pure: the lists in, the lists out; the
# shell renders each context's text (bise_proto parts::render of its one
# Part::ViewContext), so the tag is never written here: text joins are
# render(a ++ b) == render(a) ++ "\n" ++ render(b) for these parts, which
# the shell's tests pin.
import Base

import ../vendor/json.bend as J

import ./gen/state.bend as S

import ./lazy.bend as LZ

# the same view: the same agent and call
def same(+c: S.Vctx, +agent: String, +call: String) -> Bool:
  String.eq(S.Vctx.agent(c), agent) && String.eq(S.Vctx.call(c), call)

def agent_of(+c: S.Vctx) -> String:
  S.Vctx.agent(c)

# the contexts but that view's
def drop_view(cs: List<&2, S.Vctx>, +agent: String, +call: String) -> List<&2, S.Vctx>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, +rest}:
      LZ.pick(List<&2, S.Vctx>, same(c, agent, call), u => drop_view(rest, agent, call), u => c <> drop_view(rest, agent, call))

# a view's new context: its last one only, after the others (law
# view_context_last_wins)
def put(cs: List<&2, S.Vctx>, +c: S.Vctx) -> List<&2, S.Vctx>:
  List.append(&2, S.Vctx, drop_view(cs, agent_of(c), S.Vctx.call(c)), [c])

# `agent`'s contexts, in order
def of(cs: List<&2, S.Vctx>, +agent: String) -> List<&2, S.Vctx>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, +rest}:
      LZ.pick(List<&2, S.Vctx>, String.eq(agent_of(c), agent), u => c <> of(rest, agent), u => of(rest, agent))

# the contexts but `agent`'s: taken by his message, or its agent stopped
# (laws view_context_taken_once, view_context_keeps_others)
def drop(cs: List<&2, S.Vctx>, +agent: String) -> List<&2, S.Vctx>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, +rest}:
      LZ.pick(List<&2, S.Vctx>, String.eq(agent_of(c), agent), u => drop(rest, agent), u => c <> drop(rest, agent))

def texts(cs: List<&2, S.Vctx>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, +rest}:
      S.Vctx.text(c) <> texts(rest)

def ctx_parts(cs: List<&2, S.Vctx>) -> List<&2, J.Json>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, +rest}:
      S.Vctx.part(c) <> ctx_parts(rest)

# his message's text with `cs` before it; none: his text as it was (law
# view_context_none_no_change)
def join_text(cs: List<&2, S.Vctx>, +text: String) -> String:
  match cs:
    case Nil{}:
      text
    case Con{+c, +rest}:
      String.join(List.append(&2, String, texts(c <> rest), [text]), "\n")

# his parts (msg-parts JSON, "" none) as a list: his words as one text
# part (bise_proto Part::Text, tagged by `kind`) when he sent none;
# unreadable: none
def his_parts.of(r: Result<&2, &2, String, J.Json>) -> List<&2, J.Json>:
  match r:
    case Done{J.JArr{+xs}}:
      xs
    case _:
      Nil{}

def his_parts(+parts: String, +text: String) -> List<&2, J.Json>:
  LZ.pick(List<&2, J.Json>, String.is_empty(parts),
    u => [J.obj([J.kv("kind", J.JStr{"text"}), J.kv("text", J.JStr{text})])],
    u => his_parts.of(J.parse(parts)))

# his message's parts with `cs` before them; none: as they were (law
# view_context_none_no_change)
def join_parts(cs: List<&2, S.Vctx>, +parts: String, +text: String) -> String:
  match cs:
    case Nil{}:
      parts
    case Con{+c, +rest}:
      J.stringify(J.JArr{List.append(&2, J.Json, ctx_parts(c <> rest), his_parts(parts, text))})
