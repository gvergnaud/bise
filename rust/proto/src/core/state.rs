//! sb-core's durable state, declared once (debt-audit #1, state-schema):
//! every record sb-core keeps in its `St` (an agent, a message, a card,
//! a timer, a watch, a cross-hub delivery...), each enum inside them, and
//! the JSON shapes they are written in. tests/core_gen/state.rs writes
//! from [`STATE`]:
//!
//! - `bend/hub/gen/state.bend`: the Bend types, per field a getter
//!   `T.f(x)` and a setter `T.with_f(x, v)`, and `T.empty()` (every field
//!   its default). The only place a record is built or opened by
//!   position: everywhere else reads and changes it through these, so a
//!   new field is one row here and a bless;
//! - `bend/hub/gen/state-laws.bend`: per field `T.f.get_with` (a setter
//!   then its getter gives the value) and `T.f.eta` (a setter is the
//!   record rebuilt from its getters: rewrite with it, then any other
//!   getter computes);
//! - `bend/hub/gen/state-codec.bend`: per readable [`Shape`]
//!   `T.<shape>_kvs` / `_json` / `_of` and its round law (`_of` of
//!   `_json` gives back the fields the shape carries, the others at
//!   their default), one rewrite per field; per write-only shape
//!   `_kvs` / `_json` only. Per enum its words, both ways, and its law.
//!
//! The JSON is today's exactly (key order, omitted keys, the first key
//! wins on a read, a value of the wrong type reads the default): the
//! journal and view fixtures pin each shape.
//!
//! A new field: one [`Field`] in its record (and a [`Key`] in each shape
//! that carries it), `BISE_PROTO_BLESS=1 cargo test -p bise-proto --test
//! core_gen`, then its use.

/// A field's Bend type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Nat,
    Str,
    Bool,
    /// `J.Json`, kept as is
    Json,
    /// `Maybe<&2, J.Json>`, kept as is: a value sb-core stores and
    /// never reads (a card's `about`), absent when none
    OptJson,
    OptNat,
    OptStr,
    /// `List<&2, String>`
    Strs,
    /// `List<&2, Nat>`
    Nats,
    /// a record or enum of [`STATE`]
    T(&'static str),
    /// `Maybe` of a record or enum of [`STATE`]
    OptT(&'static str),
    /// `List` of a record of [`STATE`]
    Ts(&'static str),
}

/// One field of a record (or of an enum's variant), in Bend order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    pub name: &'static str,
    pub ty: Ty,
    /// its value in `T.empty()` and when a shape doesn't carry it or a
    /// read finds no key, as a Bend term, when it is not its type's own
    /// (0n, "", False{}, None{}, Nil{}, J.JNull{}, `T.empty()`); an enum
    /// field always says it
    pub default: Option<&'static str>,
}

/// How a field is written in a shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wire {
    /// a string, always written
    Str,
    /// a string, written only when not empty
    StrOmit,
    Nat,
    Bool,
    /// a bool, written (as true) only when true
    Flag,
    /// `Maybe<String>`, null when none
    OptStr,
    /// `Maybe<String>`, absent when none
    OptStrOmit,
    /// `Maybe<Nat>`, null when none
    OptNat,
    /// `Maybe<Nat>`, absent when none
    OptNatOmit,
    /// any JSON, as is
    Json,
    /// `Maybe` of any JSON, as is, absent when none (an old line reads
    /// none, and a none writes nothing: its lines stay byte-identical)
    OptJsonOmit,
    /// a list of strings
    Strs,
    /// a list of numbers
    Nats,
    /// an enum of words, as its word (`Life`: "active")
    Word,
    /// `Maybe` of an enum of words: its word, null when none
    OptWord,
    /// a record or a tagged enum, as an object: its shape of the same
    /// name (an enum: its word under its tag key, its variant's keys)
    Obj,
    /// `Maybe` of a record, as an object (its shape of the same name,
    /// else its journal's), absent when none
    OptObjOmit,
    /// a record or a tagged enum whose keys sit in this object (a
    /// message's state: `state`, `reason`; an ended timer's timer)
    Inline,
    /// an enum written by a hand codec ([`Hand`]); its [`Key`]'s `key`
    /// is the first of the hand's keys
    Hand(&'static Hand),
    /// a list of records, each as its shape of the same name (else its
    /// journal shape), read back whole (a photo's lists)
    Recs,
}

/// A hand codec of bend/hub/state-hand.bend (imported `SH`), for an enum
/// whose JSON is one key-value whose key says the variant (a timer's
/// schedule: `every_ms` or `daily_min`), read back normalised:
/// `SH.<name>_kv(v)` writes it; `SH.<name>_of(m1, m2, ..)` reads it from
/// the lookups of `keys` (in their order), none when absent (the whole
/// shape then reads none: an every_set without a schedule is skipped);
/// `SH.<name>_norm(v)` is what a written `v` reads back as (its round
/// law, in the record's: `_of` of `_kv(v)` is `Some{_norm(v)}`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hand {
    pub name: &'static str,
    /// each variant and the key its value is written under
    pub keys: &'static [(&'static str, &'static str)],
}

/// A hand read of a field from the lookups of `keys` (in their order),
/// `SH.<name>(m1, m2, ..)`, instead of its own key's: a watch's
/// `resumes`, missing in an old line, reads from its spec. Its round is
/// its key's: the written value comes back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandRead {
    pub name: &'static str,
    pub keys: &'static [&'static str],
}

/// One key of a shape: the field it carries, its JSON key, how.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub field: &'static str,
    pub key: &'static str,
    pub wire: Wire,
    pub read: Option<&'static HandRead>,
}

/// A timer's schedule (every.rs's shape): `every_ms` (at least 1 s) or
/// `daily_min` (mod a day).
pub const SCHED: Hand = Hand { name: "sched", keys: &[("Every", "every_ms"), ("Daily", "daily_min")] };

/// A watch's `resumes`, or its spec's default when an old line has none.
pub const WATCH_RESUMES: HandRead = HandRead { name: "watch_resumes", keys: &["resumes", "spec"] };

/// One JSON shape of a record, its keys in wire order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    /// `journal` (what a journal line carries, read back at boot),
    /// `view` (what sb-core's view writes for the hub's mirror) or
    /// `photo` (the whole St a boot starts from, issue 36: lossless, its
    /// round law is identity; a record without one is written in its
    /// journal shape, whose round law must then be identity too)
    pub name: &'static str,
    /// sb-core reads it back (`_of` and its round law); a write-only
    /// shape gets `_kvs` and a law that it writes exactly its keys
    pub read: bool,
    pub keys: &'static [Key],
}

/// One record of sb-core's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    pub name: &'static str,
    pub doc: &'static str,
    pub fields: &'static [Field],
    pub shapes: &'static [Shape],
}

impl Record {
    pub fn field(&self, name: &str) -> &'static Field {
        self.fields
            .iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("{}: no field {name}", self.name))
    }

    pub fn shape(&self, name: &str) -> Option<&'static Shape> {
        self.shapes.iter().find(|s| s.name == name)
    }
}

/// One variant of an enum: its Bend constructor, its word on the wire,
/// its fields (their keys sit beside the word, under their names).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Variant {
    pub name: &'static str,
    pub word: &'static str,
    pub fields: &'static [Field],
}

/// One enum inside the records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Enum {
    pub name: &'static str,
    pub doc: &'static str,
    /// the key of its word when it is written as an object or inline
    /// (`state`, `kind`); "" an enum written as its bare word
    pub tag_key: &'static str,
    pub variants: &'static [Variant],
    /// what any other word (or none) reads: a variant's name, its
    /// fields read from the object as usual
    pub fallback: &'static str,
}

/// The whole declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    pub enums: &'static [Enum],
    pub records: &'static [Record],
}

impl State {
    pub fn record(&self, name: &str) -> Option<&'static Record> {
        self.records.iter().find(|r| r.name == name)
    }

    pub fn enum_(&self, name: &str) -> Option<&'static Enum> {
        self.enums.iter().find(|e| e.name == name)
    }
}

const fn f(name: &'static str, ty: Ty) -> Field {
    Field { name, ty, default: None }
}

const fn fd(name: &'static str, ty: Ty, default: &'static str) -> Field {
    Field { name, ty, default: Some(default) }
}

/// A key named as its field.
const fn k(field: &'static str, wire: Wire) -> Key {
    Key { field, key: field, wire, read: None }
}

/// A key with its own JSON name.
const fn kn(field: &'static str, key: &'static str, wire: Wire) -> Key {
    Key { field, key, wire, read: None }
}

/// A record's photo shape (read back, every field).
const fn photo(keys: &'static [Key]) -> Shape {
    Shape { name: "photo", read: true, keys }
}

const fn v(name: &'static str, word: &'static str) -> Variant {
    Variant { name, word, fields: &[] }
}

const fn vf(name: &'static str, word: &'static str, fields: &'static [Field]) -> Variant {
    Variant { name, word, fields }
}

use Ty::*;
use Wire as W;

/// Every record and enum of sb-core's state, enums first (a record's
/// field names one), each record after the ones it holds.
pub const STATE: State = State {
    enums: &[
        Enum {
            name: "Life",
            doc: "what the owner of a task decided (never automatic)",
            tag_key: "",
            variants: &[v("LActive", "active"), v("LStopped", "stopped"), v("LArchived", "archived"), v("LFailed", "failed")],
            fallback: "LActive",
        },
        Enum {
            name: "Run",
            doc: "what an agent's REPL is doing (runtime only)",
            tag_key: "",
            variants: &[v("RDown", "down"), v("RStarting", "starting"), v("RIdle", "idle"), v("RBusy", "busy")],
            fallback: "RDown",
        },
        Enum {
            name: "Decl",
            doc: "the status an agent declares (`sb status`)",
            tag_key: "",
            variants: &[v("DWorking", "working"), v("DDone", "done"), v("DBlocked", "blocked")],
            fallback: "DWorking",
        },
        Enum {
            name: "MSt",
            doc: "a message's delivery state; any other word reads queued",
            tag_key: "state",
            variants: &[
                vf("Queued", "queued", &[f("reason", Str)]),
                v("Delivered", "delivered"),
                vf("Rejected", "rejected", &[f("error", Str)]),
                v("Cancelled", "cancelled"),
            ],
            fallback: "Queued",
        },
        Enum {
            name: "Sched",
            doc: "when a timer fires: every `ms`, or every day at that minute (local time, the hub's next_daily)",
            // its tagged object is the photo's (exact); a journal line
            // writes it through its hand codec (SCHED, read normalised)
            tag_key: "kind",
            variants: &[vf("Every", "every", &[f("ms", Nat)]), vf("Daily", "daily", &[f("min", Nat)])],
            fallback: "Every",
        },
        Enum {
            name: "What",
            doc: "what an act does: send a held route (its id), try a cross-hub delivery again (its xid), or close a view's approval card he left unanswered (its card id; docs/mcp-apps.md, C5)",
            tag_key: "kind",
            variants: &[
                vf("RouteSend", "route_send", &[f("rid", Nat)]),
                vf("Retry", "retry", &[f("xid", Nat)]),
                vf("CardDue", "card_due", &[f("card", Nat)]),
            ],
            fallback: "RouteSend",
        },
        Enum {
            name: "XKind",
            doc: "a cross-hub delivery: his words forwarded (input), bise's question (ask), a followed job's end (job_end)",
            tag_key: "",
            variants: &[v("KInput", "input"), v("KAsk", "ask"), v("KJobEnd", "job_end")],
            fallback: "KInput",
        },
        Enum {
            name: "OutSt",
            doc: "an outbox entry: pending, acked by the other hub, settled",
            tag_key: "",
            variants: &[v("OPending", "pending"), v("OAcked", "acked"), v("OSettled", "settled")],
            fallback: "OPending",
        },
        Enum {
            name: "RSt",
            doc: "a route: held until its act, sent, cancelled",
            tag_key: "",
            variants: &[v("RHeld", "held"), v("RSent", "sent"), v("RCancelled", "cancelled")],
            fallback: "RHeld",
        },
    ],
    records: &[
        Record {
            name: "Msg",
            doc: "a message between two parties of the group (the sender or the recipient may be `user`), without its words (c6: the journal's `message_sent` line keeps them, a `Sent`; what sb-core writes of them is a body the shell fills, body.rs)",
            fields: &[
                f("id", Nat),
                f("thread", Nat),
                f("from", Str),
                f("to", Str),
                f("reply_to", OptNat),
                f("expect", Bool),
                f("auto", Bool),
                f("created", Nat),
                f("plain", Bool),
                f("queued", Bool),
                f("via", OptStr),
                // it has parts (msg-parts): its msg-parts and sent-parts
                // lines are written; the parts are in its Sent line
                f("has_parts", Bool),
                // msg-forward: the message this one forwards (`sb send
                // --forward m_<n>`), none for any other; its text says it
                // for the model, this field for every other reader
                f("fwd", OptNat),
                // mcp-apps: the plugin whose view wrote it (`from` is
                // the fixed sender `view`); none for any other message
                f("view", OptStr),
            ],
            // no journal line carries a Msg (message_sent carries a
            // Sent): this is the shape the view and the photo write
            shapes: &[Shape {
                name: "journal",
                read: true,
                keys: &[
                    k("id", W::Nat),
                    k("thread", W::Nat),
                    k("from", W::Str),
                    k("to", W::Str),
                    k("reply_to", W::OptNat),
                    kn("expect", "expect_reply", W::Bool),
                    k("auto", W::Bool),
                    kn("created", "created_ms", W::Nat),
                    k("plain", W::Bool),
                    k("queued", W::Bool),
                    k("via", W::OptStrOmit),
                    k("has_parts", W::Bool),
                    k("fwd", W::OptNatOmit),
                    k("view", W::OptStrOmit),
                ],
            }],
        },
        Record {
            name: "Sent",
            doc: "a message as its `message_sent` journal line carries it: its Msg and its words (`text`, `parts` as JSON, \"\" none). sb-core makes it, journals it and keeps the Msg (c6): the words stay in the journal",
            fields: &[
                f("id", Nat),
                f("thread", Nat),
                f("from", Str),
                f("to", Str),
                f("reply_to", OptNat),
                f("expect", Bool),
                f("auto", Bool),
                f("text", Str),
                f("created", Nat),
                f("plain", Bool),
                f("queued", Bool),
                f("via", OptStr),
                // his message's parts as JSON (msg-parts); "" none
                f("parts", Str),
                f("fwd", OptNat),
                f("view", OptStr),
            ],
            shapes: &[Shape {
                name: "journal",
                read: true,
                keys: &[
                    k("id", W::Nat),
                    k("thread", W::Nat),
                    k("from", W::Str),
                    k("to", W::Str),
                    k("reply_to", W::OptNat),
                    kn("expect", "expect_reply", W::Bool),
                    k("auto", W::Bool),
                    k("text", W::Str),
                    kn("created", "created_ms", W::Nat),
                    k("plain", W::Bool),
                    k("queued", W::Bool),
                    k("via", W::OptStrOmit),
                    k("parts", W::StrOmit),
                    k("fwd", W::OptNatOmit),
                    k("view", W::OptStrOmit),
                ],
            }],
        },
        Record {
            name: "Orig",
            doc: "a forwarded message's words as the shell puts them on a forward request (`orig`, switchboard forward.rs): sb-core keeps no message's words (c6), the journal does",
            fields: &[f("text", Str), f("parts", Str)],
            shapes: &[Shape { name: "journal", read: true, keys: &[k("text", W::Str), k("parts", W::StrOmit)] }],
        },
        Record {
            name: "MRec",
            doc: "a message, its delivery state and whether it got its (first) reply",
            fields: &[f("msg", T("Msg")), fd("st", T("MSt"), "Queued{\"\"}"), f("settled", Bool)],
            shapes: &[photo(&[k("msg", W::Obj), k("st", W::Obj), k("settled", W::Bool)])],
        },
        Record {
            // runtime only (cx.bend's R.ctx, never journaled): no shape
            name: "Vctx",
            doc: "an MCP App view's context for its agent's next message from him (mcp-apps review d5): the view (`agent`, `call`), its model text (the shell's render) and its Part::ViewContext",
            fields: &[f("agent", Str), f("call", Str), f("text", Str), f("part", Json)],
            shapes: &[],
        },
        Record {
            name: "ViewAsk",
            doc: "what a view's card asks (mcp-apps, C5): the view's agent and call, and its ask (the tool and its arguments as canonical JSON, the shell's): sb-core reads a view's cards by it, never by their place",
            fields: &[f("agent", Str), f("call", Str), f("ask", Str)],
            shapes: &[Shape { name: "journal", read: true, keys: &[k("agent", W::Str), k("call", W::Str), k("ask", W::Str)] }],
        },
        Record {
            name: "Card",
            doc: "a card of his inbox; `place`, `pr`: a place's card (ready to merge: its id, PR); `view`: a view's call's card (mcp-apps); `about`: the artifact it is about (the shell's ArtLink, opaque: sb-core stores it, never reads it)",
            fields: &[
                f("id", Nat),
                f("kind", Str),
                f("agent", Str),
                f("text", Str),
                f("for_msg", OptNat),
                f("created", Nat),
                f("place", OptStr),
                f("pr", OptNat),
                f("view", OptT("ViewAsk")),
                f("about", OptJson),
            ],
            shapes: &[Shape {
                name: "journal",
                read: true,
                keys: &[
                    k("id", W::Nat),
                    k("kind", W::Str),
                    k("agent", W::Str),
                    k("text", W::Str),
                    k("for_msg", W::OptNat),
                    kn("created", "created_ms", W::Nat),
                    k("place", W::OptStr),
                    k("pr", W::OptNat),
                    k("view", W::OptObjOmit),
                    k("about", W::OptJsonOmit),
                ],
            }],
        },
        Record {
            name: "Ans",
            doc: "his answer as a card's close journals it: `said`, the option `pick_n`/`pick` (\"\" none), `via` inbox|thread|page (\"\" closed without him), the page's question `page`/`block`",
            fields: &[f("said", Str), f("pick_n", OptNat), f("pick", Str), f("via", Str), f("page", Str), f("block", Str)],
            shapes: &[Shape {
                name: "journal",
                read: true,
                keys: &[
                    k("said", W::StrOmit),
                    k("pick_n", W::OptNatOmit),
                    k("pick", W::StrOmit),
                    k("via", W::StrOmit),
                    k("page", W::StrOmit),
                    k("block", W::StrOmit),
                ],
            }],
        },
        Record {
            // its view row is core::view's ViewAgent (with the runtime's
            // run, waiting, turns): view.bend fills it from the getters
            name: "Agent",
            doc: "an agent of the group; `dir` its folder (stable on a rename); `follow` the hub that follows it (\"\" this one)",
            fields: &[
                f("name", Str),
                f("is_main", Bool),
                f("parent", OptStr),
                fd("life", T("Life"), "LActive{}"),
                f("declared", OptT("Decl")),
                f("aliases", Strs),
                f("files", Strs),
                f("ws", Json),
                f("dir", Str),
                f("brief", Json),
                f("created", Nat),
                f("failure", OptStr),
                f("note", Str),
                f("report", Json),
                f("snap", OptStr),
                f("follow", OptStr),
            ],
            shapes: &[photo(&[
                k("name", W::Str),
                k("is_main", W::Bool),
                k("parent", W::OptStr),
                k("life", W::Word),
                k("declared", W::OptWord),
                k("aliases", W::Strs),
                k("files", W::Strs),
                k("ws", W::Json),
                k("dir", W::Str),
                k("brief", W::Json),
                k("created", W::Nat),
                k("failure", W::OptStr),
                k("note", W::Str),
                k("report", W::Json),
                k("snap", W::OptStr),
                k("follow", W::OptStr),
            ])],
        },
        Record {
            name: "Item",
            doc: "a card main withdrew he may still answer (gone.bend)",
            fields: &[f("id", Nat), f("text", Str)],
            shapes: &[photo(&[k("id", W::Nat), k("text", W::Str)])],
        },
        Record {
            name: "Timer",
            doc: "an `sb every` timer: `next` when it is due; `last` when it last woke its agent (0 never); `runs` its last runs, oldest first; `name` what the lists show; `held` set without a name, its set line not written yet",
            fields: &[
                f("id", Nat),
                f("agent", Str),
                f("by", Str),
                f("text", Str),
                fd("sched", T("Sched"), "Every{0n}"),
                f("next", Nat),
                f("until", OptNat),
                f("times", OptNat),
                f("fired", Nat),
                f("page", OptStr),
                f("last", Nat),
                f("runs", Nats),
                f("name", Str),
                f("held", Bool),
            ],
            shapes: &[
                // every_set, in the shape every.rs wrote it (old hubs
                // read it); its `at` is the line's
                Shape {
                    name: "journal",
                    read: true,
                    keys: &[
                        k("id", W::Nat),
                        k("agent", W::Str),
                        k("by", W::Str),
                        k("text", W::Str),
                        kn("next", "next_ms", W::Nat),
                        kn("sched", "every_ms", W::Hand(&SCHED)),
                        kn("until", "until_ms", W::OptNatOmit),
                        k("times", W::OptNatOmit),
                        k("page", W::OptStrOmit),
                        k("name", W::StrOmit),
                        k("held", W::Flag),
                    ],
                },
                Shape {
                    name: "view",
                    read: false,
                    keys: &[
                        k("id", W::Nat),
                        k("agent", W::Str),
                        k("by", W::Str),
                        k("text", W::Str),
                        kn("sched", "every_ms", W::Hand(&SCHED)),
                        kn("next", "next_ms", W::Nat),
                        k("fired", W::Nat),
                        kn("last", "last_ms", W::Nat),
                        k("runs", W::Nats),
                        kn("until", "until_ms", W::OptNatOmit),
                        k("times", W::OptNatOmit),
                        k("page", W::OptStrOmit),
                        k("name", W::StrOmit),
                    ],
                },
                // exact: its schedule as its tagged object, never normalised
                photo(&[
                    k("id", W::Nat),
                    k("agent", W::Str),
                    k("by", W::Str),
                    k("text", W::Str),
                    k("sched", W::Obj),
                    k("next", W::Nat),
                    k("until", W::OptNat),
                    k("times", W::OptNat),
                    k("fired", W::Nat),
                    k("page", W::OptStr),
                    k("last", W::Nat),
                    k("runs", W::Nats),
                    k("name", W::Str),
                    k("held", W::Bool),
                ]),
            ],
        },
        Record {
            name: "TimerEnd",
            doc: "a timer that ended: when (0: an old line without it) and why",
            fields: &[f("t", T("Timer")), f("at", Nat), f("why", Str)],
            shapes: &[
                Shape {
                    name: "view",
                    read: false,
                    keys: &[k("t", W::Inline), kn("at", "ended_ms", W::Nat), k("why", W::Str)],
                },
                photo(&[k("t", W::Obj), k("at", W::Nat), k("why", W::Str)]),
            ],
        },
        Record {
            name: "Act",
            doc: "an internal step sb-core takes at a time, on the timers' clock",
            fields: &[f("id", Nat), f("due", Nat), fd("what", T("What"), "RouteSend{0n}")],
            shapes: &[Shape {
                name: "journal",
                read: true,
                keys: &[k("id", W::Nat), k("due", W::Nat), k("what", W::Obj)],
            }],
        },
        Record {
            name: "Watch",
            doc: "an `sb wake` watch: `spec` what it looks at; `max_text` the wake at its max (\"\" with `quiet`); `quiet` its max ends it without a wake; `resumes` a pause (True) or an alarm",
            fields: &[
                f("id", Nat),
                f("agent", Str),
                f("spec", Json),
                f("max_text", Str),
                f("set_at", Nat),
                f("max_at", Nat),
                f("quiet", Bool),
                f("resumes", Bool),
            ],
            shapes: &[Shape {
                name: "journal",
                read: true,
                keys: &[
                    k("id", W::Nat),
                    k("agent", W::Str),
                    k("spec", W::Json),
                    k("max_text", W::Str),
                    k("set_at", W::Nat),
                    k("max_at", W::Nat),
                    k("quiet", W::Bool),
                    // an old line has none: its spec says (a pid's
                    // watch resumes, wakes.bend resumes_of)
                    Key { field: "resumes", key: "resumes", wire: W::Bool, read: Some(&WATCH_RESUMES) },
                ],
            }, photo(&[
                k("id", W::Nat),
                k("agent", W::Str),
                k("spec", W::Json),
                k("max_text", W::Str),
                k("set_at", W::Nat),
                k("max_at", W::Nat),
                k("quiet", W::Bool),
                // read plainly: a photo always has it
                k("resumes", W::Bool),
            ])],
        },
        Record {
            name: "WatchEnd",
            doc: "a watch that ended: when and why (hit, max, stop, gone)",
            fields: &[f("w", T("Watch")), f("at", Nat), f("why", Str)],
            shapes: &[
                Shape {
                    name: "view",
                    read: false,
                    keys: &[k("w", W::Inline), kn("at", "ended_at", W::Nat), k("why", W::Str)],
                },
                photo(&[k("w", W::Obj), k("at", W::Nat), k("why", W::Str)]),
            ],
        },
        Record {
            name: "Wakes",
            doc: "the watches of `sb wake`: live, ended (newest first), the last id given",
            fields: &[f("live", Ts("Watch")), f("ended", Ts("WatchEnd")), f("last_id", Nat)],
            shapes: &[photo(&[k("live", W::Recs), k("ended", W::Recs), k("last_id", W::Nat)])],
        },
        Record {
            name: "Tms",
            doc: "the agents' wake sources: the timers, the acts on the same clock, the watches",
            fields: &[
                f("live", Ts("Timer")),
                f("ended", Ts("TimerEnd")),
                f("last_id", Nat),
                f("acts", Ts("Act")),
                fd("next_act", Nat, "1n"),
                fd("wk", T("Wakes"), "Wakes.empty()"),
            ],
            shapes: &[photo(&[
                k("live", W::Recs),
                k("ended", W::Recs),
                k("last_id", W::Nat),
                k("acts", W::Recs),
                k("next_act", W::Nat),
                k("wk", W::Obj),
            ])],
        },
        Record {
            name: "Out",
            doc: "an outbox entry: a delivery to another project's hub, until it is settled",
            fields: &[
                f("xid", Nat),
                f("project", Str),
                fd("kind", T("XKind"), "KInput{}"),
                f("msg", Nat),
                f("text", Str),
                fd("st", T("OutSt"), "OPending{}"),
                f("tries", Nat),
                f("context", Str),
                f("parts", Str),
            ],
            shapes: &[Shape {
                name: "journal",
                read: true,
                keys: &[
                    k("xid", W::Nat),
                    k("project", W::Str),
                    k("kind", W::Word),
                    k("msg", W::Nat),
                    k("text", W::Str),
                    kn("st", "state", W::Word),
                    k("tries", W::Nat),
                    k("context", W::Str),
                    k("parts", W::StrOmit),
                ],
            }],
        },
        Record {
            name: "XIn",
            doc: "a delivery another hub made here, and whether it got its reply",
            fields: &[f("hub", Str), f("xid", Nat), f("msg", Nat), fd("kind", T("XKind"), "KInput{}"), f("replied", Bool)],
            shapes: &[Shape {
                name: "journal",
                read: true,
                keys: &[k("hub", W::Str), k("xid", W::Nat), k("msg", W::Nat), k("kind", W::Word), k("replied", W::Bool)],
            }],
        },
        Record {
            name: "Route",
            doc: "a message routed to another project, held until its act (`due`), then sent or cancelled",
            fields: &[
                f("rid", Nat),
                f("text", Str),
                f("via", Str),
                f("to", Str),
                f("name", Str),
                f("due", Nat),
                f("act", Nat),
                f("why", Str),
                fd("st", T("RSt"), "RHeld{}"),
                f("context", Str),
                f("parts", Str),
            ],
            shapes: &[Shape {
                name: "journal",
                read: true,
                keys: &[
                    k("rid", W::Nat),
                    k("text", W::Str),
                    k("via", W::Str),
                    k("to", W::Str),
                    k("name", W::Str),
                    kn("due", "due_ms", W::Nat),
                    k("act", W::Nat),
                    k("why", W::Str),
                    kn("st", "state", W::Word),
                    k("context", W::Str),
                    k("parts", W::StrOmit),
                ],
            }],
        },
        Record {
            name: "Xs",
            doc: "the cross-hub state: the outbox, the deliveries in, the routes",
            fields: &[
                f("outbox", Ts("Out")),
                fd("next_xid", Nat, "1n"),
                f("xin", Ts("XIn")),
                f("routes", Ts("Route")),
                fd("next_rid", Nat, "1n"),
            ],
            shapes: &[photo(&[
                k("outbox", W::Recs),
                k("next_xid", W::Nat),
                k("xin", W::Recs),
                k("routes", W::Recs),
                k("next_rid", W::Nat),
            ])],
        },
        Record {
            name: "St",
            doc: "the hub's durable state; `paused` since that ms (None runs); `hot`/`old` the messages split (hot ids ascending, old newest first); `gone` the withdrawn cards he may still answer, newest first; `agents`/`arch` the agents split (agents: not archived, in creation order; arch: archived, newest archived first; `order` every name, in creation order)",
            fields: &[
                f("agents", Ts("Agent")),
                f("order", Strs),
                f("hot", Ts("MRec")),
                f("cards", Ts("Card")),
                f("notes", Strs),
                fd("next_msg", Nat, "1n"),
                fd("next_card", Nat, "1n"),
                fd("tm", T("Tms"), "Tms.empty()"),
                fd("x", T("Xs"), "Xs.empty()"),
                f("paused", OptNat),
                f("old", Ts("MRec")),
                f("gone", Ts("Item")),
                f("arch", Ts("Agent")),
            ],
            // the photo a boot starts from (issue 36): every field
            shapes: &[photo(&[
                k("agents", W::Recs),
                k("order", W::Strs),
                k("hot", W::Recs),
                k("cards", W::Recs),
                k("notes", W::Strs),
                k("next_msg", W::Nat),
                k("next_card", W::Nat),
                k("tm", W::Obj),
                k("x", W::Obj),
                k("paused", W::OptNat),
                k("old", W::Recs),
                k("gone", W::Recs),
                k("arch", W::Recs),
            ])],
        },
    ],
};

/// The records and enums of [`STATE`] a Rust reader parses (switchboard's
/// mirror of sb-core's view), with their Rust names. core_gen writes
/// their types in `state/generated.rs` from their journal shape (a field
/// is named as its key; a variant as its word, in CamelCase), read the
/// way sb-core reads them: a missing key, or a value of another type,
/// reads the field's default; an enum's other word reads its fallback.
/// The others get a Rust name when a Rust reader needs one.
pub const RUST: &[(&str, &str)] =
    &[("Life", "Lifecycle"), ("Run", "Run"), ("Decl", "Declared"), ("MSt", "MsgState"), ("Msg", "Msg"), ("ViewAsk", "ViewAsk"), ("Card", "Card")];

mod generated;
mod opaque;
mod read;

pub use generated::*;
pub use opaque::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// Every name a field or a shape gives is declared, and a record comes
    /// after the records it holds (gen/state.bend's `empty()` calls
    /// theirs).
    #[test]
    fn the_declaration_holds_together() {
        let mut seen: Vec<&str> = STATE.enums.iter().map(|e| e.name).collect();
        for r in STATE.records {
            for fl in r.fields {
                if let T(n) | OptT(n) | Ts(n) = fl.ty {
                    assert!(seen.contains(&n), "{}.{}: {n} is not declared before it", r.name, fl.name);
                }
                if let T(n) = fl.ty {
                    if STATE.enum_(n).is_some() {
                        assert!(fl.default.is_some(), "{}.{}: an enum field says its default", r.name, fl.name);
                    }
                }
            }
            for s in r.shapes {
                let mut keys: Vec<&str> = Vec::new();
                for key in s.keys {
                    let fl = r.field(key.field);
                    assert!(!keys.contains(&key.key), "{}.{}: key {} twice", r.name, s.name, key.key);
                    keys.push(key.key);
                    let ok = match key.wire {
                        Wire::Str | Wire::StrOmit => fl.ty == Str,
                        Wire::Nat => fl.ty == Nat,
                        Wire::Bool | Wire::Flag => fl.ty == Bool,
                        Wire::OptStr | Wire::OptStrOmit => fl.ty == OptStr,
                        Wire::OptNat | Wire::OptNatOmit => fl.ty == OptNat,
                        Wire::Json => fl.ty == Json,
                        Wire::OptJsonOmit => fl.ty == OptJson,
                        Wire::Strs => fl.ty == Strs,
                        Wire::Nats => fl.ty == Nats,
                        Wire::Word => matches!(fl.ty, T(n) if STATE.enum_(n).is_some_and(|e| e.tag_key.is_empty())),
                        Wire::OptWord => matches!(fl.ty, OptT(n) if STATE.enum_(n).is_some_and(|e| e.tag_key.is_empty())),
                        Wire::Obj | Wire::Inline => matches!(fl.ty, T(_)),
                        Wire::OptObjOmit => matches!(fl.ty, OptT(n) if STATE.record(n).is_some()),
                        Wire::Hand(_) => matches!(fl.ty, T(_)),
                        Wire::Recs => matches!(fl.ty, Ts(n) if STATE.record(n).is_some()),
                    };
                    assert!(ok, "{}.{}: {:?} doesn't fit {:?}", r.name, key.field, key.wire, fl.ty);
                }
            }
            seen.push(r.name);
        }
        let mut names: Vec<&str> = STATE.enums.iter().map(|e| e.name).chain(STATE.records.iter().map(|r| r.name)).collect();
        let n = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), n, "a name declared twice");
    }

    /// The record's shape a photo writes it in: its photo shape, else its
    /// journal shape.
    fn photo_of(r: &Record) -> Option<&'static Shape> {
        STATE.record(r.name).and_then(|r| r.shape("photo").or_else(|| r.shape("journal")))
    }

    /// The photo is lossless (issue 36): St's photo shape and every
    /// shape it reaches carry every field, read back exactly (no hand
    /// codec, no inline record, no hand read in a photo shape: each round
    /// law is identity).
    #[test]
    fn the_photo_carries_every_field_exactly() {
        let mut todo = vec!["St"];
        let mut done: Vec<&str> = Vec::new();
        while let Some(name) = todo.pop() {
            if done.contains(&name) {
                continue;
            }
            done.push(name);
            let Some(r) = STATE.record(name) else { continue };
            let sh = photo_of(r).unwrap_or_else(|| panic!("{name}: no photo or journal shape"));
            assert!(sh.read, "{name}.{}: a photo is read back", sh.name);
            for fl in r.fields {
                let key = sh.keys.iter().find(|k| k.field == fl.name);
                let key = key.unwrap_or_else(|| panic!("{name}.{}: {} doesn't carry it", fl.name, sh.name));
                // an omitted default reads back as that default (exact); a
                // hand codec normalises, an inline record is written only
                let exact = !matches!(key.wire, Wire::Inline | Wire::Hand(_));
                assert!(exact, "{name}.{}: {:?} is not exact", fl.name, key.wire);
                if sh.name == "photo" {
                    assert!(key.read.is_none(), "{name}.{}: a hand read in a photo", fl.name);
                }
                if let T(n) | OptT(n) | Ts(n) = fl.ty {
                    todo.push(n);
                }
            }
        }
        assert!(done.contains(&"Msg") && done.contains(&"WatchEnd"), "the walk reaches every record");
    }

    /// A card's `about` (inbox-answered, architect m_55388): a released
    /// card_opened line (none) reads none and writes back byte-identical;
    /// a new one keeps its JSON whole.
    #[test]
    fn a_cards_about_is_absent_when_none_and_whole_when_some() {
        let old = r#"{"id":3,"kind":"question","agent":"docs","text":"which theme?","for_msg":7,"created_ms":1000200,"place":null,"pr":null}"#;
        let c: Card = serde_json::from_str(old).unwrap();
        assert_eq!(c.about, None);
        assert_eq!(serde_json::to_string(&c).unwrap(), old);
        let about = serde_json::json!({"id": "signup-bench", "kind": "page", "title": "signup, before / after", "v": 3, "at": {"kind": "block", "block": "q-bench"}});
        let new = format!(r#"{{"id":4,"kind":"question","agent":"perf","text":"run it?","for_msg":null,"created_ms":1,"place":null,"pr":null,"about":{about}}}"#);
        let c: Card = serde_json::from_str(&new).unwrap();
        assert_eq!(c.about.as_ref(), Some(&about));
        assert_eq!(serde_json::to_value(&c).unwrap()["about"], about);
    }
}
