# bend/proofs/parts.bend: the proofs of bend/laws/parts.bend. Each holds
# by unfolding: the message keeps the request's parts, and an empty parts
# string picks the branch that writes nothing.
import Base
import ../vendor/json.bend as J
import ../hub/model.bend as HM
import ../hub/gen/state.bend as HS
import ../hub/gen/state-laws.bend as HSL
import ../hub/gen/feed.bend as HFD
import ../hub/gen/kit.bend as HK
import ../hub/lazy.bend as HLZ
import ../hub/cx.bend as HX
import ../hub/core.bend as HC
import ../hub/newtask.bend as HNT
import ../laws/parts.bend as Laws

def Laws.agent_send_keeps_parts(from, to, expect, reply_to, q):
  {==}

def Laws.agent_no_parts_no_line(o, cx, agent, m):
  %Equal.sym(Bool, HS.Msg.has_parts(HS.Msg.with_has_parts(m, False{})), False{}, HSL.Msg.has_parts.get_with(m, False{})) : {HLZ.pick(HX.Cx<o>, Bool.not(_), u => cx,
    u => HC.feed(o, cx, agent, HFD.msg_parts(HS.Msg.id(HS.Msg.with_has_parts(m, False{})), HK.BParts{HS.Msg.id(HS.Msg.with_has_parts(m, False{}))}))) == cx : HX.Cx<o>}
  {==}

def Laws.agent_no_parts_no_sent_line(o, cx, agent, m):
  %Equal.sym(Bool, HS.Msg.has_parts(HS.Msg.with_has_parts(m, False{})), False{}, HSL.Msg.has_parts.get_with(m, False{})) : {HLZ.pick(HX.Cx<o>, Bool.not(_), u => cx,
    u => HC.feed(o, cx, agent, HFD.sent_parts(HS.Msg.id(HS.Msg.with_has_parts(m, False{})), HK.BParts{HS.Msg.id(HS.Msg.with_has_parts(m, False{}))}))) == cx : HX.Cx<o>}
  {==}

def Laws.brief_no_parts(head):
  {==}
