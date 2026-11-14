# bend/laws/inbox.bend: the inbox's history (inbox-record, architect
# m_24257): what a card's close journals of his answer, and its position
# in the asker's thread (hub/answers.bend). Closed by bend/proofs/inbox.bend;
# the gate checks every bend/proofs/*.bend: a law here with no proof there
# fails it.
import Base
import ../vendor/json.bend as J
import ../hub/model.bend as HM
import ../hub/gen/state.bend as HS
import ../hub/cx.bend as HX
import ../hub/core.bend as HC
import ../hub/answers.bend as HA
import ../hub/picks.bend as HPK
import ../hub/place.bend as HPL
import ../hub/cardkind.bend as CK

# LAW: a card closed without him (withdrawn, closed, an agent stopped)
# journals no answer.
law closed_without_him_has_no_answer:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +id: Nat
  for +res: String
  {HC.close_card(o, cx, id, res) == HC.close_card_ans(o, cx, id, res, HS.Ans.empty()) : HX.Cx<o>}

# LAW: his answer to a question journals his words as he said them, and
# the option's words when he picked by its number (the pick of the route
# line, picks.bend): the history never reads the digit alone.
law answered_close_carries_his_words:
  for +c: HS.Card
  for +said: String
  for +via: String
  {HA.his_ans(c, said, via) == HS.Ans.with_via(HS.Ans.with_pick(HS.Ans.with_pick_n(HS.Ans.with_said(HS.Ans.empty(), said), HPK.pick_n(HPK.option_pick(HS.Card.text(c), said))), HA.opt_words(HPK.pick_words(HPK.option_pick(HS.Card.text(c), said)))), via) : HS.Ans}

# LAW: the digit `2` on '2. only at checkout' journals the option's words.
law a_picked_digit_journals_its_words:
  {HA.his_ans(HS.Card.with_text(HS.Card.with_agent(HS.Card.with_kind(HS.Card.with_id(HS.Card.empty(), 7n), "question"), "perf"), "which one?
1. at every step
2. only at checkout"), "2", "inbox") == HS.Ans.with_via(HS.Ans.with_pick(HS.Ans.with_pick_n(HS.Ans.with_said(HS.Ans.empty(), "2"), Some{2n}), "only at checkout"), "inbox") : HS.Ans}

# LAW: an approval's digit journals its words (1 allow, 2 always, 3 no).
law an_approval_digit_journals_its_words:
  {HA.confirm_ans(" 2 ", "inbox") == HS.Ans.with_via(HS.Ans.with_pick(HS.Ans.with_pick_n(HS.Ans.with_said(HS.Ans.empty(), "2"), Some{2n}), "always"), "inbox") : HS.Ans}

# LAW: a card not due (closed without him, or its position given
# already) takes no position: nothing changes.
law answer_pos_not_due:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +card: Nat
  for +agent: String
  for +pos: Nat
  {HA.answer_pos.go(o, False{}, cx, card, agent, pos) == cx : HX.Cx<o>}

# LAW: the position is journaled at most once per card: a second
# answer_pos for the same card changes nothing.
law answer_pos_once:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +card: Nat
  for +agent: String
  for +pos: Nat
  for +agent2: String
  for +pos2: Nat
  {HA.answer_pos(o, HA.answer_pos(o, cx, card, agent, pos), card, agent2, pos2) == HA.answer_pos(o, cx, card, agent, pos) : HX.Cx<o>}

# drop-card (Gabriel: 1 for yes, 2 for no): main's drop card, with its
# options (place.bend drop_card), and a card opened before them
def drop_card(+text: String) -> HS.Card:
  HS.Card.with_text(HS.Card.with_agent(HS.Card.with_kind(HS.Card.with_id(HS.Card.empty(), 4n), "drop"), "fix"), text)

def new_drop() -> HS.Card:
  drop_card(HPL.drop_card(HPL.drop_body(True{}, 4n, 0n)))

def old_drop() -> HS.Card:
  drop_card("main suggests: archive @fix? 4 changed files and 0 unpushed commit will be saved (/restore). (answer yes or no)")

# LAW: main's drop card says what it keeps and shows its two options.
law drop_card_words:
  {HPL.drop_card(HPL.drop_body(True{}, 4n, 0n)) == "main suggests it. it's working right now: archiving stops its turn. its 4 changed files are kept; /restore brings it back.
1. archive it
2. keep it" : String}

# LAW: option 1 of a drop card archives exactly like `yes`, its pick in
# the history; option 2 keeps the agent exactly like `no`.
law drop_option_1_is_yes:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +parts: String
  for +via: String
  {HC.answer.kind(o, HC.KDrop{}, cx, new_drop(), "1", parts, via) == HC.answer.drop(o, True{}, cx, new_drop(), HA.his_ans(new_drop(), "1", via)) : HX.Cx<o>}

law drop_option_2_is_no:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +parts: String
  for +via: String
  {HC.answer.kind(o, HC.KDrop{}, cx, new_drop(), "2", parts, via) == HC.answer.drop(o, False{}, cx, new_drop(), HA.his_ans(new_drop(), "2", via)) : HX.Cx<o>}

# LAW: a digit on a drop card opened before its options is no option: it
# keeps the agent (the safe default); `yes` still archives it.
law old_drop_card_digit_keeps:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +parts: String
  for +via: String
  {HC.answer.kind(o, HC.KDrop{}, cx, old_drop(), "1", parts, via) == HC.answer.drop(o, False{}, cx, old_drop(), HA.his_ans(old_drop(), "1", via)) : HX.Cx<o>}

law old_drop_card_yes_archives:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +parts: String
  for +via: String
  {HC.answer.kind(o, HC.KDrop{}, cx, old_drop(), "yes", parts, via) == HC.answer.drop(o, True{}, cx, old_drop(), HA.his_ans(old_drop(), "yes", via)) : HX.Cx<o>}

# LAW (BISE-299, tui_inbox_split_tmux): the user's inbox holds only what
# needs him: main's escalations (question, drop), confirmations, the
# hub's items and a page's own cards. The agents' traffic (a report, a
# blocked task, an ask to main, an overlap, a crash) is never his card.
def his_kinds() -> List<&2, String>:
  ["question", "drop", "confirm", "merge", "plan"]

def traffic_kinds() -> List<&2, String>:
  ["report", "blocked", "ask", "overlap", "crash"]

def all_his(xs: List<&2, String>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Con{+h, +t}:
      Bool.and(CK.user_kind(h), all_his(t))

def none_his(xs: List<&2, String>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Con{+h, +t}:
      Bool.and(Bool.not(CK.user_kind(h)), none_his(t))

law only_his_kinds_are_his_cards:
  {Bool.and(all_his(his_kinds()), none_his(traffic_kinds())) == True{} : Bool}

# tasks (architect m_65127): an agent's blocked report is his card (the
# typed flag, never the kind): it reaches his inbox and stays at boot; an
# older journal's blocked card (no from_report) is still main's and
# closes at boot (BISE-299 holds for it).
def blocked_card(+id: Nat, from_report: Bool) -> HS.Card:
  HS.Card.with_from_report(HS.Card.with_kind(HS.Card.with_id(HS.Card.empty(), id), "blocked"), from_report)

law a_report_blocked_card_reaches_his_inbox:
  {Bool.and(CK.user_card(blocked_card(7n, True{})), List.is_empty(&2, Nat, CK.legacy_cards([blocked_card(7n, True{})]))) == True{} : Bool}

law legacy_blocked_still_closes_at_boot:
  {CK.legacy_cards([blocked_card(7n, False{})]) == [7n] : List<&2, Nat>}

# LAW: his own /archive of a working or dirty agent asks with the same
# drop card (architect m_37339): the facts alone (he asked, no 'main
# suggests it') and its 2 options; nothing is archived until he picks 1
# (answer.drop, the laws above).
law archive_ask_is_a_drop_card:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +q: String
  for +name: String
  {HC.user_drop.res(o, (cx, HC.DAsk{q}), name) == HC.open_card(o, cx, "drop", Maybe.default(&2, String, HM.resolve(HC.st(o, cx), name), name), HPL.drop_asked(q), None{}) : HX.Cx<o>}

# LAW: a drop that must ask changes nothing: it hands back the same
# context with what the card will say.
law a_drop_that_asks_archives_nothing:
  for -o: HS.St
  for +cx: HX.Cx<o>
  for +a: HS.Agent
  for +has_wt: Bool
  for +loss: J.Json
  for +busy: Bool
  {HC.drop.decide(o, True{}, cx, a, has_wt, loss, busy) == (cx, HC.DAsk{HPL.drop_body(busy, J.get_num_or(loss, "dirty", 0n), J.get_num_or(loss, "unpushed", 0n))}) : HX.Cx<o> & HC.DropR}
