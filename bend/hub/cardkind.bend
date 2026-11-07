# hub/cardkind.bend — what a card's kind says (BISE-299): the user's
# kinds, the hub's items with numbered options, a page's own cards; and
# the open cards an older journal left or that hold a message. Out of
# model.bend (its line limit). Pure.
import Base
import ./model.bend as M
import ./gen/state.bend as S
import ./lazy.bend as LZ

# the hub's items with numbered options (opened by the hub's Rust side,
# `card_open`): the user's digit goes back to Rust (`card_choice`), which
# acts and closes the item; typed words go to main
def choice_kind(+k: String) -> Bool:
  String.eq(k, "merge") || String.eq(k, "review") || String.eq(k, "feature_try") || String.eq(k, "feature_merge") || String.eq(k, "update") || String.eq(k, "signin")

# a page's own card (plan-ok): a plan an agent proposes, or a page's
# question or drawn pick (notif-artifact), asked by that agent (core.bend
# page_card); his answer is the hub's page path
def page_kind(+k: String) -> Bool:
  String.eq(k, "plan") || String.eq(k, "question")

# BISE-299: the user's inbox holds only what needs the user, by kind, set
# at the card's creation: main's escalations (`question`: `sb card`,
# `drop`: main's drop of a task with attention) and, later, tool-call
# confirmations (`confirm`), and the hub's own items about a place
# (`merge`: a PR ready to merge, pr-design §6.3). Only the user answers or
# closes them. The agents' traffic (reports, blocked, overlap, crashes) is
# main's, never a card: an older journal's cards of other kinds close at
# boot.
def user_kind(+k: String) -> Bool:
  String.eq(k, "question") || String.eq(k, "drop") || String.eq(k, "confirm") || choice_kind(k) || page_kind(k)

# the ids of the open cards that are not the user's (an older journal's)
def legacy_cards(xs: List<&2, S.Card>) -> List<&2, Nat>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = legacy_cards(t)
      Bool.pick(List<&2, Nat>, user_kind(S.Card.kind(h)), rest, S.Card.id(h) <> rest)

# the open card that holds message `id` (an escalated question)
def card_holding(xs: List<&2, S.Card>, +id: Nat) -> Maybe<&2, Nat>:
  match xs:
    case Nil{}:
      None{}
    case Con{+h, +t}:
      LZ.pick(Maybe<&2, Nat>, M.maybe_nat_is(S.Card.for_msg(h), id), u => Some{S.Card.id(h)}, u => card_holding(t, id))
