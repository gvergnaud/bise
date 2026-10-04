# core/image.bend — image markers in message text (docs/images.md).
#
# An image travels through the harness as a one-line text marker:
#   <image name="[Image #1]" path="shot.png" mime="image/png" b64="/…/x.b64">
# (written by the TUI and bend-jsrt, crate rust/images). Here, pure and
# total: the split of a text into text and image parts, the provider
# JSON of those parts (OpenAI-style and Anthropic), and the helpers the
# IO step (runtime/provider.bend) uses to load the images and splice
# their base64 into the printed body. The JSON carries a PLACEHOLDER
# "@@BENDIMG:<b64 path>@@" for the data: megabytes never go through the
# JSON printer. A text without marker is never touched.
import Base
import ./text.bend as Tx
import ../vendor/json.bend as J

type IPart is Data:
  IText{s: String}
  IImg{name: String, path: String, mime: String, b64: String}

def img_open() -> String:
  "<image name=\""

# what an unusable marker becomes (the IO step found no image): text
def img_off() -> String:
  "<image-unavailable name=\""

def has_image(+s: String) -> Bool:
  String.contains(s, img_open())

# ---- one marker: the fields after the opener ----

type IField is Data:
  IF{v: String, rest: String}

def img_field.cut(m: Maybe<&2, Nat>, +s: String, +n: Nat) -> Maybe<&2, IField>:
  match m:
    case Some{+p}:
      Some{IF{String.take(s, p), String.drop(s, Nat.add(p, n))}}
    case None{}:
      None{}

def img_field(+s: String, +sep: String) -> Maybe<&2, IField>:
  img_field.cut(Tx.find(s, sep, 0n), s, String.length(sep))

# a value may not hold a quote, a '>', a line break or a backslash: the
# wire escapes exactly LF, CR and '\', so a marker reads the same in the
# wire text and in the decoded message (2026-10-02: the check was the two
# chars '\' 'n', a marker wrapped by a note kept a real newline in its path)
def img_attr_ok(+v: String) -> Bool:
  Bool.not(Bool.or(String.contains(v, "\""),
    Bool.or(String.contains(v, ">"), Bool.or(String.contains(v, "\n"),
      Bool.or(String.contains(v, "\r"), String.contains(v, "\\"))))))

def img_valid(+name: String, +path: String, +mime: String, +b64: String) -> Bool:
  Bool.and(String.starts_with(mime, "image/"),
    Bool.and(String.ends_with(b64, ".b64"),
      Bool.and(img_attr_ok(name), Bool.and(img_attr_ok(path),
        Bool.and(img_attr_ok(mime), img_attr_ok(b64))))))

type IMk is Data:
  IMk{part: IPart, rest: String}

def img_marker.ok(ok: Bool, +part: IPart, +rest: String) -> Maybe<&2, IMk>:
  match ok:
    case True{}:
      Some{IMk{part, rest}}
    case False{}:
      None{}

def img_marker.d(m: Maybe<&2, IField>, +name: String, +path: String,
    +mime: String) -> Maybe<&2, IMk>:
  match m:
    case Some{+f}:
      match f:
        case IF{+b64, +rest}:
          img_marker.ok(img_valid(name, path, mime, b64),
            IImg{name, path, mime, b64}, rest)
    case None{}:
      None{}

def img_marker.c(m: Maybe<&2, IField>, +name: String, +path: String)
    -> Maybe<&2, IMk>:
  match m:
    case Some{+f}:
      match f:
        case IF{+mime, +rest}:
          img_marker.d(img_field(rest, "\">"), name, path, mime)
    case None{}:
      None{}

def img_marker.b(m: Maybe<&2, IField>, +name: String) -> Maybe<&2, IMk>:
  match m:
    case Some{+f}:
      match f:
        case IF{+path, +rest}:
          img_marker.c(img_field(rest, "\" b64=\""), name, path)
    case None{}:
      None{}

def img_marker.a(m: Maybe<&2, IField>) -> Maybe<&2, IMk>:
  match m:
    case Some{+f}:
      match f:
        case IF{+name, +rest}:
          img_marker.b(img_field(rest, "\" mime=\""), name)
    case None{}:
      None{}

# the marker whose opener was just cut off `s` (s = the text after it)
def img_marker(+s: String) -> Maybe<&2, IMk>:
  img_marker.a(img_field(s, "\" path=\""))

# ---- a text -> its parts ----

# a text part, unless blank (Anthropic refuses blank text blocks)
def push_text.pick(blank: Bool, +s: String, +acc: List<&2, IPart>)
    -> List<&2, IPart>:
  match blank:
    case True{}:
      acc
    case False{}:
      Con{IText{s}, acc}

def push_text(+s: String, +acc: List<&2, IPart>) -> List<&2, IPart>:
  push_text.pick(String.is_empty(String.trim(s)), s, acc)

type ISplit is Data:
  ISGo{s: String}
  ISAt{m: Maybe<&2, Nat>, s: String}
  ISMk{mk: Maybe<&2, IMk>, rest: String}

def img_split.go(fuel: Nat, st: ISplit, +acc: List<&2, IPart>) -> List<&2, IPart>:
  match fuel:
    case 0n:
      List.reverse(&2, IPart, acc)
    case 1n+k:
      match st:
        case ISGo{+s}:
          img_split.go(k, ISAt{Tx.find(s, img_open(), 0n), s}, acc)
        case ISAt{+m, +s}:
          match m:
            case None{}:
              List.reverse(&2, IPart, push_text(s, acc))
            case Some{+p}:
              img_split.go(k,
                ISMk{img_marker(String.drop(s, Nat.add(p, 13n))),
                  String.drop(s, Nat.add(p, 13n))},
                push_text(String.take(s, p), acc))
        case ISMk{+mk, +rest}:
          match mk:
            case Some{+found}:
              match found:
                case IMk{+part, +after}:
                  img_split.go(k, ISGo{after}, Con{part, acc})
            # malformed: the opener stays text, the scan goes on
            case None{}:
              img_split.go(k, ISGo{rest}, push_text(img_open(), acc))

# three steps per marker at most; a marker is 13+ chars
def img_split(+s: String) -> List<&2, IPart>:
  img_split.go(Nat.add(String.length(s), 8n), ISGo{s}, Nil{})

# ---- the b64 path of every well-formed marker, in order ----

def img_paths.go(parts: List<&2, IPart>) -> List<&2, String>:
  match parts:
    case Nil{}:
      Nil{}
    case Con{+p, +rest}:
      match p:
        case IText{_}:
          img_paths.go(rest)
        case IImg{_, _, _, +b64}:
          Con{b64, img_paths.go(rest)}

def img_paths(+s: String) -> List<&2, String>:
  img_paths.go(img_split(s))

# ---- the images kept: loaded, and the `cap` last ones ----

def img_keep.go(flags: List<&2, Bool>, +left: Nat, +acc: List<&2, Bool>)
    -> List<&2, Bool>:
  match flags:
    case Nil{}:
      acc
    case Con{+f, +rest}:
      match f:
        case False{}:
          img_keep.go(rest, left, Con{False{}, acc})
        case True{}:
          match left:
            case 0n:
              img_keep.go(rest, 0n, Con{False{}, acc})
            case 1n+l:
              img_keep.go(rest, l, Con{True{}, acc})

# how many loaded images are sent: all up to `cap`; past it, the oldest
# go in steps of half the cap (the prompt cache, BISE-268): a slide of
# one per new image would change the history at the oldest image on
# every call, so every call past the cap missed the cache from there.
# Stepped, the dropped set stays the same for cap/2 new images: n = 21
# to 30 keep 11 to 20 (the 10 oldest dropped), n = 31 keeps 11 again.
def img_kept.half(+cap: Nat) -> Nat:
  Nat.max(Nat.div(cap, 2n), 1n)

def img_kept.over(over: Bool, +n: Nat, +cap: Nat) -> Nat:
  match over:
    case False{}:
      n
    case True{}:
      Nat.sub(n, Nat.mul(Nat.add(Nat.div(Nat.sub(Nat.sub(n, cap), 1n),
        img_kept.half(cap)), 1n), img_kept.half(cap)))

def img_kept(+n: Nat, +cap: Nat) -> Nat:
  img_kept.over(Nat.is_gt(n, cap), n, cap)

def img_count(+flags: List<&2, Bool>) -> Nat:
  match flags:
    case Nil{}:
      0n
    case Con{+f, +rest}:
      match f:
        case True{}:
          1n+img_count(rest)
        case False{}:
          img_count(rest)

# flags in marker order -> kept flags in marker order (walked from the end)
def img_keep(+loaded: List<&2, Bool>, +cap: Nat) -> List<&2, Bool>:
  img_keep.go(List.reverse(&2, Bool, loaded),
    img_kept(img_count(loaded), cap), Nil{})

# ---- unusable markers become text (the `keep` flags, in order) ----

type IDf is Data:
  IDGo{s: String, keep: List<&2, Bool>}
  IDAt{m: Maybe<&2, Nat>, s: String, keep: List<&2, Bool>}
  IDMk{is_mk: Bool, rest: String, keep: List<&2, Bool>}

def img_is_mk(m: Maybe<&2, IMk>) -> Bool:
  match m:
    case Some{_}:
      True{}
    case None{}:
      False{}

def img_opener_of(+keep: List<&2, Bool>) -> String:
  match keep:
    case Con{+k, _}:
      match k:
        case True{}:
          img_open()
        case False{}:
          img_off()
    case Nil{}:
      img_off()

def img_keep_rest(+keep: List<&2, Bool>) -> List<&2, Bool>:
  match keep:
    case Con{_, +rest}:
      rest
    case Nil{}:
      Nil{}

def img_defang.go(fuel: Nat, st: IDf, +acc: List<&2, String>) -> String:
  match fuel:
    case 0n:
      String.concat(List.reverse(&2, String, acc))
    case 1n+k:
      match st:
        case IDGo{+s, +keep}:
          img_defang.go(k, IDAt{Tx.find(s, img_open(), 0n), s, keep}, acc)
        case IDAt{+m, +s, +keep}:
          match m:
            case None{}:
              String.concat(List.reverse(&2, String, Con{s, acc}))
            case Some{+p}:
              img_defang.go(k,
                IDMk{img_is_mk(img_marker(String.drop(s, Nat.add(p, 13n)))),
                  String.drop(s, Nat.add(p, 13n)), keep},
                Con{String.take(s, p), acc})
        case IDMk{+is_mk, +rest, +keep}:
          match is_mk:
            case True{}:
              img_defang.go(k, IDGo{rest, img_keep_rest(keep)},
                Con{img_opener_of(keep), acc})
            case False{}:
              img_defang.go(k, IDGo{rest, keep}, Con{img_open(), acc})

def img_all_kept(+keep: List<&2, Bool>) -> Bool:
  match keep:
    case Nil{}:
      True{}
    case Con{+k, +rest}:
      Bool.and(k, img_all_kept(rest))

def img_defang.pick(all: Bool, +s: String, +keep: List<&2, Bool>) -> String:
  match all:
    case True{}:
      s
    case False{}:
      img_defang.go(Nat.add(String.length(s), 8n), IDGo{s, keep}, Nil{})

# the text with each marker whose flag is False turned into plain text
def img_defang(+s: String, +keep: List<&2, Bool>) -> String:
  img_defang.pick(img_all_kept(keep), s, keep)

# ---- the JSON of the parts ----

def img_placeholder(+b64: String) -> String:
  String.append("@@BENDIMG:", String.append(b64, "@@"))

# the Codex frame around each image: the model reads its name and path
def img_open_text(+name: String, +path: String) -> String:
  String.append("<image name=", String.append(name,
    String.append(" path=\"", String.append(path, "\">"))))

def text_json(+t: String) -> J.Json:
  J.obj(Con{J.kv("type", J.str("text")), Con{J.kv("text", J.str(t)), Nil{}}})

def oai_image_json(+mime: String, +b64: String) -> J.Json:
  J.obj(Con{J.kv("type", J.str("image_url")),
    Con{J.kv("image_url", J.obj(Con{J.kv("url",
      J.str(String.append("data:", String.append(mime,
        String.append(";base64,", img_placeholder(b64)))))), Nil{}})), Nil{}}})

def anth_image_json(+mime: String, +b64: String) -> J.Json:
  J.obj(Con{J.kv("type", J.str("image")),
    Con{J.kv("source", J.obj(Con{J.kv("type", J.str("base64")),
      Con{J.kv("media_type", J.str(mime)),
      Con{J.kv("data", J.str(img_placeholder(b64))), Nil{}}}})), Nil{}}})

type IStyle is Data:
  IOai{}
  IAnth{}
  IResp{}

# the OpenAI Responses input parts (BISE-147): input_text, and
# input_image with its data URL as a plain string
def resp_image_json(+mime: String, +b64: String) -> J.Json:
  J.obj(Con{J.kv("type", J.str("input_image")),
    Con{J.kv("image_url",
      J.str(String.append("data:", String.append(mime,
        String.append(";base64,", img_placeholder(b64)))))), Nil{}}})

def text_json_st(+st: IStyle, +t: String) -> J.Json:
  match st:
    case IResp{}:
      J.obj(Con{J.kv("type", J.str("input_text")), Con{J.kv("text", J.str(t)), Nil{}}})
    case _:
      text_json(t)

def image_json(+st: IStyle, +mime: String, +b64: String) -> J.Json:
  match st:
    case IOai{}:
      oai_image_json(mime, b64)
    case IAnth{}:
      anth_image_json(mime, b64)
    case IResp{}:
      resp_image_json(mime, b64)

# ---- the part placeholder: an image the IO step checks before sending ----
# The adapters print each image as ONE JSON string in the content array,
# "@@BENDPART:<style>|<mime>|<b64 path>|<name>@@". The IO step
# (runtime/provider.bend, img_body) checks each file and puts in its place
# the image object (its data: the file part) or a text part
# "[image unavailable: <name> (<reason>)]". So no image part leaves without
# its data, whatever produced it. 2026-10-02: a marker whose path held a
# newline passed the per-message parse but not the wire-level scan, so the
# placeholder itself went out as the base64, and Anthropic answered 400 on
# every turn of three agents.

def st_code(+st: IStyle) -> String:
  match st:
    case IOai{}:
      "o"
    case IAnth{}:
      "a"
    case IResp{}:
      "r"

def st_of(+c: String) -> IStyle:
  Bool.pick(IStyle, String.eq(c, "a"), IAnth{},
    Bool.pick(IStyle, String.eq(c, "r"), IResp{}, IOai{}))

def img_part_ph(+st: IStyle, +name: String, +mime: String, +b64: String)
    -> String:
  String.concat(["@@BENDPART:", st_code(st), "|", mime, "|", b64, "|", name,
    "@@"])

def parts_blocks(+st: IStyle, parts: List<&2, IPart>) -> List<&2, J.Json>:
  match parts:
    case Nil{}:
      Nil{}
    case Con{+p, +rest}:
      match p:
        case IText{+s}:
          Con{text_json_st(st, s), parts_blocks(st, rest)}
        case IImg{+name, +path, +mime, +b64}:
          Con{text_json_st(st, img_open_text(name, path)),
            Con{J.str(img_part_ph(st, name, mime, b64)),
              Con{text_json_st(st, "</image>"), parts_blocks(st, rest)}}}

# the content blocks of a text (markers split out)
def content_blocks(+st: IStyle, +s: String) -> List<&2, J.Json>:
  parts_blocks(st, img_split(s))

# ---- the splice: each placeholder <- its loaded base64 ----

type ILoaded is Data:
  IL{path: String, data: String}

def img_lookup(+path: String, +ls: List<&2, ILoaded>) -> String:
  match ls:
    case Nil{}:
      ""
    case Con{+l, +rest}:
      match l:
        case IL{+p, +d}:
          Bool.pick(String, String.eq(p, path), d, img_lookup(path, rest))

type ISp is Data:
  ISpGo{s: String}
  ISpAt{m: Maybe<&2, Nat>, s: String}
  ISpEnd{m: Maybe<&2, Nat>, s: String}

def img_splice.go(fuel: Nat, st: ISp, +ls: List<&2, ILoaded>,
    +acc: List<&2, String>) -> String:
  match fuel:
    case 0n:
      String.concat(List.reverse(&2, String, acc))
    case 1n+k:
      match st:
        case ISpGo{+s}:
          img_splice.go(k, ISpAt{Tx.find(s, "@@BENDIMG:", 0n), s}, ls, acc)
        case ISpAt{+m, +s}:
          match m:
            case None{}:
              String.concat(List.reverse(&2, String, Con{s, acc}))
            case Some{+p}:
              img_splice.go(k,
                ISpEnd{Tx.find(String.drop(s, Nat.add(p, 10n)), "@@", 0n),
                  String.drop(s, Nat.add(p, 10n))},
                ls, Con{String.take(s, p), acc})
        case ISpEnd{+m, +s}:
          match m:
            case None{}:
              String.concat(List.reverse(&2, String, Con{s, acc}))
            case Some{+q}:
              img_splice.go(k, ISpGo{String.drop(s, Nat.add(q, 2n))}, ls,
                Con{img_lookup(String.take(s, q), ls), acc})

def img_splice.pick(none: Bool, +body: String, +ls: List<&2, ILoaded>) -> String:
  match none:
    case True{}:
      body
    case False{}:
      img_splice.go(Nat.add(Nat.mul(List.length(&2, ILoaded, ls), 64n), 64n),
        ISpGo{body}, ls, Nil{})

# the printed body with every placeholder replaced by its image data
def img_splice(+body: String, +ls: List<&2, ILoaded>) -> String:
  img_splice.pick(List.is_empty(&2, ILoaded, ls), body, ls)

# ---- the IO step's pure half: the part placeholders of a printed body ----

type IPh is Data:
  IPh{st: IStyle, mime: String, b64: String, name: String}

# a body cut at its part placeholders, in order
type IBSeg is Data:
  IBText{s: String}
  IBPart{ph: IPh}

# what the IO step found for a placeholder: the file part and its size
# (bytes of base64 sent), or why not
type IRes is Data:
  IROk{part: String, size: Nat}
  IRBad{why: String}

# a placeholder is always an item after the frame's text part: `,"@@BENDPART:`
# (a placeholder written inside a text is escaped `\"`, never matched)
def part_open() -> String:
  ",\"@@BENDPART:"

def ph_of.c(m: Maybe<&2, Nat>, +st: IStyle, +mime: String, +s: String) -> IPh:
  match m:
    case Some{+p}:
      IPh{st, mime, String.take(s, p), String.drop(s, Nat.add(p, 1n))}
    case None{}:
      IPh{st, mime, s, ""}

def ph_of.b(m: Maybe<&2, Nat>, +st: IStyle, +s: String) -> IPh:
  match m:
    case Some{+p}:
      ph_of.c(Tx.find(String.drop(s, Nat.add(p, 1n)), "|", 0n), st,
        String.take(s, p), String.drop(s, Nat.add(p, 1n)))
    case None{}:
      IPh{st, s, "", ""}

# "<style>|<mime>|<b64 path>|<name>" (the name last: it may hold a '|')
def ph_of(+s: String) -> IPh:
  ph_of.b(Tx.find(String.drop(s, 2n), "|", 0n), st_of(String.take(s, 1n)),
    String.drop(s, 2n))

type IBW is Data:
  IBGo{s: String}
  IBAt{m: Maybe<&2, Nat>, s: String}
  IBEnd{m: Maybe<&2, Nat>, s: String}

def body_segs.go(fuel: Nat, st: IBW, +acc: List<&2, IBSeg>) -> List<&2, IBSeg>:
  match fuel:
    case 0n:
      List.reverse(&2, IBSeg, acc)
    case 1n+k:
      match st:
        case IBGo{+s}:
          body_segs.go(k, IBAt{Tx.find(s, part_open(), 0n), s}, acc)
        case IBAt{+m, +s}:
          match m:
            case None{}:
              List.reverse(&2, IBSeg, Con{IBText{s}, acc})
            case Some{+p}:
              body_segs.go(k,
                IBEnd{Tx.find(String.drop(s, Nat.add(p, 13n)), "@@\"", 0n),
                  String.drop(s, Nat.add(p, 13n))},
                Con{IBText{String.take(s, Nat.add(p, 1n))}, acc})
        case IBEnd{+m, +s}:
          match m:
            # no end: not a placeholder after all, the text stays
            case None{}:
              List.reverse(&2, IBSeg,
                Con{IBText{String.append("\"@@BENDPART:", s)}, acc})
            case Some{+q}:
              body_segs.go(k, IBGo{String.drop(s, Nat.add(q, 3n))},
                Con{IBPart{ph_of(String.take(s, q))}, acc})

# three steps per placeholder at most; a placeholder is 13+ chars
def body_segs(+body: String) -> List<&2, IBSeg>:
  body_segs.go(Nat.add(String.length(body), 8n), IBGo{body}, Nil{})

def body_phs(segs: List<&2, IBSeg>) -> List<&2, IPh>:
  match segs:
    case Nil{}:
      Nil{}
    case Con{+g, +rest}:
      match g:
        case IBText{_}:
          body_phs(rest)
        case IBPart{+ph}:
          Con{ph, body_phs(rest)}

def ph_b64(+ph: IPh) -> String:
  match ph:
    case IPh{_, _, +b64, _}:
      b64

# a .b64 of the image store: inside it, a .b64 name, no way up
def img_store_ok(+path: String, +dir: String) -> Bool:
  Bool.and(String.starts_with(path, dir),
    Bool.and(String.ends_with(path, ".b64"),
      Bool.not(String.contains(path, "/../"))))

def b64_char(+c: Char) -> Bool:
  Bool.or(Char.is_digit(c), Bool.or(Char.is_upper(c), Bool.or(Char.is_lower(c),
    Bool.or(U32.is_eq(Char.to_u32(c), 43), Bool.or(U32.is_eq(Char.to_u32(c), 47),
      U32.is_eq(Char.to_u32(c), 61))))))

def b64_all(cs: List<&2, Char>) -> Bool:
  match cs:
    case Nil{}:
      True{}
    case Con{+c, rest}:
      Bool.and(b64_char(c), b64_all(rest))

# why a .b64 of `size` bytes whose first bytes are `head` cannot be sent,
# "" when it can: a cheap check (the data never enters the heap)
def img_why(+size: U32, +head: String) -> String:
  Bool.pick(String, U32.is_eq(size, 0), "empty file",
    Bool.pick(String, Bool.not(U32.is_eq(U32.mod(size, 4), 0)), "invalid base64",
      Bool.pick(String, Bool.or(String.is_empty(head),
        Bool.not(b64_all(String.to_list(head)))), "invalid base64", "")))

def res_count(+rs: List<&2, IRes>) -> Nat:
  match rs:
    case Nil{}:
      0n
    case Con{+r, +rest}:
      match r:
        case IROk{_, _}:
          1n+res_count(rest)
        case IRBad{_}:
          res_count(rest)

# the oldest `d1` good images become text `w1`, the `d2` good ones after
# them `w2`; the others go
def res_drop.go(rs: List<&2, IRes>, +d1: Nat, +d2: Nat, +w1: String,
    +w2: String) -> List<&2, IRes>:
  match rs:
    case Nil{}:
      Nil{}
    case Con{+r, +rest}:
      match r:
        case IROk{+part, +size}:
          match d1:
            case 0n:
              match d2:
                case 0n:
                  Con{IROk{part, size}, res_drop.go(rest, 0n, 0n, w1, w2)}
                case 1n+e:
                  Con{IRBad{w2}, res_drop.go(rest, 0n, e, w1, w2)}
            case 1n+d:
              Con{IRBad{w1}, res_drop.go(rest, d, d2, w1, w2)}
        case IRBad{+w}:
          Con{IRBad{w}, res_drop.go(rest, d1, d2, w1, w2)}

def res_cap.why(+cap: Nat) -> String:
  String.append("older than the ", String.append(Nat.show(cap),
    " most recent images"))

# the `cap` most recent images go (stepped, img_kept); the older ones
# become text
def res_cap(+rs: List<&2, IRes>, +cap: Nat) -> List<&2, IRes>:
  res_drop.go(rs, Nat.sub(res_count(rs), img_kept(res_count(rs), cap)), 0n,
    res_cap.why(cap), "")

# ---- the request's byte limit (2026-10-04, task big-request) ----
# A provider refuses a body over its size (Anthropic through the foundry
# proxy: 400 "Request content length exceeded 32 MB limit"). Screenshots
# are most of it: ambient's 20 most recent were 32.9 MB of base64, so
# every turn got that 400, and an agent stuck that way never took a turn
# again. Now the images that go fit a byte limit: when the body without
# its images (`base`) plus the kept ones would pass `limit`, the count
# cap halves (20, 10, 5, 2, 1, 0), stepped like img_kept so the dropped
# set stays put for a while (the prompt cache); the oldest go first and
# become text that says why.

# the bytes of the `k` most recent good images (rs: newest first)
def res_last_bytes.go(rs: List<&2, IRes>, +k: Nat) -> Nat:
  match rs:
    case Nil{}:
      0n
    case Con{+r, +rest}:
      match r:
        case IROk{_, +size}:
          match k:
            case 0n:
              0n
            case 1n+j:
              Nat.add(size, res_last_bytes.go(rest, j))
        case IRBad{_}:
          res_last_bytes.go(rest, k)

def res_last_bytes(+rs: List<&2, IRes>, +k: Nat) -> Nat:
  res_last_bytes.go(List.reverse(&2, IRes, rs), k)

# the bytes the kept images of count cap `c` weigh
def res_cap_bytes(+rs: List<&2, IRes>, +c: Nat) -> Nat:
  res_last_bytes(rs, img_kept(res_count(rs), c))

def res_fit.pick(fuel: Nat, fits: Bool, +rs: List<&2, IRes>, +c: Nat,
    +base: Nat, +limit: Nat) -> Nat:
  match fuel:
    case 0n:
      Bool.pick(Nat, fits, c, 0n)
    case 1n+k:
      match fits:
        case True{}:
          c
        case False{}:
          res_fit.pick(k,
            Nat.is_le(Nat.add(base, res_cap_bytes(rs, Nat.div(c, 2n))), limit),
            rs, Nat.div(c, 2n), base, limit)

# the count cap that fits: `cap`, else halved until the body fits (0: no
# image at all; a body over the limit without images is the provider's
# to refuse)
def res_fit_cap(+rs: List<&2, IRes>, +cap: Nat, +base: Nat, +limit: Nat) -> Nat:
  res_fit.pick(32n, Nat.is_le(Nat.add(base, res_cap_bytes(rs, cap)), limit),
    rs, cap, base, limit)

# a byte count the user reads: MB from 1 MB (MiB, as the APIs count),
# else KB
def bytes_text(+n: Nat) -> String:
  Bool.pick(String, Nat.is_le(1048576n, n),
    String.append(Nat.show(Nat.div(n, 1048576n)), " MB"),
    String.append(Nat.show(Nat.div(n, 1024n)), " KB"))

def res_fit.why(+limit: Nat) -> String:
  String.append("removed to keep the request under ", bytes_text(limit))

# the images that go: the `cap` most recent, fewer when they would make
# the body pass `limit` bytes (`base`: the body without its images)
def res_fit(+rs: List<&2, IRes>, +cap: Nat, +base: Nat, +limit: Nat)
    -> List<&2, IRes>:
  +n = res_count(rs)
  +by_count = Nat.sub(n, img_kept(n, cap))
  res_drop.go(rs, by_count,
    Nat.sub(Nat.sub(n, img_kept(n, res_fit_cap(rs, cap, base, limit))), by_count),
    res_cap.why(cap), res_fit.why(limit))

# the bytes of the good images that go
def res_bytes(+rs: List<&2, IRes>) -> Nat:
  res_last_bytes(rs, res_count(rs))

def unavailable_text(+name: String, +why: String) -> String:
  String.concat(["[image unavailable: ", name, " (", why, ")]"])

# a placeholder's JSON: the image object with its file part, or a text part
def ph_json(+ph: IPh, r: IRes) -> String:
  match ph:
    case IPh{+st, +mime, +b64, +name}:
      match r:
        case IROk{+part, _}:
          img_splice(J.stringify(image_json(st, mime, b64)),
            Con{IL{b64, part}, Nil{}})
        case IRBad{+why}:
          J.stringify(text_json_st(st, unavailable_text(name, why)))

def res_head(+rs: List<&2, IRes>) -> IRes:
  match rs:
    case Nil{}:
      IRBad{"no image data"}
    case Con{+r, _}:
      r

def res_tail(+rs: List<&2, IRes>) -> List<&2, IRes>:
  match rs:
    case Nil{}:
      Nil{}
    case Con{_, +rest}:
      rest

def body_fill.go(segs: List<&2, IBSeg>, +rs: List<&2, IRes>,
    +acc: List<&2, String>) -> String:
  match segs:
    case Nil{}:
      String.concat(List.reverse(&2, String, acc))
    case Con{+g, +rest}:
      match g:
        case IBText{+s}:
          body_fill.go(rest, rs, Con{s, acc})
        case IBPart{+ph}:
          body_fill.go(rest, res_tail(rs), Con{ph_json(ph, res_head(rs)), acc})

# the body with each placeholder replaced (rs: one per placeholder, in order)
def body_fill(segs: List<&2, IBSeg>, +rs: List<&2, IRes>) -> String:
  body_fill.go(segs, rs, Nil{})

# the chars of a body without its image parts (about its bytes: the JSON
# is ASCII but for the text the user and the tools wrote)
def segs_text_len(segs: List<&2, IBSeg>) -> Nat:
  match segs:
    case Nil{}:
      0n
    case Con{+g, rest}:
      match g:
        case IBText{+s}:
          Nat.add(String.length(s), segs_text_len(rest))
        case IBPart{_}:
          segs_text_len(rest)

# a filled body, what it weighs (about) and what its images weigh
type IFill is Data:
  IFill{body: String, bytes: Nat, img_bytes: Nat}

def fill_fit.go(+segs: List<&2, IBSeg>, +rs: List<&2, IRes>) -> IFill:
  IFill{body_fill(segs, rs), Nat.add(segs_text_len(segs), res_bytes(rs)),
    res_bytes(rs)}

# the body with its images filled: the `cap` most recent, fewer when they
# would make it pass `limit` bytes (res_fit)
def fill_fit(+segs: List<&2, IBSeg>, +rs: List<&2, IRes>, +cap: Nat,
    +limit: Nat) -> IFill:
  fill_fit.go(segs, res_fit(rs, cap, segs_text_len(segs), limit))
