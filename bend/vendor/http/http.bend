# vendor/http/ - hub package 0xbf477e663cf4acb1369a68e0f0fa713b (http,
# dns, url, encoding, zlib, json, wire; no LICENSE file, so MIT-0 under
# the hub terms), vendored so the build never depends on a patched
# ~/.bend/lib: a clean one (CI) fetches the published files, which bend
# 2.0.32 refuses (dns.bend's `def resolve.3`). The changes from the
# published files: dns.bend names resolve.dns3/dns_2/dns_2b/dns_1b and
# binds UDP.bind("0.0.0.0", 0); http.bend listens with
# TCP.listen("127.0.0.1", port); effs/wire.c takes the Bend 2.0.32 C
# types (u64 heap words) and names its effects with CID(name).
# HTTP/1.1 client for http and https, with DNS and TLS.
import Base
import ./about.bend as About
import ./wire.bend as Wire
import ./dns/dns.bend as Dns
import ./url/url.bend as Url
import ./encoding/encoding.bend as Enc
import ./json/json.bend as Json
import ./zlib/zlib.bend as Zlib
import ./bytes/bytes.bend as Bytes

# HTTP/1.1 codec and client (RFC 9112) for http:// and https:// (OpenSSL 3 at run time).
# parse: start-line → field lines → blank → body length (§6.3); chunked (§7.1).
# Bodies are byte strings: one Char per octet, so lengths are exact.
# fetch reads until frame() says the response is whole. It returns Result, not Maybe.
# Build with -o for big bodies: the `bend file.bend` runner overflows on ~30 KB strings; native binaries do not.
#   import ./http.bend as Http

type Req is Data:
  Req{method: String, path: String, headers: Map<&2, List<&2, String>>, body: String}

type Res is Data:
  Res{status: U32, headers: Map<&2, List<&2, String>>, body: String}

# Why fetch failed. Wire errors keep the errno and the text the effect already had.
# ponytail: 60 and 110 are ETIMEDOUT on macOS and Linux. Windows is out of scope.
type Err is Data:
  ErrUrl{}
  ErrDns{}
  ErrConnect{code: U32, why: String}
  ErrTls{code: U32, why: String}
  ErrRead{code: U32, why: String}
  ErrWrite{code: U32, why: String}
  ErrTimeout{}
  ErrRedirect{}
  ErrBad{}

def err.late(+code: U32) -> Bool:
  Bool.or(U32.is_eq(code, 60), U32.is_eq(code, 110))

def err.pick(e: Err, late: Bool) -> Err:
  match late:
    case True{}:
      ErrTimeout{}
    case False{}:
      e

def err.or_late(code: U32, e: Err) -> Err:
  err.pick(e, err.late(code))

type Fields is Data:
  FieldsBad{}
  FieldsOk{m: Map<&2, List<&2, String>>}

# One list per field name, in arrival order. header() is the first value.
# Set-Cookie is never joined. Encode writes one line per value.
def empty() -> Map<&2, List<&2, String>>:
  Map.new(&2, List<&2, String>)

def fields.of(r: Map<&2, List<&2, String>> & List<&2, String>) -> List<&2, String>:
  (h, xs) = r
  xs

def fields(+h: Map<&2, List<&2, String>>, k: String) -> List<&2, String>:
  fields.of(Map.get(List<&2, String>, Nil{}, h, k))

def header.first(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case Con{v, t}:
      v

def header(+h: Map<&2, List<&2, String>>, k: String) -> String:
  header.first(fields(h, k))

def field.last(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case Con{v, Nil{}}:
      v
    case Con{v, t}:
      field.last(t)

# Transfer-Encoding is a list; chunked, when present, is the last coding.
def header.last(+h: Map<&2, List<&2, String>>, k: String) -> String:
  field.last(fields(h, k))

def snoc(xs: List<&2, String>, v: String) -> List<&2, String>:
  match xs:
    case Nil{}:
      [v]
    case Con{h, t}:
      Con{h, snoc(t, v)}

def set(m: Map<&2, List<&2, String>>, k: String, v: String) -> Map<&2, List<&2, String>>:
  Map.set(&2, List<&2, String>, m, k, [v])

def add(+m: Map<&2, List<&2, String>>, +k: String, v: String) -> Map<&2, List<&2, String>>:
  Map.set(&2, List<&2, String>, m, k, snoc(fields(m, k), v))

def sanitize.cons(ch: Char, rest: String, drop: Bool) -> String:
  match drop:
    case True{}:
      rest
    case False{}:
      SCon{ch, rest}

def sanitize(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      sanitize.cons(Chr{c}, sanitize(t), Bool.or(U32.is_eq(c, 13), U32.is_eq(c, 10)))

def drop_cr.if(+s: String, cr: Bool) -> String:
  match cr:
    case True{}:
      String.reverse(String.drop(String.reverse(s), 1n))
    case False{}:
      s

def drop_cr(+s: String) -> String:
  drop_cr.if(s, String.ends_with(s, "\r"))

# Bool.pick evaluates both arms; a split that recursed inside it would reverse acc at every byte.
def take.cut(acc: String, rest: String) -> String & String:
  (String.reverse(String.drop(acc, 1n)), rest)

# hit: the last char pushed onto acc was the stop char.
def take_sp.go(s: String, acc: String, hit: Bool) -> String & String:
  match s:
    case SNil{}:
      match hit:
        case True{}:
          take.cut(acc, SNil{})
        case False{}:
          (String.reverse(acc), SNil{})
    case SCon{Chr{+c}, t}:
      match hit:
        case True{}:
          take.cut(acc, SCon{Chr{c}, t})
        case False{}:
          take_sp.go(t, SCon{Chr{c}, acc}, U32.is_eq(c, 32))

def take_sp(s: String) -> String & String:
  take_sp.go(s, SNil{}, False{})

def start_line.ver(m: String, pv: String & String) -> String & String & String:
  (p, v) = pv
  (m, p, drop_cr(v))

def start_line.rest(mr: String & String) -> String & String & String:
  (m, r) = mr
  start_line.ver(m, take_sp(r))

def start_line(s: String) -> String & String & String:
  start_line.rest(take_sp(s))

def split_at_blank.go(s: String, e: Bool, w: U32, acc: String) -> String & String:
  match s:
    case SNil{}:
      (String.reverse(acc), SNil{})
    case SCon{Chr{+a}, t}:
      match e:
        case True{}:
          (String.reverse(acc), SCon{Chr{a}, t})
        case False{}:
          +w2 = (w * 256 + a : U32)
          split_at_blank.go(t, U32.is_eq(w2, 218762506), w2, SCon{Chr{a}, acc})

def split_at_blank(s: String) -> String & String:
  split_at_blank.go(s, False{}, 0, SNil{})

def split_colon.go(s: String, acc: String, hit: Bool) -> String & String:
  match s:
    case SNil{}:
      match hit:
        case True{}:
          take.cut(acc, SNil{})
        case False{}:
          (String.reverse(acc), SNil{})
    case SCon{Chr{+c}, t}:
      match hit:
        case True{}:
          take.cut(acc, SCon{Chr{c}, t})
        case False{}:
          split_colon.go(t, SCon{Chr{c}, acc}, U32.is_eq(c, 58))

def split_colon(s: String) -> String & String:
  split_colon.go(s, SNil{}, False{})

def has_header.of(r: Map<&2, List<&2, String>> & Bool) -> Bool:
  (h, b) = r
  b

def has_header(+h: Map<&2, List<&2, String>>, k: String) -> Bool:
  has_header.of(Map.has(&2, List<&2, String>, h, k))

def folded(+line: String) -> Bool:
  Bool.or(String.starts_with(line, " "), String.starts_with(line, "\t"))

def tracked(+k: String) -> Bool:
  Bool.or(String.eq(k, "host"), String.eq(k, "content-length"))

def fields_put.dup(m: Map<&2, List<&2, String>>, k: String, v: String, bad: Bool) -> Fields:
  match bad:
    case True{}:
      FieldsBad{}
    case False{}:
      FieldsOk{add(m, k, v)}

def fields_put.key(+m: Map<&2, List<&2, String>>, +k: String, v: String) -> Fields:
  fields_put.dup(m, k, v, Bool.and(tracked(k), has_header(m, k)))

def fields_put.kv(m: Map<&2, List<&2, String>>, kv: String & String) -> Fields:
  (k, v) = kv
  fields_put.key(m, String.to_lower(k), String.trim(v))

def fields_put.fold(m: Map<&2, List<&2, String>>, line: String, fold: Bool) -> Fields:
  match fold:
    case True{}:
      FieldsBad{}
    case False{}:
      fields_put.kv(m, split_colon(line))

def fields_put.empty(m: Map<&2, List<&2, String>>, +line: String, e: Bool) -> Fields:
  match e:
    case True{}:
      FieldsOk{m}
    case False{}:
      fields_put.fold(m, line, folded(line))

def fields_put.ok(m: Map<&2, List<&2, String>>, +line: String) -> Fields:
  fields_put.empty(m, line, String.is_empty(line))

def fields_put(acc: Fields, line: String) -> Fields:
  match acc:
    case FieldsBad{}:
      FieldsBad{}
    case FieldsOk{m}:
      fields_put.ok(m, line)

def parse.headers.done(acc: Fields) -> Maybe<&2, Map<&2, List<&2, String>>>:
  match acc:
    case FieldsBad{}:
      None{}
    case FieldsOk{m}:
      Some{m}

def parse.headers(xs: List<&2, String>, acc: Fields) -> Maybe<&2, Map<&2, List<&2, String>>>:
  match xs:
    case Nil{}:
      parse.headers.done(acc)
    case Con{line, rest}:
      parse.headers(rest, fields_put(acc, drop_cr(line)))

def parse_u32.digit(+c: U32) -> Bool:
  Bool.and(U32.is_le(48, c), U32.is_le(c, 57))

def parse_u32.add(acc: U32, c: U32) -> U32:
  (acc * 10 + (c - 48 : U32) : U32)

def parse_u32.done(acc: U32, bad: Bool) -> Maybe<&2, U32>:
  match bad:
    case True{}:
      None{}
    case False{}:
      Some{acc}

def parse_u32.go(s: String, acc: U32, bad: Bool) -> Maybe<&2, U32>:
  match s:
    case SNil{}:
      parse_u32.done(acc, bad)
    case SCon{Chr{+c}, t}:
      parse_u32.go(t, parse_u32.add(acc, c), Bool.or(bad, Bool.not(parse_u32.digit(c))))

def parse_u32(s: String) -> Maybe<&2, U32>:
  match s:
    case SNil{}:
      None{}
    case SCon{Chr{+c}, t}:
      parse_u32.go(t, parse_u32.add(0, c), Bool.not(parse_u32.digit(c)))

# More: a valid prefix. Bad: cannot become a body. Body: a whole chunked body.
type Hold is Data:
  HoldMore{}
  HoldBad{}
  HoldBody{b: String}
  HoldUntil{b: String}

def is_hex(+c: U32) -> Bool:
  Bool.or(parse_u32.digit(c), Bool.or(Bool.and(U32.is_le(97, c), U32.is_le(c, 102)), Bool.and(U32.is_le(65, c), U32.is_le(c, 70))))

def hexv(+c: U32) -> U32:
  Bool.pick(U32, parse_u32.digit(c), (c - 48 : U32), Bool.pick(U32, U32.is_le(c, 70), (c - 55 : U32), (c - 87 : U32)))

# A chunked body decoder that reads each byte once, so a body fed in pieces
# costs O(bytes) in total. Tr{j}: j bytes of the closing CRLF CRLF matched.
type Dc is Data:
  DcSize{acc: U32, seen: Bool}
  DcExt{n: U32}
  DcSizeLf{n: U32}
  DcData{k: U32}
  DcDataCr{}
  DcDataLf{}
  DcTr{j: U32}
  DcDone{}
  DcBad{}

# st: where the decoder is. racc: the body so far, reversed. rest: bytes after a Done body.
type Dco is Data:
  Dco{st: Dc, racc: String, rest: String}

def dc.start() -> Dc:
  DcSize{0, False{}}

def dc.size.semi(+n: U32, semi: Bool) -> Dc:
  match semi:
    case True{}:
      DcExt{n}
    case False{}:
      DcBad{}

def dc.size.cr(+n: U32, +c: U32, cr: Bool) -> Dc:
  match cr:
    case True{}:
      DcSizeLf{n}
    case False{}:
      dc.size.semi(n, U32.is_eq(c, 59))

def dc.size.end(+n: U32, +c: U32, seen: Bool) -> Dc:
  match seen:
    case False{}:
      DcBad{}
    case True{}:
      dc.size.cr(n, c, U32.is_eq(c, 13))

# A size that would overflow 32 bits ends the digits, and size.end then refuses it.
def dc.size(+acc: U32, seen: Bool, +c: U32, digit: Bool) -> Dc:
  match digit:
    case True{}:
      DcSize{(acc * 16 + hexv(c) : U32), True{}}
    case False{}:
      dc.size.end(acc, c, seen)

# RFC 9112 §7.1.1: recipients ignore unknown chunk extensions; a bare LF is not CRLF.
def dc.ext.lf(+n: U32, lf: Bool) -> Dc:
  match lf:
    case True{}:
      DcBad{}
    case False{}:
      DcExt{n}

def dc.ext(+n: U32, +c: U32, cr: Bool) -> Dc:
  match cr:
    case True{}:
      DcSizeLf{n}
    case False{}:
      dc.ext.lf(n, U32.is_eq(c, 10))

def dc.sized(+n: U32, zero: Bool) -> Dc:
  match zero:
    case True{}:
      DcTr{2}
    case False{}:
      DcData{n}

def dc.size.lf(+n: U32, lf: Bool) -> Dc:
  match lf:
    case True{}:
      dc.sized(n, U32.is_eq(n, 0))
    case False{}:
      DcBad{}

# k data bytes are left and m of them were just read.
def dc.data(+k: U32, +m: U32) -> Dc:
  Bool.pick(Dc, U32.is_le(k, m), DcDataCr{}, DcData{(k - m : U32)})

def dc.pick(ok: Bool, next: Dc) -> Dc:
  match ok:
    case True{}:
      next
    case False{}:
      DcBad{}

# ponytail: trailer fields are dropped unparsed (§7.1.2 allows discarding)
def dc.tr.hit(+j: U32) -> Dc:
  Bool.pick(Dc, U32.is_eq(j, 3), DcDone{}, DcTr{(j + 1 : U32)})

def dc.tr(+j: U32, +c: U32, hit: Bool) -> Dc:
  match hit:
    case True{}:
      dc.tr.hit(j)
    case False{}:
      Bool.pick(Dc, U32.is_eq(c, 13), DcTr{1}, DcTr{0})

def dc.tr.want(+j: U32) -> U32:
  Bool.pick(U32, U32.is_eq(U32.mod(j, 2), 0), 13, 10)

def dc.byte(h: Char) -> U32:
  Chr{c} = h
  c

# One byte c. Done and Bad never look at it, and a data byte is not inspected.
def dc.step(st: Dc, +c: U32) -> Dc:
  match st:
    case DcBad{}:
      DcBad{}
    case DcSize{+acc, seen}:
      dc.size(acc, seen, c, Bool.and(is_hex(c), U32.is_le(acc, 268435455)))
    case DcExt{+n}:
      dc.ext(n, c, U32.is_eq(c, 13))
    case DcSizeLf{+n}:
      dc.size.lf(n, U32.is_eq(c, 10))
    case DcDataCr{}:
      dc.pick(U32.is_eq(c, 13), DcDataLf{})
    case DcDataLf{}:
      dc.pick(U32.is_eq(c, 10), dc.start())
    case DcTr{+j}:
      dc.tr(j, c, U32.is_eq(c, dc.tr.want(j)))
    case DcData{+k}:
      dc.data(k, 1)
    case DcDone{}:
      DcDone{}

def dc.live(st: Dc) -> Bool:
  match st:
    case DcDone{}:
      False{}
    case DcBad{}:
      False{}
    case DcSize{a, b}:
      True{}
    case DcExt{n}:
      True{}
    case DcSizeLf{n}:
      True{}
    case DcData{k}:
      True{}
    case DcDataCr{}:
      True{}
    case DcDataLf{}:
      True{}
    case DcTr{j}:
      True{}

def dc.feed(s: String, st: Dc, racc: String) -> Dco:
  match s:
    case SNil{}:
      Dco{st, racc, ""}
    case SCon{h, t}:
      match st:
        case DcBad{}:
          Dco{DcBad{}, racc, ""}
        case DcDone{}:
          Dco{DcDone{}, racc, SCon{h, t}}
        case DcData{+k}:
          dc.feed(t, dc.data(k, 1), SCon{h, racc})
        case DcSize{+acc, seen}:
          dc.feed(t, dc.step(DcSize{acc, seen}, dc.byte(h)), racc)
        case DcExt{+n}:
          dc.feed(t, dc.step(DcExt{n}, dc.byte(h)), racc)
        case DcSizeLf{+n}:
          dc.feed(t, dc.step(DcSizeLf{n}, dc.byte(h)), racc)
        case DcDataCr{}:
          dc.feed(t, dc.step(DcDataCr{}, dc.byte(h)), racc)
        case DcDataLf{}:
          dc.feed(t, dc.step(DcDataLf{}, dc.byte(h)), racc)
        case DcTr{+j}:
          dc.feed(t, dc.step(DcTr{j}, dc.byte(h)), racc)

def chunk.hold(o: Dco) -> Hold:
  Dco{st, racc, rest} = o
  match st:
    case DcDone{}:
      HoldBody{String.reverse(racc)}
    case DcBad{}:
      HoldBad{}
    case DcSize{a, b}:
      HoldMore{}
    case DcExt{n}:
      HoldMore{}
    case DcSizeLf{n}:
      HoldMore{}
    case DcData{k}:
      HoldMore{}
    case DcDataCr{}:
      HoldMore{}
    case DcDataLf{}:
      HoldMore{}
    case DcTr{j}:
      HoldMore{}

def chunk.live(s: String) -> Hold:
  chunk.hold(dc.feed(s, dc.start(), ""))

def chunk.rest(o: Dco) -> Maybe<&2, String>:
  Dco{st, racc, rest} = o
  match st:
    case DcDone{}:
      Some{rest}
    case DcBad{}:
      None{}
    case DcSize{a, b}:
      None{}
    case DcExt{n}:
      None{}
    case DcSizeLf{n}:
      None{}
    case DcData{k}:
      None{}
    case DcDataCr{}:
      None{}
    case DcDataLf{}:
      None{}
    case DcTr{j}:
      None{}

def chunk.suffix(s: String) -> Maybe<&2, String>:
  chunk.rest(dc.feed(s, dc.start(), ""))

def chunk.decode.hold(h: Hold) -> Maybe<&2, String>:
  match h:
    case HoldMore{}:
      None{}
    case HoldBad{}:
      None{}
    case HoldUntil{b}:
      None{}
    case HoldBody{b}:
      Some{b}

def chunk.decode(s: String) -> Maybe<&2, String>:
  chunk.decode.hold(chunk.live(s))

def bytes.snd(r: Bytes.Bytes & Bytes.Bytes) -> Bytes.Bytes:
  (a, b) = r
  b

def found.at(+from: U32, m: Maybe<&2, U32>) -> Maybe<&2, U32>:
  match m:
    case None{}:
      None{}
    case Some{+j}:
      Some{(from + j : U32)}

# A chunked body decoded from piece[i..]: a data run is sliced whole, and every other byte steps dc.
type Cs is Type:
  Cs{piece: Bytes.Bytes, i: U32, st: Dc, parts: List<&1, Bytes.Bytes>}

# st: where the decoder stopped. parts: the data so far, newest first. rest: the bytes after a Done body.
type Ck is Type:
  Ck{st: Dc, parts: List<&1, Bytes.Bytes>, rest: Bytes.Bytes}

def ck.next(+len: U32, +i: U32, +st: Dc, piece: Bytes.Bytes, parts: List<&1, Bytes.Bytes>) -> Bool & Cs:
  (Bool.and(U32.is_lt(i, len), dc.live(st)), Cs{piece, i, st, parts})

def ck.data(+len: U32, +i: U32, +k: U32, +m: U32, parts: List<&1, Bytes.Bytes>, r: Bytes.Bytes & Bytes.Bytes) -> Bool & Cs:
  (piece, run) = r
  ck.next(len, (i + m : U32), dc.data(k, m), piece, Con{run, parts})

def ck.byte(+len: U32, +i: U32, st: Dc, parts: List<&1, Bytes.Bytes>, r: Bytes.Bytes & Maybe<&2, U32>) -> Bool & Cs:
  (piece, m) = r
  match m:
    case None{}:
      ck.next(len, i, DcBad{}, piece, parts)
    case Some{+c}:
      ck.next(len, (i + 1 : U32), dc.step(st, c), piece, parts)

def ck.step(+len: U32, cs: Cs) -> Bool & Cs:
  Cs{piece, +i, st, parts} = cs
  match st:
    case DcData{+k}:
      +m = U32.min(k, (len - i : U32))
      ck.data(len, i, k, m, parts, Bytes.slice(piece, i, m))
    case DcDone{}:
      (False{}, Cs{piece, i, DcDone{}, parts})
    case DcBad{}:
      (False{}, Cs{piece, i, DcBad{}, parts})
    case DcSize{+acc, seen}:
      ck.byte(len, i, DcSize{acc, seen}, parts, Bytes.get(piece, i))
    case DcExt{+n}:
      ck.byte(len, i, DcExt{n}, parts, Bytes.get(piece, i))
    case DcSizeLf{+n}:
      ck.byte(len, i, DcSizeLf{n}, parts, Bytes.get(piece, i))
    case DcDataCr{}:
      ck.byte(len, i, DcDataCr{}, parts, Bytes.get(piece, i))
    case DcDataLf{}:
      ck.byte(len, i, DcDataLf{}, parts, Bytes.get(piece, i))
    case DcTr{+j}:
      ck.byte(len, i, DcTr{j}, parts, Bytes.get(piece, i))

def ck.rest(st: Dc, parts: List<&1, Bytes.Bytes>, r: Bytes.Bytes & Bytes.Bytes) -> Ck:
  (piece, rest) = r
  Ck{st, parts, rest}

def ck.end(+len: U32, cs: Cs) -> Ck:
  Cs{piece, +i, st, parts} = cs
  ck.rest(st, parts, Bytes.slice(piece, i, (len - i : U32)))

# Each step reads at least one byte, so len steps are enough.
def ck.feed(f: Nat, +len: U32, r: Bool & Cs) -> Ck:
  match f:
    case 0n:
      (go, cs) = r
      ck.end(len, cs)
    case 1n+p:
      (go, cs) = r
      match go:
        case False{}:
          ck.end(len, cs)
        case True{}:
          ck.feed(p, len, ck.step(len, cs))

def ck(piece: Bytes.Bytes, +st: Dc, parts: List<&1, Bytes.Bytes>) -> Ck:
  Bytes.Bytes{+len, buf} = piece
  ck.feed(U32.to_nat(len), len, ck.next(len, 0, st, Bytes.Bytes{len, buf}, parts))

def parse.res.cl.have(+rest: String, +k: Nat, short: Bool) -> Hold:
  match short:
    case True{}:
      HoldMore{}
    case False{}:
      HoldBody{String.take(rest, k)}

def parse.res.cl.k(+rest: String, +k: Nat) -> Hold:
  parse.res.cl.have(rest, k, Nat.is_lt(String.length(rest), k))

def parse.res.cl.n(rest: String, n: Maybe<&2, U32>) -> Hold:
  match n:
    case None{}:
      HoldBad{}
    case Some{k}:
      parse.res.cl.k(rest, U32.to_nat(k))

def parse.body.cl(+h: Map<&2, List<&2, String>>, rest: String, hascl: Bool) -> Hold:
  match hascl:
    case False{}:
      HoldBody{""}
    case True{}:
      parse.res.cl.n(rest, parse_u32(header(h, "content-length")))

# RFC 9112 §6.3 (4): a request coding other than chunked cannot be framed.
def parse.body.chunked.eq(rest: String, ok: Bool) -> Hold:
  match ok:
    case False{}:
      HoldBad{}
    case True{}:
      chunk.live(rest)

def parse.body.chunked.val(te: String, rest: String) -> Hold:
  parse.body.chunked.eq(rest, String.eq(String.to_lower(te), "chunked"))

def parse.body.chunked(+h: Map<&2, List<&2, String>>, rest: String, hascl: Bool) -> Hold:
  match hascl:
    case True{}:
      HoldBad{}
    case False{}:
      parse.body.chunked.val(header.last(h, "transfer-encoding"), rest)

def parse.body.te(+h: Map<&2, List<&2, String>>, rest: String, te: Bool) -> Hold:
  match te:
    case True{}:
      parse.body.chunked(h, rest, has_header(h, "content-length"))
    case False{}:
      parse.body.cl(h, rest, has_header(h, "content-length"))

def parse.body(+h: Map<&2, List<&2, String>>, rest: String) -> Hold:
  parse.body.te(h, rest, has_header(h, "transfer-encoding"))

# Bad: never a request. More: the head is not whole; read on. Head: the head is whole
# and valid, and the body is still coming (req.body is empty). Req: a whole request.
type Got is Data:
  GotBad{}
  GotMore{}
  GotHead{req: Req}
  GotReq{req: Req}

def parse.finish(method: String, path: String, h: Map<&2, List<&2, String>>, body: Hold) -> Got:
  match body:
    case HoldBad{}:
      GotBad{}
    case HoldMore{}:
      GotHead{Req{method, path, h, ""}}
    case HoldUntil{b}:
      GotBad{}
    case HoldBody{b}:
      GotReq{Req{method, path, h, b}}

def parse.host(method: String, path: String, +h: Map<&2, List<&2, String>>, rest: String, has: Bool) -> Got:
  match has:
    case False{}:
      GotBad{}
    case True{}:
      parse.finish(method, path, h, parse.body(h, rest))

# RFC 9112 §3.2: only HTTP/1.1 requires Host.
def parse.fields(method: String, path: String, rest: String, v11: Bool, fs: Maybe<&2, Map<&2, List<&2, String>>>) -> Got:
  match fs:
    case None{}:
      GotBad{}
    case Some{+h}:
      parse.host(method, path, h, rest, Bool.or(Bool.not(v11), has_header(h, "host")))

def target_ok(+p: String) -> Bool:
  Bool.or(String.starts_with(p, "/"), Bool.or(String.eq(p, "*"), String.starts_with(p, "http://")))

def parse.target(method: String, +path: String, headers: List<&2, String>, rest: String, v11: Bool, ok: Bool) -> Got:
  match ok:
    case False{}:
      GotBad{}
    case True{}:
      parse.fields(method, path, rest, v11, parse.headers(headers, FieldsOk{empty()}))

def parse.ver(method: String, +path: String, +v: String, headers: List<&2, String>, rest: String, ok: Bool) -> Got:
  match ok:
    case False{}:
      GotBad{}
    case True{}:
      parse.target(method, path, headers, rest, String.eq(v, "HTTP/1.1"), target_ok(path))

def parse.empty_method(method: String, path: String, +v: String, headers: List<&2, String>, rest: String, empty: Bool) -> Got:
  match empty:
    case True{}:
      GotBad{}
    case False{}:
      parse.ver(method, path, v, headers, rest, Bool.or(String.eq(v, "HTTP/1.1"), String.eq(v, "HTTP/1.0")))

def parse.start3(mpv: String & String & String, headers: List<&2, String>, rest: String) -> Got:
  (+method, path, v) = mpv
  parse.empty_method(method, path, v, headers, rest, String.is_empty(method))

def parse.lines(xs: List<&2, String>, rest: String) -> Got:
  match xs:
    case Nil{}:
      GotBad{}
    case Con{line, headers}:
      parse.start3(start_line(line), headers, rest)

def parse.head(+head: String, rest: String, whole: Bool) -> Got:
  match whole:
    case False{}:
      GotMore{}
    case True{}:
      parse.lines(String.lines(head), rest)

def parse.of(hb: String & String) -> Got:
  (+head, rest) = hb
  parse.head(head, rest, String.ends_with(head, "\r\n\r\n"))

# serve reads until this is not GotMore.
def parse.got(raw: String) -> Got:
  parse.of(split_at_blank(raw))

def parse.req(g: Got) -> Maybe<&2, Req>:
  match g:
    case GotReq{req}:
      Some{req}
    case GotBad{}:
      None{}
    case GotMore{}:
      None{}
    case GotHead{req}:
      None{}

def parse(raw: String) -> Maybe<&2, Req>:
  parse.req(parse.got(raw))

def parse.res.cl(+h: Map<&2, List<&2, String>>, rest: String, hascl: Bool) -> Hold:
  match hascl:
    case False{}:
      HoldBody{rest}
    case True{}:
      parse.res.cl.n(rest, parse_u32(header(h, "content-length")))

def parse.res.chunked.eq(rest: String, ok: Bool) -> Hold:
  match ok:
    case False{}:
      HoldUntil{rest}
    case True{}:
      chunk.live(rest)

def parse.res.chunked.val(te: String, rest: String) -> Hold:
  parse.res.chunked.eq(rest, String.eq(String.to_lower(te), "chunked"))

def parse.res.chunked(+h: Map<&2, List<&2, String>>, rest: String, hascl: Bool) -> Hold:
  match hascl:
    case True{}:
      HoldBad{}
    case False{}:
      parse.res.chunked.val(header.last(h, "transfer-encoding"), rest)

def parse.res.te(+h: Map<&2, List<&2, String>>, rest: String, te: Bool) -> Hold:
  match te:
    case True{}:
      parse.res.chunked(h, rest, has_header(h, "content-length"))
    case False{}:
      parse.res.cl(h, rest, has_header(h, "content-length"))

def parse.res.body(+h: Map<&2, List<&2, String>>, rest: String) -> Hold:
  parse.res.te(h, rest, has_header(h, "transfer-encoding"))

# Bad: never a message. More: a valid prefix; read on. Done: a whole response.
type Frame is Data:
  FrameBad{}
  FrameMore{}
  FrameDone{res: Res}

# RFC 9112 §8: a message cut short by close is incomplete, not valid.
def frame.cut(closed: Bool) -> Frame:
  match closed:
    case True{}:
      FrameBad{}
    case False{}:
      FrameMore{}

def frame.wait(res: Res, wait: Bool) -> Frame:
  match wait:
    case True{}:
      FrameMore{}
    case False{}:
      FrameDone{res}

def frame.done(status: U32, h: Map<&2, List<&2, String>>, body: Hold, +closed: Bool, open: Bool) -> Frame:
  match body:
    case HoldBad{}:
      FrameBad{}
    case HoldMore{}:
      frame.cut(closed)
    case HoldUntil{b}:
      frame.wait(Res{status, h, b}, Bool.not(closed))
    case HoldBody{b}:
      frame.wait(Res{status, h, b}, Bool.and(open, Bool.not(closed)))

# RFC 9112 §6.3 (8): no TE and no CL ⇒ the body runs until the server closes.
def frame.open(+h: Map<&2, List<&2, String>>) -> Bool:
  Bool.not(Bool.or(has_header(h, "transfer-encoding"), has_header(h, "content-length")))

# RFC 9112 §6.3 (1): HEAD, 1xx, 204 and 304 responses end at the blank line.
def frame.nobody(+status: U32, head: Bool) -> Bool:
  Bool.or(head, Bool.or(U32.is_lt(status, 200), Bool.or(U32.is_eq(status, 204), U32.is_eq(status, 304))))

# 101 is the final response. Other 1xx are interim; a final response follows.
def frame.interim(+status: U32) -> Bool:
  Bool.and(U32.is_lt(status, 200), Bool.not(U32.is_eq(status, 101)))

def frame.status(status: U32, +h: Map<&2, List<&2, String>>, rest: String, closed: Bool, nobody: Bool) -> Frame:
  match nobody:
    case True{}:
      FrameDone{Res{status, h, ""}}
    case False{}:
      frame.done(status, h, parse.res.body(h, rest), closed, frame.open(h))

def parse.res.fields(+status: U32, rest: String, fs: Maybe<&2, Map<&2, List<&2, String>>>, closed: Bool, head: Bool) -> Frame:
  match fs:
    case None{}:
      FrameBad{}
    case Some{h}:
      frame.status(status, h, rest, closed, frame.nobody(status, head))

def parse.res.num(code: Maybe<&2, U32>, headers: List<&2, String>, rest: String, closed: Bool, head: Bool) -> Frame:
  match code:
    case None{}:
      FrameBad{}
    case Some{n}:
      parse.res.fields(n, rest, parse.headers(headers, FieldsOk{empty()}), closed, head)

def parse.res.ver(code: String, headers: List<&2, String>, rest: String, closed: Bool, head: Bool, ok: Bool) -> Frame:
  match ok:
    case False{}:
      FrameBad{}
    case True{}:
      parse.res.num(parse_u32(code), headers, rest, closed, head)

def parse.res.start3(mpv: String & String & String, headers: List<&2, String>, rest: String, closed: Bool, head: Bool) -> Frame:
  (+ver, code, reason) = mpv
  parse.res.ver(code, headers, rest, closed, head, Bool.or(String.eq(ver, "HTTP/1.1"), String.eq(ver, "HTTP/1.0")))

def parse.res.lines(xs: List<&2, String>, rest: String, closed: Bool, head: Bool) -> Frame:
  match xs:
    case Nil{}:
      FrameBad{}
    case Con{line, headers}:
      parse.res.start3(start_line(line), headers, rest, closed, head)

def parse.res.blank(hd: String, rest: String, +closed: Bool, head: Bool, ok: Bool) -> Frame:
  match ok:
    case False{}:
      frame.cut(closed)
    case True{}:
      parse.res.lines(String.lines(hd), rest, closed, head)

def parse.res.of(hb: String & String, closed: Bool, head: Bool) -> Frame:
  (+hd, rest) = hb
  parse.res.blank(hd, rest, closed, head, String.ends_with(hd, "\r\n\r\n"))

type Lead is Data:
  LeadCut{}
  LeadFinal{}
  LeadSkip{rest: String}

def frame.lead.if(rest: String, interim: Bool) -> Lead:
  match interim:
    case True{}:
      LeadSkip{rest}
    case False{}:
      LeadFinal{}

def frame.lead.num(n: Maybe<&2, U32>, rest: String) -> Lead:
  match n:
    case None{}:
      LeadFinal{}
    case Some{s}:
      frame.lead.if(rest, frame.interim(s))

def frame.lead.code(mpv: String & String & String, rest: String) -> Lead:
  (ver, code, reason) = mpv
  frame.lead.num(parse_u32(code), rest)

def frame.lead.lines(xs: List<&2, String>, rest: String) -> Lead:
  match xs:
    case Nil{}:
      LeadFinal{}
    case Con{line, headers}:
      frame.lead.code(start_line(line), rest)

def frame.lead.blank(+hd: String, rest: String, ok: Bool) -> Lead:
  match ok:
    case False{}:
      LeadCut{}
    case True{}:
      frame.lead.lines(String.lines(hd), rest)

def frame.lead(hb: String & String) -> Lead:
  (+hd, rest) = hb
  frame.lead.blank(hd, rest, String.ends_with(hd, "\r\n\r\n"))

def frame.at.zero.go(+raw: String, closed: Bool, head: Bool, lead: Lead) -> Frame:
  match lead:
    case LeadCut{}:
      frame.cut(closed)
    case LeadFinal{}:
      parse.res.of(split_at_blank(raw), closed, head)
    case LeadSkip{rest}:
      FrameBad{}

def frame.at.zero(+raw: String, closed: Bool, head: Bool) -> Frame:
  frame.at.zero.go(raw, closed, head, frame.lead(split_at_blank(raw)))

def frame.at.cont(+raw: String, lead: Lead) -> String:
  match lead:
    case LeadCut{}:
      raw
    case LeadFinal{}:
      raw
    case LeadSkip{rest}:
      rest

# ponytail: 8 interim responses; raise the fuel if a server sends more
def frame.at(fuel: Nat, +raw: String, closed: Bool, head: Bool) -> Frame:
  match fuel:
    case 0n:
      frame.at.zero(raw, closed, head)
    case 1n+f:
      frame.at(f, frame.at.cont(raw, frame.lead(split_at_blank(raw))), closed, head)

# frame(bytes so far, server closed?, request was HEAD?)
def frame(raw: String, closed: Bool, head: Bool) -> Frame:
  frame.at(8n, raw, closed, head)

def frame.res(f: Frame) -> Maybe<&2, Res>:
  match f:
    case FrameDone{res}:
      Some{res}
    case FrameBad{}:
      None{}
    case FrameMore{}:
      None{}

def parse_res(raw: String) -> Maybe<&2, Res>:
  frame.res(frame(raw, True{}, False{}))

def res_fields.res(res: Res, k: String) -> List<&2, String>:
  Res{status, headers, body} = res
  fields(headers, k)

def res_fields(m: Maybe<&2, Res>, k: String) -> List<&2, String>:
  match m:
    case None{}:
      Nil{}
    case Some{res}:
      res_fields.res(res, k)

# The response body as it arrives. Len: bytes left. Chunk: the decoder. Close: until the
# server closes. Done: the body is whole. Bad: it cannot become a body.
type Rb is Data:
  RbLen{left: U32}
  RbChunk{st: Dc}
  RbClose{}
  RbDone{}
  RbBad{}

def rb.cl(n: Maybe<&2, U32>) -> Maybe<&2, Rb>:
  match n:
    case None{}:
      None{}
    case Some{+k}:
      Some{Bool.pick(Rb, U32.is_zero(k), RbDone{}, RbLen{k})}

def rb.te(+h: Map<&2, List<&2, String>>, cl: Bool) -> Maybe<&2, Rb>:
  match cl:
    case True{}:
      None{}
    case False{}:
      Some{Bool.pick(Rb, String.eq(String.to_lower(header.last(h, "transfer-encoding")), "chunked"), RbChunk{dc.start()}, RbClose{})}

def rb.len(+h: Map<&2, List<&2, String>>, te: Bool) -> Maybe<&2, Rb>:
  match te:
    case True{}:
      rb.te(h, has_header(h, "content-length"))
    case False{}:
      Bool.pick(Maybe<&2, Rb>, has_header(h, "content-length"), rb.cl(parse_u32(header(h, "content-length"))), Some{RbClose{}})

# RFC 9112 §6.3: how the body of this response ends. None: it cannot be framed.
def rb.of(+h: Map<&2, List<&2, String>>, nobody: Bool) -> Maybe<&2, Rb>:
  match nobody:
    case True{}:
      Some{RbDone{}}
    case False{}:
      rb.len(h, has_header(h, "transfer-encoding"))

# rb: how the body goes on. parts: body bytes so far, newest first. rest: bytes past a Done body.
type Bf is Type:
  Bf{rb: Rb, parts: List<&1, Bytes.Bytes>, rest: Bytes.Bytes}

def rb.len.cut(parts: List<&1, Bytes.Bytes>, +len: U32, +left: U32, r: Bytes.Bytes & Bytes.Bytes) -> Bf:
  (piece, data) = r
  Bf{RbDone{}, Con{data, parts}, bytes.snd(Bytes.slice(piece, left, (len - left : U32)))}

def rb.len.have(short: Bool, +left: U32, parts: List<&1, Bytes.Bytes>, +len: U32, piece: Bytes.Bytes) -> Bf:
  match short:
    case True{}:
      Bf{RbLen{(left - len : U32)}, Con{piece, parts}, Bytes.new(0)}
    case False{}:
      rb.len.cut(parts, len, left, Bytes.slice(piece, 0, left))

def rb.chunk(c: Ck) -> Bf:
  Ck{st, parts, rest} = c
  match st:
    case DcDone{}:
      Bf{RbDone{}, parts, rest}
    case DcBad{}:
      Bf{RbBad{}, parts, rest}
    case DcSize{a, b}:
      Bf{RbChunk{DcSize{a, b}}, parts, rest}
    case DcExt{x}:
      Bf{RbChunk{DcExt{x}}, parts, rest}
    case DcSizeLf{x}:
      Bf{RbChunk{DcSizeLf{x}}, parts, rest}
    case DcData{k}:
      Bf{RbChunk{DcData{k}}, parts, rest}
    case DcDataCr{}:
      Bf{RbChunk{DcDataCr{}}, parts, rest}
    case DcDataLf{}:
      Bf{RbChunk{DcDataLf{}}, parts, rest}
    case DcTr{j}:
      Bf{RbChunk{DcTr{j}}, parts, rest}

# The body bytes in piece, added to parts.
def rb.feed(rb: Rb, parts: List<&1, Bytes.Bytes>, piece: Bytes.Bytes) -> Bf:
  match rb:
    case RbLen{+left}:
      Bytes.Bytes{+len, buf} = piece
      rb.len.have(U32.is_lt(len, left), left, parts, len, Bytes.Bytes{len, buf})
    case RbChunk{st}:
      rb.chunk(ck(piece, st, parts))
    case RbClose{}:
      Bf{RbClose{}, Con{piece, parts}, Bytes.new(0)}
    case RbDone{}:
      Bf{RbDone{}, parts, piece}
    case RbBad{}:
      Bf{RbBad{}, parts, piece}

# RFC 9112 §8: only a close-delimited body may end at close.
def rb.eof(rb: Rb) -> Bool:
  match rb:
    case RbClose{}:
      True{}
    case RbDone{}:
      True{}
    case RbLen{n}:
      False{}
    case RbChunk{st}:
      False{}
    case RbBad{}:
      False{}

def rb.body(parts: List<&1, Bytes.Bytes>) -> String:
  Bytes.to_string(Bytes.concat(List.reverse(&1, Bytes.Bytes, parts)))

# The head scan. Seek: look for the blank line from byte from; k interim heads are behind.
# Wait: no blank line yet. Res: the final head, how its body ends, and the bytes after it.
type Hs is Type:
  HsSeek{buf: Bytes.Bytes, from: U32, k: U32}
  HsWait{buf: Bytes.Bytes, k: U32}
  HsBad{}
  HsRes{res: Res, rb: Rb, rest: Bytes.Bytes}

def hs.final(+status: U32, +h: Map<&2, List<&2, String>>, rest: Bytes.Bytes, m: Maybe<&2, Rb>) -> Hs:
  match m:
    case None{}:
      HsBad{}
    case Some{rb}:
      HsRes{Res{status, h, ""}, rb, rest}

def hs.skip(more: Bool, +k: U32, rest: Bytes.Bytes) -> Hs:
  match more:
    case True{}:
      HsSeek{rest, 0, (k + 1 : U32)}
    case False{}:
      HsBad{}

def hs.pick(interim: Bool, +head: Bool, +k: U32, +status: U32, +h: Map<&2, List<&2, String>>, rest: Bytes.Bytes) -> Hs:
  match interim:
    case True{}:
      hs.skip(U32.is_lt(k, 8), k, rest)
    case False{}:
      hs.final(status, h, rest, rb.of(h, frame.nobody(status, head)))

def hs.res(+head: Bool, +k: U32, rest: Bytes.Bytes, res: Res) -> Hs:
  Res{+status, +h, body} = res
  hs.pick(frame.interim(status), head, k, status, h, rest)

# The head alone, framed as a HEAD response so it stops at the blank line.
def hs.parsed(+head: Bool, +k: U32, rest: Bytes.Bytes, f: Frame) -> Hs:
  match f:
    case FrameDone{res}:
      hs.res(head, k, rest, res)
    case FrameBad{}:
      HsBad{}
    case FrameMore{}:
      HsBad{}

def hs.split(+head: Bool, +k: U32, +hlen: U32, +len: U32, r: Bytes.Bytes & Bytes.Bytes) -> Hs:
  (buf, hb) = r
  hs.parsed(head, k, bytes.snd(Bytes.slice(buf, hlen, (len - hlen : U32))), parse.res.of(split_at_blank(Bytes.to_string(hb)), False{}, True{}))

def hs.hit(+head: Bool, +k: U32, +len: U32, buf: Bytes.Bytes, at: Maybe<&2, U32>) -> Hs:
  match at:
    case None{}:
      HsWait{buf, k}
    case Some{+i}:
      +hlen = (i + 4 : U32)
      hs.split(head, k, hlen, len, Bytes.slice(buf, 0, hlen))

def hs.seek.of(+head: Bool, +k: U32, +len: U32, +from: U32, buf: Bytes.Bytes, r: Bytes.Bytes & Maybe<&2, U32>) -> Hs:
  (tail, m) = r
  hs.hit(head, k, len, buf, found.at(from, m))

def hs.seek(+head: Bool, +k: U32, +len: U32, +from: U32, r: Bytes.Bytes & Bytes.Bytes) -> Hs:
  (buf, tail) = r
  hs.seek.of(head, k, len, from, buf, Bytes.find(tail, "\r\n\r\n"))

def hs.step(+head: Bool, st: Hs) -> Hs:
  match st:
    case HsSeek{buf, +from, +k}:
      Bytes.Bytes{+len, b} = buf
      hs.seek(head, k, len, from, Bytes.slice(Bytes.Bytes{len, b}, from, (len - from : U32)))
    case HsWait{buf, k}:
      HsWait{buf, k}
    case HsBad{}:
      HsBad{}
    case HsRes{res, rb, rest}:
      HsRes{res, rb, rest}

# Each step ends the scan or passes one interim head, and at most 8 interim heads pass.
def hs.scan(fuel: Nat, +head: Bool, st: Hs) -> Hs:
  match fuel:
    case 0n:
      match st:
        case HsSeek{buf, from, k}:
          HsBad{}
        case HsWait{buf, k}:
          HsWait{buf, k}
        case HsBad{}:
          HsBad{}
        case HsRes{res, rb, rest}:
          HsRes{res, rb, rest}
    case 1n+f:
      hs.scan(f, head, hs.step(head, st))

# piece joins buf; only the new bytes, and the 3 before them, can finish the blank line.
def hs.more(+head: Bool, +k: U32, buf: Bytes.Bytes, piece: Bytes.Bytes) -> Hs:
  Bytes.Bytes{+ol, ob} = buf
  hs.scan(10n, head, HsSeek{Bytes.append(Bytes.Bytes{ol, ob}, piece), Bool.pick(U32, U32.is_lt(ol, 3), 0, (ol - 3 : U32)), k})

# A response read in pieces. Head: the head is not whole yet. Body: it is, and parts is the body so far.
type Rv is Type:
  RvHead{buf: Bytes.Bytes, k: U32}
  RvBody{res: Res, rb: Rb, parts: List<&1, Bytes.Bytes>}

# More: read on. Done: the response, and the bytes after it when the body is self-delimited.
type Rt is Type:
  RtMore{rv: Rv}
  RtBad{}
  RtDone{res: Res, rest: Maybe<&2, String>}

def rv.start() -> Rv:
  RvHead{Bytes.new(0), 0}

def rv.done(res: Res, parts: List<&1, Bytes.Bytes>, rest: Maybe<&2, String>) -> Rt:
  Res{status, headers, body} = res
  RtDone{Res{status, headers, rb.body(parts)}, rest}

def rv.fed(res: Res, bf: Bf) -> Rt:
  Bf{rb, parts, rest} = bf
  match rb:
    case RbDone{}:
      rv.done(res, parts, Some{Bytes.to_string(rest)})
    case RbBad{}:
      RtBad{}
    case RbLen{n}:
      RtMore{RvBody{res, RbLen{n}, parts}}
    case RbChunk{st}:
      RtMore{RvBody{res, RbChunk{st}, parts}}
    case RbClose{}:
      RtMore{RvBody{res, RbClose{}, parts}}

def rv.head(hs: Hs) -> Rt:
  match hs:
    case HsWait{buf, k}:
      RtMore{RvHead{buf, k}}
    case HsBad{}:
      RtBad{}
    case HsSeek{buf, from, k}:
      RtBad{}
    case HsRes{res, rb, rest}:
      rv.fed(res, rb.feed(rb, Nil{}, rest))

def rv.eof(res: Res, parts: List<&1, Bytes.Bytes>, ok: Bool) -> Rt:
  match ok:
    case True{}:
      rv.done(res, parts, None{})
    case False{}:
      RtBad{}

# One read: piece, or the server closed (RFC 9112 §8: a message cut short is not a message).
def rv.step(closed: Bool, +head: Bool, rv: Rv, piece: Bytes.Bytes) -> Rt:
  match closed:
    case True{}:
      match rv:
        case RvHead{buf, k}:
          RtBad{}
        case RvBody{res, rb, parts}:
          rv.eof(res, parts, rb.eof(rb))
    case False{}:
      match rv:
        case RvHead{buf, +k}:
          rv.head(hs.more(head, k, buf, piece))
        case RvBody{res, rb, parts}:
          rv.fed(res, rb.feed(rb, parts, piece))

def rv.end(closed: Bool, +head: Bool, t: Rt) -> Rt:
  match closed:
    case False{}:
      t
    case True{}:
      match t:
        case RtMore{rv}:
          rv.step(True{}, head, rv, Bytes.new(0))
        case RtBad{}:
          RtBad{}
        case RtDone{res, rest}:
          RtDone{res, rest}

def rv.go(xs: List<&2, String>, closed: Bool, +head: Bool, t: Rt) -> Rt:
  match xs:
    case Nil{}:
      rv.end(closed, head, t)
    case Con{+x, rest}:
      match t:
        case RtMore{rv}:
          rv.go(rest, closed, head, rv.step(False{}, head, rv, Bytes.from_string(x)))
        case RtBad{}:
          RtBad{}
        case RtDone{res, r}:
          RtDone{res, r}

def rt.frame(t: Rt) -> Frame:
  match t:
    case RtMore{rv}:
      FrameMore{}
    case RtBad{}:
      FrameBad{}
    case RtDone{res, rest}:
      FrameDone{res}

# What the read loop makes of these pieces, then a close when closed is set.
def res.frame(xs: List<&2, String>, closed: Bool, +head: Bool) -> Frame:
  rt.frame(rv.go(xs, closed, head, RtMore{rv.start()}))

def rt.rest(t: Rt) -> Maybe<&2, String>:
  match t:
    case RtMore{rv}:
      None{}
    case RtBad{}:
      None{}
    case RtDone{res, rest}:
      rest

# Bytes after a complete self-delimited message. None if it is not one yet.
def after(+raw: String, +head: Bool) -> Maybe<&2, String>:
  rt.rest(rv.go([raw], False{}, head, RtMore{rv.start()}))

def req.v11.ver(mpv: String & String & String) -> Bool:
  (+method, path, v) = mpv
  String.eq(v, "HTTP/1.1")

def req.v11.line(+line: String) -> Bool:
  req.v11.ver(start_line(line))

def req.v11.lines(xs: List<&2, String>) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{line, headers}:
      req.v11.line(line)

def req.v11.pick(+head: String, ok: Bool) -> Bool:
  match ok:
    case False{}:
      False{}
    case True{}:
      req.v11.lines(String.lines(head))

def req.v11.of(hb: String & String) -> Bool:
  (+head, rest) = hb
  req.v11.pick(head, String.ends_with(head, "\r\n\r\n"))

def req.v11(+raw: String) -> Bool:
  req.v11.of(split_at_blank(raw))

def req.conn_close(+headers: Map<&2, List<&2, String>>) -> Bool:
  String.eq(String.to_lower(header(headers, "connection")), "close")

def res.conn_close(+headers: Map<&2, List<&2, String>>) -> Bool:
  String.eq(String.to_lower(header(headers, "connection")), "close")

def serve.fail_close(+status: U32) -> Bool:
  Bool.or(U32.is_eq(status, 400), Bool.or(U32.is_eq(status, 413), U32.is_eq(status, 431)))

def serve.keep.ok(fail: Bool, res_close: Bool) -> Bool:
  match fail:
    case True{}:
      False{}
    case False{}:
      Bool.not(res_close)

def serve.keep.v11(v11: Bool, +req_close: Bool, +status: U32, res_close: Bool) -> Bool:
  match v11:
    case False{}:
      False{}
    case True{}:
      match req_close:
        case True{}:
          False{}
        case False{}:
          serve.keep.ok(serve.fail_close(status), res_close)

def serve.keep(+headers: Map<&2, List<&2, String>>, v11: Bool, +status: U32, +res: Map<&2, List<&2, String>>) -> Bool:
  serve.keep.v11(v11, req.conn_close(headers), status, res.conn_close(res))


def again.rest(+close: Bool, +h: Map<&2, List<&2, String>>, rest: Maybe<&2, String>) -> Bool:
  match rest:
    case None{}:
      False{}
    case Some{x}:
      Bool.not(String.eq(String.to_lower(header(h, "connection")), "close"))

# True when the request did not ask to close, the response is self-delimited, and the peer did not say close.
def again(+close: Bool, h: Map<&2, List<&2, String>>, rest: Maybe<&2, String>) -> Bool:
  match close:
    case True{}:
      False{}
    case False{}:
      again.rest(close, h, rest)

def res_body.res(res: Res) -> String:
  Res{status, headers, body} = res
  body

def res_body(m: Maybe<&2, Res>) -> String:
  match m:
    case None{}:
      ""
    case Some{res}:
      res_body.res(res)

def text(res: Res) -> String:
  Enc.utf8.decode(res_body.res(res))

def json(res: Res) -> Maybe<&2, Json.Val>:
  Json.parse(text(res))

def got_body.req(req: Req) -> String:
  Req{method, path, headers, body} = req
  body

def got_body(m: Maybe<&2, Req>) -> String:
  match m:
    case None{}:
      ""
    case Some{req}:
      got_body.req(req)

def body.of(hb: String & String) -> String:
  (h, b) = hb
  b

def body(s: String) -> String:
  body.of(split_at_blank(s))

def headers_lines(+k: String, vs: List<&2, String>, acc: String) -> String:
  match vs:
    case Nil{}:
      acc
    case Con{v, t}:
      headers_lines(k, t, acc ++ ("\r\n" ++ sanitize(k) ++ ": " ++ sanitize(v)))

def headers_block.go(xs: List<&2, Sigma<&2, &2, String, _ => List<&2, String>>>, acc: String) -> String:
  match xs:
    case Nil{}:
      acc
    case (k, vs) <> t:
      headers_block.go(t, headers_lines(k, vs, acc))

def headers_block(h: Map<&2, List<&2, String>>) -> String:
  headers_block.go(Map.to_list(&2, List<&2, String>, h), "")

# RFC 9110 §15. An unknown code gets an empty reason, which RFC 9112 §4 allows.
type Reason is Data:
  Reason{code: U32, text: String}

def reasons() -> List<&2, Reason>:
  [Reason{100, "Continue"}, Reason{101, "Switching Protocols"}, Reason{103, "Early Hints"},
   Reason{200, "OK"}, Reason{201, "Created"}, Reason{202, "Accepted"}, Reason{203, "Non-Authoritative Information"}, Reason{204, "No Content"}, Reason{205, "Reset Content"}, Reason{206, "Partial Content"},
   Reason{300, "Multiple Choices"}, Reason{301, "Moved Permanently"}, Reason{302, "Found"}, Reason{303, "See Other"}, Reason{304, "Not Modified"}, Reason{307, "Temporary Redirect"}, Reason{308, "Permanent Redirect"},
   Reason{400, "Bad Request"}, Reason{401, "Unauthorized"}, Reason{402, "Payment Required"}, Reason{403, "Forbidden"}, Reason{404, "Not Found"}, Reason{405, "Method Not Allowed"}, Reason{406, "Not Acceptable"}, Reason{407, "Proxy Authentication Required"}, Reason{408, "Request Timeout"}, Reason{409, "Conflict"}, Reason{410, "Gone"}, Reason{411, "Length Required"}, Reason{412, "Precondition Failed"}, Reason{413, "Content Too Large"}, Reason{414, "URI Too Long"}, Reason{415, "Unsupported Media Type"}, Reason{416, "Range Not Satisfiable"}, Reason{417, "Expectation Failed"}, Reason{421, "Misdirected Request"}, Reason{422, "Unprocessable Content"}, Reason{426, "Upgrade Required"}, Reason{428, "Precondition Required"}, Reason{429, "Too Many Requests"}, Reason{431, "Request Header Fields Too Large"},
   Reason{500, "Internal Server Error"}, Reason{501, "Not Implemented"}, Reason{502, "Bad Gateway"}, Reason{503, "Service Unavailable"}, Reason{504, "Gateway Timeout"}, Reason{505, "HTTP Version Not Supported"}]

def reason.go(xs: List<&2, Reason>, +status: U32) -> String:
  match xs:
    case Nil{}:
      ""
    case Con{Reason{k, v}, t}:
      Bool.pick(String, U32.is_eq(k, status), v, reason.go(t, status))

def reason(+status: U32) -> String:
  reason.go(reasons(), status)

def response(+status: U32, headers: Map<&2, List<&2, String>>, body: String) -> String:
  ("HTTP/1.1 " ++ U32.show(status) ++ " " ++ reason(status) ++ headers_block(headers)) ++ ("\r\n\r\n" ++ body)

def nat_u32(n: Nat) -> U32:
  match n:
    case 0n:
      0
    case 1n+p:
      (nat_u32(p) + 1 : U32)

# RFC 9110 §6.4.1, §8.6: 1xx, 204 and 304 have no body; HEAD gets the length but no body.
def encode.pick(+status: U32, headers: Map<&2, List<&2, String>>, +body: String, head: Bool, nobody: Bool) -> String:
  match nobody:
    case True{}:
      response(status, headers, "")
    case False{}:
      response(status, set(headers, "content-length", U32.show(nat_u32(String.length(body)))), Bool.pick(String, head, "", body))

def encode.on(r: Res, head: Bool) -> String:
  Res{+status, headers, body} = r
  encode.pick(status, headers, body, head, frame.nobody(status, False{}))

def encode(r: Res) -> String:
  encode.on(r, False{})

def request(method: String, path: String, headers: Map<&2, List<&2, String>>, body: String) -> String:
  (method ++ " " ++ path ++ " HTTP/1.1" ++ headers_block(headers)) ++ ("\r\n\r\n" ++ body)

def req.reserved(+k: String) -> Bool:
  Bool.or(Bool.or(String.eq(k, "host"), String.eq(k, "connection")), Bool.or(String.eq(k, "content-length"), String.eq(k, "transfer-encoding")))

def req.put.one(m: Map<&2, List<&2, String>>, +k: String, vs: List<&2, String>, skip: Bool) -> Map<&2, List<&2, String>>:
  match skip:
    case True{}:
      m
    case False{}:
      Map.set(&2, List<&2, String>, m, k, vs)

def req.put(xs: List<&2, Sigma<&2, &2, String, _ => List<&2, String>>>, m: Map<&2, List<&2, String>>) -> Map<&2, List<&2, String>>:
  match xs:
    case Nil{}:
      m
    case (k, vs) <> t:
      +lk = String.to_lower(k)
      req.put(t, req.put.one(m, lk, vs, req.reserved(lk)))

# ---- body parts read from a file at send time ----
# A body may hold file parts, file_part(path, size): NUL "bend-file:" size ":"
# path NUL. Content-Length counts the file's size, not the part's text, and
# io.send sends the file's bytes from a C buffer (Wire.send.file): they never
# enter the Bend heap (a model request's images: hundreds of KB of base64
# each, ~47 bytes of heap per char as a String, and a heap that never
# shrinks). For text bodies (JSON) that hold no raw NUL of their own; a body
# without the marker is measured and sent as before.
def file_mark() -> String:
  SCon{Chr{0}, "bend-file:"}

def file_part(+path: String, +size: U32) -> String:
  String.append(file_mark(), String.append(U32.show(size),
    String.append(":", String.append(path, SCon{Chr{0}, SNil{}}))))

type WPart is Data:
  WText{s: String}
  WFile{path: String, size: U32}

# "bend-file:<size>:<path>" (an odd segment of the wire split at NUL)
def wpart.size(m: Maybe<&2, U32>, +path: String, +seg: String) -> WPart:
  match m:
    case Some{+n}:
      WFile{path, n}
    case None{}:
      WText{seg}

def wpart.file(+rest: String, +seg: String) -> WPart:
  +n = String.concat(List.take(&2, String, String.split(rest, Chr{58}), 1n))
  wpart.size(parse_u32(n), String.drop(rest, Nat.add(String.length(n), 1n)), seg)

def wpart.odd(+seg: String) -> WPart:
  Bool.pick(WPart, String.starts_with(seg, "bend-file:"),
    wpart.file(String.drop(seg, 10n), seg), WText{seg})

def fparts.add(empty: Bool, +x: String, acc: List<&2, WPart>) -> List<&2, WPart>:
  match empty:
    case True{}:
      acc
    case False{}:
      Con{WText{x}, acc}

def fparts.go(xs: List<&2, String>, odd: Bool, acc: List<&2, WPart>) -> List<&2, WPart>:
  match xs:
    case Nil{}:
      List.reverse(&2, WPart, acc)
    case Con{+x, t}:
      match odd:
        case True{}:
          fparts.go(t, False{}, Con{wpart.odd(x), acc})
        case False{}:
          fparts.go(t, True{}, fparts.add(String.is_empty(x), x, acc))

def fparts(+s: String) -> List<&2, WPart>:
  fparts.go(String.split(s, Chr{0}), False{}, Nil{})

def wpart.len(p: WPart) -> Nat:
  match p:
    case WText{+t}:
      String.length(t)
    case WFile{_, +n}:
      U32.to_nat(n)

def fparts.len.go(ps: List<&2, WPart>, +acc: Nat) -> Nat:
  match ps:
    case Nil{}:
      acc
    case Con{p, t}:
      fparts.len.go(t, Nat.add(acc, wpart.len(p)))

# the bytes a body puts on the wire: its file parts count their file's size
def fparts.len(+s: String) -> Nat:
  Bool.pick(Nat, String.contains(s, file_mark()), fparts.len.go(fparts(s), 0n), String.length(s))

# RFC 9110 §8.6: no Content-Length on a bodiless request whose method gives a body no meaning.
def req.cl(+m: Map<&2, List<&2, String>>, +method: String, +body: String) -> Map<&2, List<&2, String>>:
  +bodied = Bool.or(Bool.not(String.is_empty(body)), Bool.or(String.eq(method, "POST"), Bool.or(String.eq(method, "PUT"), String.eq(method, "PATCH"))))
  Bool.pick(Map<&2, List<&2, String>>, bodied, set(m, "content-length", U32.show(nat_u32(fparts.len(body)))), m)

# Caller headers are lowercased so they cannot duplicate ours; host, connection,
# content-length and transfer-encoding are always ours (no smuggling through them).
def encode_req.word(close: Bool) -> String:
  match close:
    case True{}:
      "close"
    case False{}:
      "keep-alive"

def encode_req.on(+method: String, target: String, host: String, headers: Map<&2, List<&2, String>>, +body: String, close: Bool) -> String:
  +h = set(set(req.put(Map.to_list(&2, List<&2, String>, headers), empty()), "host", host), "connection", encode_req.word(close))
  request(method, target, req.cl(h, method, body), body)

def encode_req(+method: String, target: String, host: String, headers: Map<&2, List<&2, String>>, +body: String) -> String:
  encode_req.on(method, target, host, headers, body, True{})

# Redirects (RFC 9110 §15.4, as WHATWG fetch applies them)
# --------------------------------------------------------

# One request to make: headers are already lowercased (see req.put).
type Hop is Data:
  Hop{method: String, url: Url.Abs, headers: Map<&2, List<&2, String>>, body: String}

def redirect.code(+s: U32) -> Bool:
  Bool.or(Bool.or(U32.is_eq(s, 301), U32.is_eq(s, 302)), Bool.or(U32.is_eq(s, 303), Bool.or(U32.is_eq(s, 307), U32.is_eq(s, 308))))

def redirect.origin(a: Url.Abs) -> String:
  Url.Abs{scheme, host, port, target} = a
  scheme ++ "://" ++ host ++ ":" ++ U32.show(port)

def redirect.del(ks: List<&2, String>, m: Map<&2, List<&2, String>>) -> Map<&2, List<&2, String>>:
  match ks:
    case Nil{}:
      m
    case Con{k, t}:
      redirect.del(t, Map.del(&2, List<&2, String>, m, k))

def redirect.hdrs(+h: Map<&2, List<&2, String>>, drop: Bool, cross: Bool) -> Map<&2, List<&2, String>>:
  +h2 = Bool.pick(Map<&2, List<&2, String>>, drop, redirect.del(["content-type", "content-encoding", "content-language", "content-location"], h), h)
  Bool.pick(Map<&2, List<&2, String>>, cross, redirect.del(["authorization", "cookie", "proxy-authorization"], h2), h2)

# 303 turns any method but HEAD into GET; 301 and 302 turn POST into GET;
# 307 and 308 keep method and body. A GET carries no body or body headers.
# Credentials do not follow a hop to another origin.
def redirect.to(+status: U32, +method: String, +from: Url.Abs, headers: Map<&2, List<&2, String>>, +body: String, next: Maybe<&2, Url.Abs>) -> Maybe<&2, Hop>:
  match next:
    case None{}:
      None{}
    case Some{+to}:
      +drop = Bool.or(Bool.and(U32.is_eq(status, 303), Bool.not(String.eq(method, "HEAD"))), Bool.and(Bool.or(U32.is_eq(status, 301), U32.is_eq(status, 302)), String.eq(method, "POST")))
      Some{Hop{Bool.pick(String, drop, "GET", method), to, redirect.hdrs(headers, drop, Bool.not(String.eq(redirect.origin(from), redirect.origin(to)))), Bool.pick(String, drop, "", body)}}

def redirect.hop(status: U32, h: Hop, loc: String) -> Maybe<&2, Hop>:
  Hop{method, +url, headers, body} = h
  redirect.to(status, method, url, headers, body, Url.resolve(url, loc))

def redirect.if(status: U32, h: Hop, loc: String, ok: Bool) -> Maybe<&2, Hop>:
  match ok:
    case False{}:
      None{}
    case True{}:
      redirect.hop(status, h, loc)

# The next request after a response, or None when it is not a redirect we follow.
def redirect(+status: U32, +headers: Map<&2, List<&2, String>>, h: Hop) -> Maybe<&2, Hop>:
  redirect.if(status, h, header(headers, "location"), Bool.and(redirect.code(status), has_header(headers, "location")))

def io.recv.words(tls: Bool, s: Socket, max: U32, ms: U32) -> IO(Socket & Result<&1, &1, U32 & String, U32 & Array<U32>>):
  match tls:
    case True{}:
      Wire.tls.recv.words(s, max, ms)
    case False{}:
      Wire.recv.words(s, max, ms)

def io.send.text(tls: Bool, s: Socket, data: String) -> IO(Socket & Result<&1, &1, U32 & String, Unit>):
  match tls:
    case True{}:
      Wire.tls.send(s, data)
    case False{}:
      Wire.send(s, data)

def io.send.file(tls: Bool, s: Socket, +path: String, +size: U32) -> IO(Socket & Result<&1, &1, U32 & String, Unit>):
  match tls:
    case True{}:
      Wire.tls.send.file(s, path, size)
    case False{}:
      Wire.send.file(s, path, size)

def io.send.part(tls: Bool, s: Socket, p: WPart) -> IO(Socket & Result<&1, &1, U32 & String, Unit>):
  match p:
    case WText{+t}:
      io.send.text(tls, s, t)
    case WFile{+path, +size}:
      io.send.file(tls, s, path, size)

# the parts in order; the first failed send ends it
@unsafe
def io.send.parts(ps: List<&2, WPart>, +tls: Bool, s: Socket) -> IO(Socket & Result<&1, &1, U32 & String, Unit>):
  match ps:
    case Nil{}:
      IO.pure(Socket & Result<&1, &1, U32 & String, Unit>, (s, Done{Unit{}}))
    case Con{p, rest}:
      do IO<Socket & Result<&1, &1, U32 & String, Unit>>:
        m : Socket & Result<&1, &1, U32 & String, Unit> <- io.send.part(tls, s, p)
        io.send.after(rest, tls, m)

@unsafe
def io.send.after(rest: List<&2, WPart>, +tls: Bool, m: Socket & Result<&1, &1, U32 & String, Unit>) -> IO(Socket & Result<&1, &1, U32 & String, Unit>):
  (s, r) = m
  match r:
    case Fail{e}:
      IO.pure(Socket & Result<&1, &1, U32 & String, Unit>, (s, Fail{e}))
    case Done{u}:
      io.send.parts(rest, tls, s)

def io.send.pick(+tls: Bool, s: Socket, +data: String, parts: Bool) -> IO(Socket & Result<&1, &1, U32 & String, Unit>):
  match parts:
    case True{}:
      io.send.parts(fparts(data), tls, s)
    case False{}:
      io.send.text(tls, s, data)

# A wire with file parts (file_part) goes out in pieces: its text, and each
# file's bytes from a C buffer. A wire without one: one send, as always.
def io.send(+tls: Bool, s: Socket, +data: String) -> IO(Socket & Result<&1, &1, U32 & String, Unit>):
  io.send.pick(tls, s, data, String.contains(data, file_mark()))

def io.close(tls: Bool, s: Socket) -> IO(Unit):
  match tls:
    case True{}:
      Wire.tls.close(s)
    case False{}:
      Socket.close(s)

def fetch.close(tls: Bool, s: Socket, r: Result<&2, &2, Err, Res>) -> IO(Result<&2, &2, Err, Res>):
  do IO<Result<&2, &2, Err, Res>>:
    io.close(tls, s)
    return r

# Untrusted servers must not grow the buffer without bound.
def fetch.max() -> Nat:
  U32.to_nat(16777216)

# none: no response byte arrived, so an idempotent request may be retried (RFC 9112 §9.3.1).
# sock: the socket, when it can take another request.
type Out is Type:
  OutDone{res: Res, sock: Maybe<&1, Socket>, rest: String}
  OutFail{e: Err, none: Bool}

def fetch.boxed(r: Result<&2, &2, Err, Res>, none: Bool) -> Out:
  match r:
    case Done{res}:
      OutDone{res, None{}, ""}
    case Fail{e}:
      OutFail{e, none}

def fetch.bad(tls: Bool, s: Socket, e: Maybe<&2, Err>) -> IO(Result<&2, &2, Err, Res>):
  match e:
    case None{}:
      fetch.close(tls, s, Fail{ErrBad{}})
    case Some{err}:
      fetch.close(tls, s, Fail{err})

def fetch.rest(m: Maybe<&2, String>) -> String:
  match m:
    case None{}:
      ""
    case Some{s}:
      s

def fetch.shut(tls: Bool, s: Socket, r: Result<&2, &2, Err, Res>, none: Bool) -> IO(Out):
  do IO<Out>:
    x : Result<&2, &2, Err, Res> <- fetch.close(tls, s, r)
    return fetch.boxed(x, none)

def fetch.hold.go(tls: Bool, s: Socket, res: Res, left: Maybe<&2, String>, ok: Bool) -> IO(Out):
  match ok:
    case True{}:
      IO.pure(Out, OutDone{res, Some{s}, fetch.rest(left)})
    case False{}:
      fetch.shut(tls, s, Done{res}, False{})

def fetch.hold(tls: Bool, +close: Bool, s: Socket, res: Res, +left: Maybe<&2, String>) -> IO(Out):
  Res{status, +headers, body} = res
  fetch.hold.go(tls, s, Res{status, headers, body}, left, again(close, headers, left))

def fetch.failout(tls: Bool, s: Socket, e: Maybe<&2, Err>, none: Bool) -> IO(Out):
  do IO<Out>:
    r : Result<&2, &2, Err, Res> <- fetch.bad(tls, s, e)
    return fetch.boxed(r, none)

# Read state: the response so far and the bytes read, or how it ended.
# none: no byte arrived, so an idempotent request may be retried.
type Fl is Type:
  FlMore{rv: Rv, n: U32}
  FlBad{err: Maybe<&2, Err>, none: Bool}
  FlDone{res: Res, rest: Maybe<&2, String>}

def fetch.rt(+n: U32, t: Rt) -> Fl:
  match t:
    case RtMore{rv}:
      FlMore{rv, n}
    case RtBad{}:
      FlBad{None{}, U32.is_zero(n)}
    case RtDone{res, rest}:
      FlDone{res, rest}

# 64 KiB of head past the body cap, as serve allows.
def fetch.over(+n: U32) -> Bool:
  Nat.is_lt(Nat.add(fetch.max(), 65536n), U32.to_nat(n))

def fetch.piece.cap(over: Bool, +head: Bool, rv: Rv, +n2: U32, +len: U32, buf: Array<U32>) -> Fl:
  match over:
    case True{}:
      FlBad{None{}, False{}}
    case False{}:
      fetch.rt(n2, rv.step(U32.is_zero(len), head, rv, Bytes.Bytes{len, buf}))

def fetch.piece(+head: Bool, rv: Rv, +n: U32, p: U32 & Array<U32>) -> Fl:
  (+len, buf) = p
  +n2 = (n + len : U32)
  fetch.piece.cap(fetch.over(n2), head, rv, n2, len, buf)

def fetch.next(+head: Bool, rv: Rv, +n: U32, m: Socket & Result<&1, &1, U32 & String, U32 & Array<U32>>) -> Socket & Fl:
  (s, r) = m
  match r:
    case Fail{(+code, why)}:
      (s, FlBad{Some{err.or_late(code, ErrRead{code, why})}, U32.is_zero(n)})
    case Done{p}:
      (s, fetch.piece(head, rv, n, p))

# Reads until the response is whole or cannot be; an empty read means the server closed.
@unsafe
def fetch.loop(+close: Bool, +head: Bool, +tls: Bool, +ms: U32, st: Socket & Fl) -> IO(Out):
  (s, fl) = st
  match fl:
    case FlMore{rv, +n}:
      do IO<Out>:
        m : Socket & Result<&1, &1, U32 & String, U32 & Array<U32>> <- io.recv.words(tls, s, 65536, ms)
        fetch.loop(close, head, tls, ms, fetch.next(head, rv, n, m))
    case FlBad{err, none}:
      fetch.failout(tls, s, err, none)
    case FlDone{res, rest}:
      fetch.hold(tls, close, s, res, rest)

def exchange.sent(+tls: Bool, +ms: U32, +close: Bool, +head: Bool, m: Socket & Result<&1, &1, U32 & String, Unit>) -> IO(Out):
  (s, r) = m
  match r:
    case Fail{(+code, why)}:
      fetch.shut(tls, s, Fail{err.or_late(code, ErrWrite{code, why})}, True{})
    case Done{u}:
      fetch.loop(close, head, tls, ms, (s, FlMore{rv.start(), 0}))

def exchange.go(+tls: Bool, +ms: U32, +close: Bool, +head: Bool, s: Socket, wire: String) -> IO(Out):
  do IO<Out>:
    sent : Socket & Result<&1, &1, U32 & String, Unit> <- io.send(tls, s, wire)
    exchange.sent(tls, ms, close, head, sent)

def exchange.tup(o: Out) -> Maybe<&1, Socket> & Result<&2, &2, Err, Res> & String:
  match o:
    case OutFail{e, none}:
      (None{}, Fail{e}, "")
    case OutDone{res, sock, rest}:
      (sock, Done{res}, rest)

# One request on an open socket. Some{socket}: it can take another request.
def exchange(+tls: Bool, +ms: U32, +close: Bool, +head: Bool, s: Socket, wire: String) -> IO(Maybe<&1, Socket> & Result<&2, &2, Err, Res> & String):
  do IO<Maybe<&1, Socket> & Result<&2, &2, Err, Res> & String>:
    o : Out <- exchange.go(tls, ms, close, head, s, wire)
    return exchange.tup(o)

# Connection pool: idle sockets by scheme, host, and port. fetch uses a fresh pool per call.
type Conn is Type:
  Conn{tls: Bool, s: Socket}

type Pool is Type:
  Pool{idle: Map<&1, Conn>}

def pool.new() -> Pool:
  Pool{Map.new(&1, Conn)}

def pool.key(+scheme: String, +host: String, +port: U32) -> String:
  scheme ++ "://" ++ host ++ ":" ++ U32.show(port)

def pool.take.of(r: Map<&1, Conn> & Maybe<&1, Conn>) -> Pool & Maybe<&1, Conn>:
  (m, c) = r
  (Pool{m}, c)

def pool.take(p: Pool, +key: String) -> Pool & Maybe<&1, Conn>:
  Pool{idle} = p
  pool.take.of(Map.pop(&1, Conn, idle, key))

def conn.close(c: Conn) -> IO(Unit):
  Conn{tls, s} = c
  io.close(tls, s)

def conn.drop(m: Maybe<&1, Conn>) -> IO(Unit):
  match m:
    case None{}:
      IO.pure(Unit, Unit{})
    case Some{c}:
      conn.close(c)

# ponytail: one idle socket per origin; a second one replaces the first.
def pool.give.of(+key: String, c: Conn, r: Map<&1, Conn> & Maybe<&1, Conn>) -> IO(Pool):
  (m, old) = r
  do IO<Pool>:
    conn.drop(old)
    return Pool{Map.set(&1, Conn, m, key, c)}

def pool.give(p: Pool, +key: String, c: Conn) -> IO(Pool):
  Pool{idle} = p
  pool.give.of(key, c, Map.pop(&1, Conn, idle, key))

def pool.close.go(xs: List<&1, Conn>) -> IO(Unit):
  match xs:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case Con{c, t}:
      do IO<Unit>:
        conn.close(c)
        pool.close.go(t)

def pool.close(p: Pool) -> IO(Unit):
  Pool{idle} = p
  pool.close.go(Map.values(&1, Conn, idle))

# RFC 9110 §9.2.2
def pool.idempotent(+method: String) -> Bool:
  Bool.or(String.eq(method, "GET"), Bool.or(String.eq(method, "HEAD"), Bool.or(String.eq(method, "OPTIONS"), Bool.or(String.eq(method, "TRACE"), Bool.or(String.eq(method, "PUT"), String.eq(method, "DELETE"))))))

def conn.tls(m: Socket & Result<&1, &1, U32 & String, Unit>) -> IO(Result<&1, &1, Err, Conn>):
  (s, r) = m
  match r:
    case Fail{(+code, why)}:
      do IO<Result<&1, &1, Err, Conn>>:
        Wire.tls.close(s)
        return Fail{err.or_late(code, ErrTls{code, why})}
    case Done{u}:
      IO.pure(Result<&1, &1, Err, Conn>, Done{Conn{True{}, s}})

# A failed handshake is an error, never plaintext.
def conn.secure(+ms: U32, sni: String, s: Socket, tls: Bool) -> IO(Result<&1, &1, Err, Conn>):
  match tls:
    case False{}:
      IO.pure(Result<&1, &1, Err, Conn>, Done{Conn{False{}, s}})
    case True{}:
      do IO<Result<&1, &1, Err, Conn>>:
        hs : Socket & Result<&1, &1, U32 & String, Unit> <- Wire.tls.connect(s, sni, ms)
        conn.tls(hs)

def conn.made(+ms: U32, sni: String, tls: Bool, r: Result<&1, &1, U32 & String, Socket>) -> IO(Result<&1, &1, Err, Conn>):
  match r:
    case Fail{(+code, why)}:
      IO.pure(Result<&1, &1, Err, Conn>, Fail{err.or_late(code, ErrConnect{code, why})})
    case Done{s}:
      conn.secure(ms, sni, s, tls)

def conn.open(+tls: Bool, +ms: U32, sni: String, ip: String, port: U32) -> IO(Result<&1, &1, Err, Conn>):
  do IO<Result<&1, &1, Err, Conn>>:
    c : Result<&1, &1, U32 & String, Socket> <- Wire.connect(ip, port, ms)
    conn.made(ms, sni, tls, c)

def conn.ex(c: Conn, +ms: U32, +head: Bool, wire: String) -> IO(Out):
  Conn{+tls, s} = c
  exchange.go(tls, ms, False{}, head, s, wire)

# A socket with bytes past the response is out of step with the server, so it is closed.
def pool.keep.s(p: Pool, +key: String, +tls: Bool, res: Res, s: Socket, clean: Bool) -> IO(Pool & Result<&2, &2, Err, Res>):
  match clean:
    case True{}:
      do IO<Pool & Result<&2, &2, Err, Res>>:
        p2 : Pool <- pool.give(p, key, Conn{tls, s})
        return (p2, Done{res})
    case False{}:
      do IO<Pool & Result<&2, &2, Err, Res>>:
        io.close(tls, s)
        return (p, Done{res})

def pool.keep(p: Pool, +key: String, +tls: Bool, res: Res, sock: Maybe<&1, Socket>, +rest: String) -> IO(Pool & Result<&2, &2, Err, Res>):
  match sock:
    case None{}:
      IO.pure(Pool & Result<&2, &2, Err, Res>, (p, Done{res}))
    case Some{s}:
      pool.keep.s(p, key, tls, res, s, String.is_empty(rest))

def pool.after(p: Pool, +key: String, +tls: Bool, o: Out) -> IO(Pool & Result<&2, &2, Err, Res>):
  match o:
    case OutFail{e, none}:
      IO.pure(Pool & Result<&2, &2, Err, Res>, (p, Fail{e}))
    case OutDone{res, sock, rest}:
      pool.keep(p, key, tls, res, sock, rest)

def pool.fresh.conn(p: Pool, +key: String, +tls: Bool, +ms: U32, +head: Bool, wire: String, r: Result<&1, &1, Err, Conn>) -> IO(Pool & Result<&2, &2, Err, Res>):
  match r:
    case Fail{e}:
      IO.pure(Pool & Result<&2, &2, Err, Res>, (p, Fail{e}))
    case Done{c}:
      do IO<Pool & Result<&2, &2, Err, Res>>:
        o : Out <- conn.ex(c, ms, head, wire)
        pool.after(p, key, tls, o)

def pool.fresh.ip(p: Pool, +key: String, +tls: Bool, +ms: U32, +head: Bool, +host: String, +port: U32, wire: String, ip: Maybe<&2, String>) -> IO(Pool & Result<&2, &2, Err, Res>):
  match ip:
    case None{}:
      IO.pure(Pool & Result<&2, &2, Err, Res>, (p, Fail{ErrDns{}}))
    case Some{addr}:
      do IO<Pool & Result<&2, &2, Err, Res>>:
        c : Result<&1, &1, Err, Conn> <- conn.open(tls, ms, host, addr, port)
        pool.fresh.conn(p, key, tls, ms, head, wire, c)

def pool.fresh(p: Pool, +key: String, +tls: Bool, +ms: U32, +head: Bool, +host: String, +port: U32, wire: String) -> IO(Pool & Result<&2, &2, Err, Res>):
  do IO<Pool & Result<&2, &2, Err, Res>>:
    ip : Maybe<&2, String> <- Dns.resolve(host)
    pool.fresh.ip(p, key, tls, ms, head, host, port, wire, ip)

# A reused socket the server already closed fails before any response byte.
def pool.retry(+none: Bool, +idem: Bool) -> Bool:
  Bool.and(none, idem)

def pool.stale(p: Pool, +key: String, +tls: Bool, +ms: U32, +head: Bool, +host: String, +port: U32, wire: String, e: Err, again: Bool) -> IO(Pool & Result<&2, &2, Err, Res>):
  match again:
    case True{}:
      pool.fresh(p, key, tls, ms, head, host, port, wire)
    case False{}:
      IO.pure(Pool & Result<&2, &2, Err, Res>, (p, Fail{e}))

def pool.reused(p: Pool, +key: String, +tls: Bool, +ms: U32, +head: Bool, +idem: Bool, +host: String, +port: U32, wire: String, o: Out) -> IO(Pool & Result<&2, &2, Err, Res>):
  match o:
    case OutFail{e, +none}:
      pool.stale(p, key, tls, ms, head, host, port, wire, e, pool.retry(none, idem))
    case OutDone{res, sock, rest}:
      pool.keep(p, key, tls, res, sock, rest)

def pool.idle(taken: Pool & Maybe<&1, Conn>, +key: String, +tls: Bool, +ms: U32, +head: Bool, +idem: Bool, +host: String, +port: U32, +wire: String) -> IO(Pool & Result<&2, &2, Err, Res>):
  (p, m) = taken
  match m:
    case None{}:
      pool.fresh(p, key, tls, ms, head, host, port, wire)
    case Some{c}:
      do IO<Pool & Result<&2, &2, Err, Res>>:
        o : Out <- conn.ex(c, ms, head, wire)
        pool.reused(p, key, tls, ms, head, idem, host, port, wire, o)

def pool.origin(p: Pool, +method: String, +ms: U32, headers: Map<&2, List<&2, String>>, body: String, +hf: String, a: Url.Abs, known: Bool) -> IO(Pool & Result<&2, &2, Err, Res>):
  Url.Abs{+scheme, +host, +port, target} = a
  match known:
    case False{}:
      IO.pure(Pool & Result<&2, &2, Err, Res>, (p, Fail{ErrUrl{}}))
    case True{}:
      +key = pool.key(scheme, host, port)
      pool.idle(pool.take(p, key), key, String.eq(scheme, "https"), ms, String.eq(method, "HEAD"), pool.idempotent(method), host, port, encode_req.on(method, target, hf, headers, body, False{}))

def pool.scheme.ok(a: Url.Abs) -> Bool:
  Url.Abs{+scheme, host, port, target} = a
  Bool.or(String.eq(scheme, "http"), String.eq(scheme, "https"))

# One request, on an idle socket to its origin when the pool has one.
def pool.one(p: Pool, h: Hop, +ms: U32) -> IO(Pool & Result<&2, &2, Err, Res>):
  Hop{method, +url, headers, body} = h
  pool.origin(p, method, ms, headers, body, Url.host_field(url), url, pool.scheme.ok(url))

type Next is Data:
  NDone{res: Result<&2, &2, Err, Res>}
  NStop{}
  NGo{left: Nat, hop: Hop}

type Mode is Data:
  ModeFollow{}
  ModeManual{}
  ModeError{}

# left: redirects still allowed after the request we just finished.
def fetch.hop(left: Nat, h: Hop) -> Next:
  match left:
    case 0n:
      NStop{}
    case 1n+p:
      NGo{p, h}

def fetch.decide.r(left: Nat, +r: Res, next: Maybe<&2, Hop>) -> Next:
  match next:
    case None{}:
      NDone{Done{r}}
    case Some{h}:
      fetch.hop(left, h)

def fetch.decide.res(left: Nat, h: Hop, +r: Res) -> Next:
  Res{status, headers, body} = r
  fetch.decide.r(left, r, redirect(status, headers, h))

def fetch.decide(left: Nat, h: Hop, m: Result<&2, &2, Err, Res>) -> Next:
  match m:
    case Fail{e}:
      NDone{Fail{e}}
    case Done{r}:
      fetch.decide.res(left, h, r)

def fetch.error.res(r: Res, bad: Bool) -> Next:
  match bad:
    case True{}:
      NDone{Fail{ErrRedirect{}}}
    case False{}:
      NDone{Done{r}}

def fetch.error(h: Hop, m: Result<&2, &2, Err, Res>) -> Next:
  match m:
    case Fail{e}:
      NDone{Fail{e}}
    case Done{r}:
      Res{+status, +headers, body} = r
      fetch.error.res(r, Bool.and(redirect.code(status), has_header(headers, "location")))

def fetch.policy(mode: Mode, left: Nat, h: Hop, m: Result<&2, &2, Err, Res>) -> Next:
  match mode:
    case ModeFollow{}:
      fetch.decide(left, h, m)
    case ModeManual{}:
      NDone{m}
    case ModeError{}:
      fetch.error(h, m)

# A caller's own Accept-Encoding wins.
def accept.enc(+h: Map<&2, List<&2, String>>) -> Map<&2, List<&2, String>>:
  Bool.pick(Map<&2, List<&2, String>>, has_header(h, "accept-encoding"), h, set(h, "accept-encoding", "gzip, deflate"))

def fetch.start(method: String, headers: Map<&2, List<&2, String>>, body: String, u: Maybe<&2, Url.Abs>) -> Next:
  match u:
    case None{}:
      NDone{Fail{ErrUrl{}}}
    case Some{a}:
      NGo{20n, Hop{method, a, accept.enc(req.put(Map.to_list(&2, List<&2, String>, headers), empty())), body}}

def pool.next(+mode: Mode, left: Nat, h: Hop, pr: Pool & Result<&2, &2, Err, Res>) -> Pool & Next:
  (p, m) = pr
  (p, fetch.policy(mode, left, h, m))

# NGo.left is how many redirects may follow this request. 20 is WHATWG's limit.
@unsafe
def pool.hops(+mode: Mode, +ms: U32, st: Pool & Next) -> IO(Pool & Result<&2, &2, Err, Res>):
  (p, n) = st
  match n:
    case NDone{m}:
      IO.pure(Pool & Result<&2, &2, Err, Res>, (p, m))
    case NStop{}:
      IO.pure(Pool & Result<&2, &2, Err, Res>, (p, Fail{ErrRedirect{}}))
    case NGo{+left, +h}:
      do IO<Pool & Result<&2, &2, Err, Res>>:
        pr : Pool & Result<&2, &2, Err, Res> <- pool.one(p, h, ms)
        pool.hops(mode, ms, pool.next(mode, left, h, pr))

# Content-Encoding (RFC 9110 §8.4): each value, trimmed and lowercased, in the order applied.
def ce.keep(+w: String, +rest: List<&2, String>) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(w), rest, Con{w, rest})

def ce.norm(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{v, t}:
      ce.keep(String.to_lower(String.trim(v)), ce.norm(t))

def ce.split(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{v, t}:
      List.append(&2, String, ce.norm(String.split(v, ',')), ce.split(t))

# Ok: decoded so far. Bad: a coding failed. Skip: a coding we cannot decode, so the body stays as sent.
type Dec is Data:
  DecOk{b: String}
  DecBad{}
  DecSkip{}

def ce.of(m: Maybe<&2, String>) -> Dec:
  match m:
    case None{}:
      DecBad{}
    case Some{b}:
      DecOk{b}

# Servers send deflate both with and without the zlib wrapper.
def ce.deflate(+b: String, m: Maybe<&2, String>) -> Dec:
  match m:
    case Some{out}:
      DecOk{out}
    case None{}:
      ce.of(Zlib.inflate(b))

def ce.apply(+c: String, +b: String) -> Dec:
  Bool.pick(Dec, Bool.or(String.eq(c, "gzip"), String.eq(c, "x-gzip")), ce.of(Zlib.gunzip(b)),
    Bool.pick(Dec, String.eq(c, "deflate"), ce.deflate(b, Zlib.unzlib(b)),
      Bool.pick(Dec, String.eq(c, "identity"), DecOk{b}, DecSkip{})))

def ce.go(cs: List<&2, String>, d: Dec) -> Dec:
  match cs:
    case Nil{}:
      d
    case Con{+c, t}:
      match d:
        case DecOk{+b}:
          ce.go(t, ce.apply(c, b))
        case DecBad{}:
          DecBad{}
        case DecSkip{}:
          DecSkip{}

def decoded.pick(d: Dec, +status: U32, headers: Map<&2, List<&2, String>>, body: String) -> Result<&2, &2, Err, Res>:
  match d:
    case DecOk{b}:
      Done{Res{status, headers, b}}
    case DecBad{}:
      Fail{ErrBad{}}
    case DecSkip{}:
      Done{Res{status, headers, body}}

def decoded.go(empty: Bool, +status: U32, +headers: Map<&2, List<&2, String>>, +body: String) -> Result<&2, &2, Err, Res>:
  match empty:
    case True{}:
      Done{Res{status, headers, body}}
    case False{}:
      decoded.pick(ce.go(List.reverse(&2, String, ce.split(fields(headers, "content-encoding"))), DecOk{body}), status, headers, body)

# The body decoded per Content-Encoding; the headers stay as sent. A corrupt body is ErrBad.
def decoded(res: Res) -> Result<&2, &2, Err, Res>:
  Res{+status, +headers, +body} = res
  decoded.go(String.is_empty(body), status, headers, body)

def decoded.m(r: Result<&2, &2, Err, Res>) -> Maybe<&2, Res>:
  match r:
    case Done{x}:
      Some{x}
    case Fail{e}:
      None{}

def decoded.some(res: Res) -> Maybe<&2, Res>:
  decoded.m(decoded(res))

# The final response of a fetch, after redirects.
def fetch.final(r: Result<&2, &2, Err, Res>) -> Result<&2, &2, Err, Res>:
  match r:
    case Done{res}:
      decoded(res)
    case Fail{e}:
      Fail{e}

def pool.final(pr: Pool & Result<&2, &2, Err, Res>) -> Pool & Result<&2, &2, Err, Res>:
  (p, r) = pr
  (p, fetch.final(r))

# pool.how(p, "GET", "https://example.com/x?y=1", headers, body, ms, mode) follows redirects
# on p's idle sockets. Fail is a bad URL, a failed lookup, connect, handshake, send, read,
# a malformed response, more than 20 redirects, or a step past ms. Bodies are bytes.
def pool.how(p: Pool, method: String, url: String, headers: Map<&2, List<&2, String>>, body: String, ms: U32, mode: Mode) -> IO(Pool & Result<&2, &2, Err, Res>):
  do IO<Pool & Result<&2, &2, Err, Res>>:
    pr : Pool & Result<&2, &2, Err, Res> <- pool.hops(mode, ms, (p, fetch.start(method, headers, body, Url.absolute(url))))
    return pool.final(pr)

def pool.fetch.with(p: Pool, method: String, url: String, headers: Map<&2, List<&2, String>>, body: String, ms: U32) -> IO(Pool & Result<&2, &2, Err, Res>):
  pool.how(p, method, url, headers, body, ms, ModeFollow{})

def pool.fetch(p: Pool, method: String, url: String, headers: Map<&2, List<&2, String>>, body: String) -> IO(Pool & Result<&2, &2, Err, Res>):
  pool.fetch.with(p, method, url, headers, body, 30000)

def fetch.end(pr: Pool & Result<&2, &2, Err, Res>) -> IO(Result<&2, &2, Err, Res>):
  (p, r) = pr
  do IO<Result<&2, &2, Err, Res>>:
    pool.close(p)
    return r

# fetch is pool.how on a fresh pool that closes afterwards.
def fetch.how(method: String, url: String, headers: Map<&2, List<&2, String>>, body: String, ms: U32, mode: Mode) -> IO(Result<&2, &2, Err, Res>):
  do IO<Result<&2, &2, Err, Res>>:
    pr : Pool & Result<&2, &2, Err, Res> <- pool.how(pool.new(), method, url, headers, body, ms, mode)
    fetch.end(pr)

def fetch.with(method: String, url: String, headers: Map<&2, List<&2, String>>, body: String, ms: U32) -> IO(Result<&2, &2, Err, Res>):
  fetch.how(method, url, headers, body, ms, ModeFollow{})

# fetch.with and a 30 s step timeout.
def fetch(method: String, url: String, headers: Map<&2, List<&2, String>>, body: String) -> IO(Result<&2, &2, Err, Res>):
  fetch.with(method, url, headers, body, 30000)

def get(url: String) -> IO(Result<&2, &2, Err, Res>):
  fetch("GET", url, empty(), "")

# Streaming (a body read or sent in pieces). Stream bodies are never content-decoded.
# What the next read returns: read more, a piece, the end, or an error.
type Po is Data:
  PoNone{}
  PoPiece{b: String}
  PoEnd{}
  PoBad{e: Err}

type Stream is Type:
  Stream{tls: Bool, s: Socket, ms: U32, res: Res, rb: Rb, out: Po}

type Pc is Data:
  Pc{rb: Rb, out: Po}

def pc.piece(+rb: Rb, +b: String) -> Pc:
  Pc{rb, Bool.pick(Po, String.is_empty(b), PoNone{}, PoPiece{b})}

def pc.fed.of(+b: String, st: Rb) -> Pc:
  match st:
    case RbDone{}:
      Pc{RbDone{}, Bool.pick(Po, String.is_empty(b), PoEnd{}, PoPiece{b})}
    case RbBad{}:
      Pc{RbDone{}, PoBad{ErrBad{}}}
    case RbLen{n}:
      pc.piece(RbLen{n}, b)
    case RbChunk{st}:
      pc.piece(RbChunk{st}, b)
    case RbClose{}:
      pc.piece(RbClose{}, b)

def pc.fed(bf: Bf) -> Pc:
  Bf{st, parts, rest} = bf
  pc.fed.of(rb.body(parts), st)

# The body bytes that just arrived: what they add to the body, and how it ends.
def pc.of(rb: Rb, piece: Bytes.Bytes) -> Pc:
  pc.fed(rb.feed(rb, Nil{}, piece))

# The server closed. Only a close-delimited body may end this way (RFC 9112 §8).
def pc.end(ok: Bool) -> Pc:
  match ok:
    case True{}:
      Pc{RbDone{}, PoEnd{}}
    case False{}:
      Pc{RbDone{}, PoBad{ErrBad{}}}

def pc.eof(st: Rb) -> Pc:
  pc.end(rb.eof(st))

def stream.mk(+tls: Bool, s: Socket, +ms: U32, res: Res, pc: Pc) -> Stream:
  Pc{rb, out} = pc
  Stream{tls, s, ms, res, rb, out}

def stream.got.piece(+tls: Bool, s: Socket, +ms: U32, res: Res, +rb: Rb, p: U32 & Array<U32>) -> Stream:
  (+len, buf) = p
  stream.mk(tls, s, ms, res, Bool.pick(Pc, U32.is_zero(len), pc.eof(rb), pc.of(rb, Bytes.Bytes{len, buf})))

def stream.got(+tls: Bool, +ms: U32, res: Res, +rb: Rb, m: Socket & Result<&1, &1, U32 & String, U32 & Array<U32>>) -> Stream:
  (s, r) = m
  match r:
    case Fail{(+code, why)}:
      Stream{tls, s, ms, res, RbDone{}, PoBad{err.or_late(code, ErrRead{code, why})}}
    case Done{p}:
      stream.got.piece(tls, s, ms, res, rb, p)

def stream.after(+rb: Rb) -> Po:
  match rb:
    case RbDone{}:
      PoEnd{}
    case RbBad{}:
      PoEnd{}
    case RbLen{n}:
      PoNone{}
    case RbChunk{st}:
      PoNone{}
    case RbClose{}:
      PoNone{}

# The next piece of the body; None at the end. A piece is never empty.
@unsafe
def stream.read(st: Stream) -> IO(Stream & Result<&2, &2, Err, Maybe<&2, String>>):
  Stream{+tls, s, +ms, res, +rb, out} = st
  match out:
    case PoPiece{b}:
      IO.pure(Stream & Result<&2, &2, Err, Maybe<&2, String>>, (Stream{tls, s, ms, res, rb, stream.after(rb)}, Done{Some{b}}))
    case PoEnd{}:
      IO.pure(Stream & Result<&2, &2, Err, Maybe<&2, String>>, (Stream{tls, s, ms, res, rb, PoEnd{}}, Done{None{}}))
    case PoBad{+e}:
      IO.pure(Stream & Result<&2, &2, Err, Maybe<&2, String>>, (Stream{tls, s, ms, res, rb, PoBad{e}}, Fail{e}))
    case PoNone{}:
      do IO<Stream & Result<&2, &2, Err, Maybe<&2, String>>>:
        m : Socket & Result<&1, &1, U32 & String, U32 & Array<U32>> <- io.recv.words(tls, s, 65536, ms)
        stream.read(stream.got(tls, ms, res, rb, m))

# bise patch (issue 13): stream.read that waits at most `ms` for the next
# piece and keeps the stream open when nothing came, so a reader can look
# at something else (a cancel) while the server is silent. Done{Some{p}}:
# what stream.read answers; Done{None{}}: `ms` passed with nothing. A
# timed-out read leaves the socket and the TLS session as they were (the
# effect returns before a record is taken), so the next read goes on.
def stream.poll.wrap(r: Stream & Result<&2, &2, Err, Maybe<&2, String>>)
    -> Stream & Result<&2, &2, Err, Maybe<&2, Maybe<&2, String>>>:
  (st, res) = r
  match res:
    case Fail{e}:
      (st, Fail{e})
    case Done{m}:
      (st, Done{Some{m}})

# a read that only timed out is quiet: the stream stays as it was
@unsafe
def stream.poll.fail(quiet: Bool, +tls: Bool, s: Socket, +ms: U32, res: Res, +rb: Rb, +code: U32, why: String) -> IO(Stream & Result<&2, &2, Err, Maybe<&2, Maybe<&2, String>>>):
  match quiet:
    case True{}:
      IO.pure(Stream & Result<&2, &2, Err, Maybe<&2, Maybe<&2, String>>>, (Stream{tls, s, ms, res, rb, PoNone{}}, Done{None{}}))
    case False{}:
      IO.bind(Stream & Result<&2, &2, Err, Maybe<&2, String>>, Stream & Result<&2, &2, Err, Maybe<&2, Maybe<&2, String>>>, stream.read(Stream{tls, s, ms, res, RbDone{}, PoBad{err.or_late(code, ErrRead{code, why})}}), x => IO.pure(Stream & Result<&2, &2, Err, Maybe<&2, Maybe<&2, String>>>, stream.poll.wrap(x)))

@unsafe
def stream.poll.got(+tls: Bool, +ms: U32, res: Res, +rb: Rb, m: Socket & Result<&1, &1, U32 & String, U32 & Array<U32>>) -> IO(Stream & Result<&2, &2, Err, Maybe<&2, Maybe<&2, String>>>):
  (s, r) = m
  match r:
    case Fail{(+code, why)}:
      stream.poll.fail(err.late(code), tls, s, ms, res, rb, code, why)
    case Done{p}:
      IO.bind(Stream & Result<&2, &2, Err, Maybe<&2, String>>, Stream & Result<&2, &2, Err, Maybe<&2, Maybe<&2, String>>>, stream.read(stream.got.piece(tls, s, ms, res, rb, p)), x => IO.pure(Stream & Result<&2, &2, Err, Maybe<&2, Maybe<&2, String>>>, stream.poll.wrap(x)))

@unsafe
def stream.poll(st: Stream, wait: U32) -> IO(Stream & Result<&2, &2, Err, Maybe<&2, Maybe<&2, String>>>):
  Stream{+tls, s, +ms, res, +rb, out} = st
  match out:
    case PoNone{}:
      do IO<Stream & Result<&2, &2, Err, Maybe<&2, Maybe<&2, String>>>>:
        m : Socket & Result<&1, &1, U32 & String, U32 & Array<U32>> <- io.recv.words(tls, s, 65536, wait)
        stream.poll.got(tls, ms, res, rb, m)
    case _:
      IO.bind(Stream & Result<&2, &2, Err, Maybe<&2, String>>, Stream & Result<&2, &2, Err, Maybe<&2, Maybe<&2, String>>>, stream.read(Stream{tls, s, ms, res, rb, out}), x => IO.pure(Stream & Result<&2, &2, Err, Maybe<&2, Maybe<&2, String>>>, stream.poll.wrap(x)))

# Status and headers. Its body is "": read the body with stream.read.
def stream.res(st: Stream) -> Stream & Res:
  Stream{tls, s, ms, +res, rb, out} = st
  (Stream{tls, s, ms, res, rb, out}, res)

def stream.close(st: Stream) -> IO(Unit):
  Stream{tls, s, ms, res, rb, out} = st
  io.close(tls, s)

# Head read state: still scanning, cut short or bad, or the head with the bytes after it.
type Sh is Type:
  ShMore{buf: Bytes.Bytes, k: U32}
  ShBad{err: Maybe<&2, Err>}
  ShRes{res: Res, rb: Rb, rest: Bytes.Bytes}

def sh.of(hs: Hs) -> Sh:
  match hs:
    case HsWait{buf, k}:
      ShMore{buf, k}
    case HsBad{}:
      ShBad{None{}}
    case HsSeek{buf, from, k}:
      ShBad{None{}}
    case HsRes{res, rb, rest}:
      ShRes{res, rb, rest}

def sh.piece(+head: Bool, buf: Bytes.Bytes, +k: U32, p: U32 & Array<U32>) -> Sh:
  (+len, b) = p
  Bool.pick(Sh, U32.is_zero(len), ShBad{None{}}, sh.of(hs.more(head, k, buf, Bytes.Bytes{len, b})))

def sh.next(+head: Bool, buf: Bytes.Bytes, +k: U32, m: Socket & Result<&1, &1, U32 & String, U32 & Array<U32>>) -> Socket & Sh:
  (s, r) = m
  match r:
    case Fail{(+code, why)}:
      (s, ShBad{Some{err.or_late(code, ErrRead{code, why})}})
    case Done{p}:
      (s, sh.piece(head, buf, k, p))

def fetch.fail.of(r: Result<&2, &2, Err, Res>) -> Result<&1, &1, Err, Stream>:
  match r:
    case Fail{e}:
      Fail{e}
    case Done{res}:
      Fail{ErrBad{}}

# Reads until the final head is in; the body bytes that came with it are the first piece.
@unsafe
def stream.head(+tls: Bool, +ms: U32, +head: Bool, st: Socket & Sh) -> IO(Result<&1, &1, Err, Stream>):
  (s, sh) = st
  match sh:
    case ShMore{buf, +k}:
      do IO<Result<&1, &1, Err, Stream>>:
        m : Socket & Result<&1, &1, U32 & String, U32 & Array<U32>> <- io.recv.words(tls, s, 65536, ms)
        stream.head(tls, ms, head, sh.next(head, buf, k, m))
    case ShBad{err}:
      do IO<Result<&1, &1, Err, Stream>>:
        r : Result<&2, &2, Err, Res> <- fetch.bad(tls, s, err)
        return fetch.fail.of(r)
    case ShRes{res, rb, rest}:
      IO.pure(Result<&1, &1, Err, Stream>, Done{stream.mk(tls, s, ms, res, pc.of(rb, rest))})

def stream.sent(+tls: Bool, +ms: U32, +head: Bool, m: Socket & Result<&1, &1, U32 & String, Unit>) -> IO(Result<&1, &1, Err, Stream>):
  (s, r) = m
  match r:
    case Fail{(+code, why)}:
      do IO<Result<&1, &1, Err, Stream>>:
        io.close(tls, s)
        return Fail{err.or_late(code, ErrWrite{code, why})}
    case Done{u}:
      stream.head(tls, ms, head, (s, ShMore{Bytes.new(0), 0}))

def stream.conn(r: Result<&1, &1, Err, Conn>, +ms: U32, +head: Bool, wire: String) -> IO(Result<&1, &1, Err, Stream>):
  match r:
    case Fail{e}:
      IO.pure(Result<&1, &1, Err, Stream>, Fail{e})
    case Done{c}:
      Conn{+tls, s} = c
      do IO<Result<&1, &1, Err, Stream>>:
        sent : Socket & Result<&1, &1, U32 & String, Unit> <- io.send(tls, s, wire)
        stream.sent(tls, ms, head, sent)

def stream.ip(+tls: Bool, +ms: U32, +head: Bool, +host: String, +port: U32, wire: String, ip: Maybe<&2, String>) -> IO(Result<&1, &1, Err, Stream>):
  match ip:
    case None{}:
      IO.pure(Result<&1, &1, Err, Stream>, Fail{ErrDns{}})
    case Some{addr}:
      do IO<Result<&1, &1, Err, Stream>>:
        c : Result<&1, &1, Err, Conn> <- conn.open(tls, ms, host, addr, port)
        stream.conn(c, ms, head, wire)

# One connection per stream; the request says close.
def stream.origin(+ms: U32, +head: Bool, wire: String, a: Url.Abs, known: Bool) -> IO(Result<&1, &1, Err, Stream>):
  Url.Abs{+scheme, +host, +port, target} = a
  match known:
    case False{}:
      IO.pure(Result<&1, &1, Err, Stream>, Fail{ErrUrl{}})
    case True{}:
      do IO<Result<&1, &1, Err, Stream>>:
        ip : Maybe<&2, String> <- Dns.resolve(host)
        stream.ip(String.eq(scheme, "https"), ms, head, host, port, wire, ip)

def stream.one(h: Hop, +ms: U32) -> IO(Result<&1, &1, Err, Stream>):
  Hop{+method, +url, headers, body} = h
  Url.Abs{scheme, host, port, +target} = url
  stream.origin(ms, String.eq(method, "HEAD"), encode_req.on(method, target, Url.host_field(url), headers, body, True{}), url, pool.scheme.ok(url))

# Go: the next request. Drop: close this stream, then go on. Done: the answer.
type Sn is Type:
  SnGo{left: Nat, hop: Hop}
  SnDrop{st: Stream, next: Next}
  SnDone{r: Result<&1, &1, Err, Stream>}

def sn.of(n: Next) -> Sn:
  match n:
    case NGo{l, h}:
      SnGo{l, h}
    case NStop{}:
      SnDone{Fail{ErrRedirect{}}}
    case NDone{m}:
      match m:
        case Fail{e}:
          SnDone{Fail{e}}
        case Done{r}:
          SnDone{Fail{ErrBad{}}}

def stream.decide.n(st: Stream, n: Next) -> Sn:
  match n:
    case NDone{m}:
      SnDone{Done{st}}
    case NGo{l, h}:
      SnDrop{st, NGo{l, h}}
    case NStop{}:
      SnDrop{st, NStop{}}

def stream.decide.of(+left: Nat, +hop: Hop, sr: Stream & Res) -> Sn:
  (st, res) = sr
  stream.decide.n(st, fetch.decide(left, hop, Done{res}))

def stream.decide(+left: Nat, +hop: Hop, r: Result<&1, &1, Err, Stream>) -> Sn:
  match r:
    case Fail{e}:
      SnDone{Fail{e}}
    case Done{st}:
      stream.decide.of(left, hop, stream.res(st))

@unsafe
def stream.hops(+ms: U32, sn: Sn) -> IO(Result<&1, &1, Err, Stream>):
  match sn:
    case SnDone{r}:
      IO.pure(Result<&1, &1, Err, Stream>, r)
    case SnGo{+left, +hop}:
      do IO<Result<&1, &1, Err, Stream>>:
        r : Result<&1, &1, Err, Stream> <- stream.one(hop, ms)
        stream.hops(ms, stream.decide(left, hop, r))
    case SnDrop{st, next}:
      do IO<Result<&1, &1, Err, Stream>>:
        stream.close(st)
        stream.hops(ms, sn.of(next))

def stream.start(method: String, headers: Map<&2, List<&2, String>>, body: String, u: Maybe<&2, Url.Abs>) -> Sn:
  match u:
    case None{}:
      SnDone{Fail{ErrUrl{}}}
    case Some{a}:
      SnGo{20n, Hop{method, a, req.put(Map.to_list(&2, List<&2, String>, headers), empty()), body}}

# Http.open(method, url, headers, body) follows redirects like fetch and returns once the head is in.
# Read the body with stream.read. Stream bodies are sent and returned as is: no Accept-Encoding, no decoding.
def open.with(method: String, url: String, headers: Map<&2, List<&2, String>>, body: String, +ms: U32) -> IO(Result<&1, &1, Err, Stream>):
  stream.hops(ms, stream.start(method, headers, body, Url.absolute(url)))

def open(method: String, url: String, headers: Map<&2, List<&2, String>>, body: String) -> IO(Result<&1, &1, Err, Stream>):
  open.with(method, url, headers, body, 30000)

# Upload: the request body sent in chunks (RFC 9112 §7.1). err: the first failed write.
type Up is Type:
  Up{tls: Bool, s: Socket, ms: U32, head: Bool, err: Maybe<&2, Err>}

def hex.digit(+n: U32) -> Char:
  Chr{Bool.pick(U32, U32.is_lt(n, 10), (48 + n : U32), (87 + n : U32))}

def hex.go(k: Nat, +n: U32, acc: String) -> String:
  match k:
    case 0n:
      acc
    case 1n+p:
      hex.go(p, U32.shrn(n, 4n), SCon{hex.digit(U32.and(n, 15)), acc})

def hex.trim(+s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, +t}:
      Bool.pick(String, Bool.and(U32.is_eq(c, 48), Bool.not(String.is_empty(t))), hex.trim(t), SCon{Chr{c}, t})

def hex(+n: U32) -> String:
  hex.trim(hex.go(8n, n, ""))

def up.sent(+tls: Bool, +ms: U32, +head: Bool, m: Socket & Result<&1, &1, U32 & String, Unit>) -> Up:
  (s, r) = m
  match r:
    case Fail{(+code, why)}:
      Up{tls, s, ms, head, Some{err.or_late(code, ErrWrite{code, why})}}
    case Done{u}:
      Up{tls, s, ms, head, None{}}

def up.conn(r: Result<&1, &1, Err, Conn>, +ms: U32, +head: Bool, wire: String) -> IO(Result<&1, &1, Err, Up>):
  match r:
    case Fail{e}:
      IO.pure(Result<&1, &1, Err, Up>, Fail{e})
    case Done{c}:
      Conn{+tls, s} = c
      do IO<Result<&1, &1, Err, Up>>:
        sent : Socket & Result<&1, &1, U32 & String, Unit> <- io.send(tls, s, wire)
        return Done{up.sent(tls, ms, head, sent)}

def up.ip(+tls: Bool, +ms: U32, +head: Bool, +host: String, +port: U32, wire: String, ip: Maybe<&2, String>) -> IO(Result<&1, &1, Err, Up>):
  match ip:
    case None{}:
      IO.pure(Result<&1, &1, Err, Up>, Fail{ErrDns{}})
    case Some{addr}:
      do IO<Result<&1, &1, Err, Up>>:
        c : Result<&1, &1, Err, Conn> <- conn.open(tls, ms, host, addr, port)
        up.conn(c, ms, head, wire)

def up.head(+method: String, +target: String, +hf: String, +headers: Map<&2, List<&2, String>>) -> String:
  request(method, target, set(set(set(req.put(Map.to_list(&2, List<&2, String>, headers), empty()), "host", hf), "connection", "close"), "transfer-encoding", "chunked"), "")

def up.origin(+method: String, +ms: U32, headers: Map<&2, List<&2, String>>, a: Url.Abs, known: Bool) -> IO(Result<&1, &1, Err, Up>):
  Url.Abs{+scheme, +host, +port, +target} = a
  match known:
    case False{}:
      IO.pure(Result<&1, &1, Err, Up>, Fail{ErrUrl{}})
    case True{}:
      do IO<Result<&1, &1, Err, Up>>:
        ip : Maybe<&2, String> <- Dns.resolve(host)
        up.ip(String.eq(scheme, "https"), ms, String.eq(method, "HEAD"), host, port, up.head(method, target, Url.host_field(a), headers), ip)

def up.url(+method: String, +ms: U32, headers: Map<&2, List<&2, String>>, u: Maybe<&2, Url.Abs>) -> IO(Result<&1, &1, Err, Up>):
  match u:
    case None{}:
      IO.pure(Result<&1, &1, Err, Up>, Fail{ErrUrl{}})
    case Some{+a}:
      up.origin(method, ms, headers, a, pool.scheme.ok(a))

# Http.upload(method, url, headers) sends the head. A streamed body cannot be replayed, so redirects are not followed.
def upload.with(+method: String, url: String, headers: Map<&2, List<&2, String>>, +ms: U32) -> IO(Result<&1, &1, Err, Up>):
  up.url(method, ms, headers, Url.absolute(url))

def upload(+method: String, url: String, headers: Map<&2, List<&2, String>>) -> IO(Result<&1, &1, Err, Up>):
  upload.with(method, url, headers, 30000)

def up.wrote(+tls: Bool, +ms: U32, +head: Bool, m: Socket & Result<&1, &1, U32 & String, Unit>) -> Up & Result<&2, &2, Err, Unit>:
  (s, r) = m
  match r:
    case Fail{(+code, why)}:
      +e = err.or_late(code, ErrWrite{code, why})
      (Up{tls, s, ms, head, Some{e}}, Fail{e})
    case Done{u}:
      (Up{tls, s, ms, head, None{}}, Done{Unit{}})

def up.put(+tls: Bool, s: Socket, +ms: U32, +head: Bool, +piece: String, skip: Bool) -> IO(Up & Result<&2, &2, Err, Unit>):
  match skip:
    case True{}:
      IO.pure(Up & Result<&2, &2, Err, Unit>, (Up{tls, s, ms, head, None{}}, Done{Unit{}}))
    case False{}:
      do IO<Up & Result<&2, &2, Err, Unit>>:
        sent : Socket & Result<&1, &1, U32 & String, Unit> <- io.send(tls, s, hex(nat_u32(String.length(piece))) ++ "\r\n" ++ piece ++ "\r\n")
        return up.wrote(tls, ms, head, sent)

# One chunk. An empty piece sends nothing, since an empty chunk would end the body.
def upload.write(up: Up, +piece: String) -> IO(Up & Result<&2, &2, Err, Unit>):
  Up{+tls, s, +ms, +head, err} = up
  match err:
    case Some{+e}:
      IO.pure(Up & Result<&2, &2, Err, Unit>, (Up{tls, s, ms, head, Some{e}}, Fail{e}))
    case None{}:
      up.put(tls, s, ms, head, piece, String.is_empty(piece))

# Ends the body and returns the response as a stream.
def upload.finish(up: Up) -> IO(Result<&1, &1, Err, Stream>):
  Up{+tls, s, +ms, +head, err} = up
  match err:
    case Some{e}:
      do IO<Result<&1, &1, Err, Stream>>:
        io.close(tls, s)
        return Fail{e}
    case None{}:
      do IO<Result<&1, &1, Err, Stream>>:
        sent : Socket & Result<&1, &1, U32 & String, Unit> <- io.send(tls, s, "0\r\n\r\n")
        stream.sent(tls, ms, head, sent)

def reply.bytes(res: Res, head: Bool, close: Bool) -> String:
  Res{+status, +headers, +body} = res
  encode.on(Res{status, Bool.pick(Map<&2, List<&2, String>>, close, set(headers, "connection", "close"), headers), body}, head)

def reply.fail.bytes(+status: U32) -> Res:
  Res{status, empty(), reason(status)}

def reply.sent.close(m: Socket & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (s, r) = m
  Socket.close(s)

def reply_fail(s: Socket, +status: U32) -> IO(Unit):
  do IO<Unit>:
    sent : Socket & Result<&1, &1, U32 & String, Unit> <- Wire.send(s, encode(reply.fail.bytes(status)))
    reply.sent.close(sent)


# The head is parsed once, when it is whole. Body pieces are kept as they come and joined once.
type Sv is Type:
  SvHead{buf: Bytes.Bytes}
  SvLen{req: Req, v11: Bool, left: U32, parts: List<&1, Bytes.Bytes>}
  SvChunk{req: Req, v11: Bool, n: U32, st: Dc, parts: List<&1, Bytes.Bytes>}

# More: read on, first answering 100 Continue when cont is set.
type Turn is Type:
  TurnClose{}
  TurnFail{status: U32}
  TurnReq{req: Req, rest: Bytes.Bytes, v11: Bool}
  TurnMore{sv: Sv, cont: Bool}

def serve.max() -> Nat:
  fetch.max()

def sv.none() -> Sv:
  SvHead{Bytes.new(0)}

def sv.body(req: Req, +v11: Bool, parts: List<&1, Bytes.Bytes>, rest: Bytes.Bytes) -> Turn:
  Req{method, path, headers, body} = req
  TurnReq{Req{method, path, headers, Bytes.to_string(Bytes.concat(List.reverse(&1, Bytes.Bytes, parts)))}, rest, v11}

def sv.len.cut(req: Req, +v11: Bool, parts: List<&1, Bytes.Bytes>, +len: U32, +left: U32, r: Bytes.Bytes & Bytes.Bytes) -> Turn:
  (piece, last) = r
  sv.body(req, v11, Con{last, parts}, bytes.snd(Bytes.slice(piece, left, (len - left : U32))))

def sv.len.have(short: Bool, req: Req, +v11: Bool, +left: U32, parts: List<&1, Bytes.Bytes>, +len: U32, piece: Bytes.Bytes) -> Turn:
  match short:
    case True{}:
      TurnMore{SvLen{req, v11, (left - len : U32), Con{piece, parts}}, False{}}
    case False{}:
      sv.len.cut(req, v11, parts, len, left, Bytes.slice(piece, 0, left))

def sv.len(req: Req, +v11: Bool, +left: U32, parts: List<&1, Bytes.Bytes>, piece: Bytes.Bytes) -> Turn:
  Bytes.Bytes{+len, buf} = piece
  sv.len.have(U32.is_lt(len, left), req, v11, left, parts, len, Bytes.Bytes{len, buf})

def sv.chunk.of(req: Req, +v11: Bool, +n: U32, c: Ck) -> Turn:
  Ck{st, parts, rest} = c
  match st:
    case DcDone{}:
      sv.body(req, v11, parts, rest)
    case DcBad{}:
      TurnFail{400}
    case DcSize{a, b}:
      TurnMore{SvChunk{req, v11, n, DcSize{a, b}, parts}, False{}}
    case DcExt{x}:
      TurnMore{SvChunk{req, v11, n, DcExt{x}, parts}, False{}}
    case DcSizeLf{x}:
      TurnMore{SvChunk{req, v11, n, DcSizeLf{x}, parts}, False{}}
    case DcData{k}:
      TurnMore{SvChunk{req, v11, n, DcData{k}, parts}, False{}}
    case DcDataCr{}:
      TurnMore{SvChunk{req, v11, n, DcDataCr{}, parts}, False{}}
    case DcDataLf{}:
      TurnMore{SvChunk{req, v11, n, DcDataLf{}, parts}, False{}}
    case DcTr{j}:
      TurnMore{SvChunk{req, v11, n, DcTr{j}, parts}, False{}}

def sv.chunk.cap(big: Bool, req: Req, +v11: Bool, +n: U32, st: Dc, parts: List<&1, Bytes.Bytes>, piece: Bytes.Bytes) -> Turn:
  match big:
    case True{}:
      TurnFail{413}
    case False{}:
      sv.chunk.of(req, v11, n, ck(piece, st, parts))

def sv.chunk(+max: Nat, req: Req, +v11: Bool, +n: U32, st: Dc, parts: List<&1, Bytes.Bytes>, piece: Bytes.Bytes) -> Turn:
  Bytes.Bytes{+len, buf} = piece
  +n2 = (n + len : U32)
  sv.chunk.cap(Bool.or(U32.is_lt(n2, n), Nat.is_lt(max, U32.to_nat(n2))), req, v11, n2, st, parts, Bytes.Bytes{len, buf})

# RFC 9110 §10.1.1: answer 100 Continue only when the body is still to come.
def sv.cont(want: Bool, t: Turn) -> Turn:
  match t:
    case TurnMore{sv, c}:
      TurnMore{sv, want}
    case TurnClose{}:
      TurnClose{}
    case TurnFail{s}:
      TurnFail{s}
    case TurnReq{req, rest, v11}:
      TurnReq{req, rest, v11}

def sv.cl.k(big: Bool, req: Req, +v11: Bool, +k: U32, rest: Bytes.Bytes) -> Turn:
  match big:
    case True{}:
      TurnFail{413}
    case False{}:
      sv.len(req, v11, k, Nil{}, rest)

def sv.cl(+max: Nat, req: Req, +v11: Bool, rest: Bytes.Bytes, n: Maybe<&2, U32>) -> Turn:
  match n:
    case None{}:
      TurnFail{400}
    case Some{+k}:
      sv.cl.k(Nat.is_lt(max, U32.to_nat(k)), req, v11, k, rest)

def sv.frame(+max: Nat, req: Req, +v11: Bool, rest: Bytes.Bytes, te: Bool) -> Turn:
  Req{method, path, +headers, body} = req
  match te:
    case True{}:
      sv.chunk(max, Req{method, path, headers, body}, v11, 0, dc.start(), Nil{}, rest)
    case False{}:
      sv.cl(max, Req{method, path, headers, body}, v11, rest, parse_u32(header(headers, "content-length")))

def sv.expects(+v11: Bool, req: Req) -> Bool:
  Req{method, path, +headers, body} = req
  Bool.and(v11, String.eq(String.to_lower(header(headers, "expect")), "100-continue"))

def sv.te(req: Req) -> Bool:
  Req{method, path, +headers, body} = req
  has_header(headers, "transfer-encoding")

def sv.parsed(+max: Nat, +v11: Bool, rest: Bytes.Bytes, g: Got) -> Turn:
  match g:
    case GotBad{}:
      TurnFail{400}
    case GotMore{}:
      TurnFail{400}
    case GotReq{req}:
      TurnReq{req, rest, v11}
    case GotHead{+req}:
      sv.cont(sv.expects(v11, req), sv.frame(max, req, v11, rest, sv.te(req)))

def sv.split(+max: Nat, +hlen: U32, +len: U32, r: Bytes.Bytes & Bytes.Bytes) -> Turn:
  (buf, hb) = r
  +head = Bytes.to_string(hb)
  sv.parsed(max, req.v11(head), bytes.snd(Bytes.slice(buf, hlen, (len - hlen : U32))), parse.got(head))

def sv.hit.none(big: Bool, buf: Bytes.Bytes) -> Turn:
  match big:
    case True{}:
      TurnFail{431}
    case False{}:
      TurnMore{SvHead{buf}, False{}}

def sv.hit.some(big: Bool, +max: Nat, +hlen: U32, +len: U32, buf: Bytes.Bytes) -> Turn:
  match big:
    case True{}:
      TurnFail{431}
    case False{}:
      sv.split(max, hlen, len, Bytes.slice(buf, 0, hlen))

# RFC 6585 §5: a head over 64 KiB is refused.
def sv.hit(+max: Nat, +len: U32, buf: Bytes.Bytes, at: Maybe<&2, U32>) -> Turn:
  match at:
    case None{}:
      sv.hit.none(U32.is_lt(65536, len), buf)
    case Some{+i}:
      +hlen = (i + 4 : U32)
      sv.hit.some(U32.is_lt(65536, hlen), max, hlen, len, buf)

def sv.seek.of(+max: Nat, +len: U32, +from: U32, buf: Bytes.Bytes, r: Bytes.Bytes & Maybe<&2, U32>) -> Turn:
  (tail, m) = r
  sv.hit(max, len, buf, found.at(from, m))

def sv.seek(+max: Nat, +len: U32, +from: U32, r: Bytes.Bytes & Bytes.Bytes) -> Turn:
  (buf, tail) = r
  sv.seek.of(max, len, from, buf, Bytes.find(tail, "\r\n\r\n"))

# Only the new bytes, and the 3 before them, can finish the blank line.
def sv.head.at(+max: Nat, +from: U32, buf: Bytes.Bytes) -> Turn:
  Bytes.Bytes{+len, b} = buf
  sv.seek(max, len, from, Bytes.slice(Bytes.Bytes{len, b}, from, (len - from : U32)))

def sv.head(+max: Nat, buf: Bytes.Bytes, piece: Bytes.Bytes) -> Turn:
  Bytes.Bytes{+ol, ob} = buf
  sv.head.at(max, Bool.pick(U32, U32.is_lt(ol, 3), 0, (ol - 3 : U32)), Bytes.append(Bytes.Bytes{ol, ob}, piece))

def sv.closed.pick(empty: Bool) -> Turn:
  match empty:
    case True{}:
      TurnClose{}
    case False{}:
      TurnFail{400}

def sv.closed.head(buf: Bytes.Bytes) -> Turn:
  Bytes.Bytes{+len, b} = buf
  sv.closed.pick(U32.is_eq(len, 0))

# A close mid-request is a request cut short (RFC 9112 §8).
def sv.step(closed: Bool, +max: Nat, sv: Sv, piece: Bytes.Bytes) -> Turn:
  match closed:
    case True{}:
      match sv:
        case SvHead{buf}:
          sv.closed.head(buf)
        case SvLen{req, v11, left, parts}:
          TurnFail{400}
        case SvChunk{req, v11, n, st, parts}:
          TurnFail{400}
    case False{}:
      match sv:
        case SvHead{buf}:
          sv.head(max, buf, piece)
        case SvLen{req, +v11, +left, parts}:
          sv.len(req, v11, left, parts, piece)
        case SvChunk{req, +v11, +n, st, parts}:
          sv.chunk(max, req, v11, n, st, parts, piece)

def sv.frame.req(t: Turn) -> Maybe<&1, String & String>:
  match t:
    case TurnReq{req, rest, v11}:
      Req{method, path, headers, body} = req
      Some{(body, Bytes.to_string(rest))}
    case TurnMore{sv, c}:
      None{}
    case TurnFail{s}:
      None{}
    case TurnClose{}:
      None{}

def sv.frame.go(xs: List<&2, String>, t: Turn) -> Maybe<&1, String & String>:
  match xs:
    case Nil{}:
      sv.frame.req(t)
    case Con{+x, rest}:
      match t:
        case TurnMore{sv, c}:
          sv.frame.go(rest, sv.step(False{}, serve.max(), sv, Bytes.from_string(x)))
        case TurnReq{req, r, v11}:
          sv.frame.req(TurnReq{req, r, v11})
        case TurnFail{s}:
          None{}
        case TurnClose{}:
          None{}

# The body and the bytes after it, once the pieces make a whole request; None otherwise.
def serve.frame(xs: List<&2, String>) -> Maybe<&1, String & String>:
  sv.frame.go(xs, TurnMore{sv.none(), False{}})

def talk.piece(+max: Nat, sv: Sv, p: U32 & Array<U32>) -> Turn:
  (+len, buf) = p
  sv.step(U32.is_eq(len, 0), max, sv, Bytes.Bytes{len, buf})

def talk.next(+max: Nat, sv: Sv, m: Socket & Result<&1, &1, U32 & String, U32 & Array<U32>>) -> Socket & Turn:
  (s, r) = m
  match r:
    case Fail{e}:
      (s, TurnClose{})
    case Done{p}:
      (s, talk.piece(max, sv, p))

def talk.keep(+headers: Map<&2, List<&2, String>>, v11: Bool, +res: Res) -> Bool:
  Res{+status, +rh, body} = res
  serve.keep(headers, v11, status, rh)

def talk.after.pick(empty: Bool, +max: Nat, rest: Bytes.Bytes) -> Turn:
  match empty:
    case True{}:
      TurnMore{sv.none(), False{}}
    case False{}:
      sv.step(False{}, max, sv.none(), rest)

def talk.after.rest(+max: Nat, rest: Bytes.Bytes, s: Socket) -> Socket & Turn:
  Bytes.Bytes{+len, buf} = rest
  (s, talk.after.pick(U32.is_eq(len, 0), max, Bytes.Bytes{len, buf}))

def talk.after.keep(keep: Bool, +max: Nat, rest: Bytes.Bytes, s: Socket) -> Socket & Turn:
  match keep:
    case False{}:
      (s, TurnClose{})
    case True{}:
      talk.after.rest(max, rest, s)

def talk.after(+max: Nat, keep: Bool, rest: Bytes.Bytes, m: Socket & Result<&1, &1, U32 & String, Unit>) -> Socket & Turn:
  (s, r) = m
  match r:
    case Fail{e}:
      (s, TurnClose{})
    case Done{u}:
      talk.after.keep(keep, max, rest, s)

def talk.send(+max: Nat, s: Socket, rest: Bytes.Bytes, +headers: Map<&2, List<&2, String>>, v11: Bool, +head: Bool, +res: Res) -> IO(Socket & Turn):
  +keep = talk.keep(headers, v11, res)
  do IO<Socket & Turn>:
    sent : Socket & Result<&1, &1, U32 & String, Unit> <- Wire.send(s, reply.bytes(res, head, Bool.not(keep)))
    return talk.after(max, keep, rest, sent)

def talk.cont.sent(m: Socket & Result<&1, &1, U32 & String, Unit>) -> Socket:
  (s, r) = m
  s

def talk.cont(cont: Bool, s: Socket) -> IO(Socket):
  match cont:
    case False{}:
      IO.pure(Socket, s)
    case True{}:
      do IO<Socket>:
        sent : Socket & Result<&1, &1, U32 & String, Unit> <- Wire.send(s, "HTTP/1.1 100 Continue\r\n\r\n")
        return talk.cont.sent(sent)

@unsafe
def talk(~h: Req -> IO(Res), +max: Nat, st: Socket & Turn) -> IO(Unit):
  (s, t) = st
  match t:
    case TurnMore{sv, cont}:
      do IO<Unit>:
        s2 : Socket <- talk.cont(cont, s)
        m : Socket & Result<&1, &1, U32 & String, U32 & Array<U32>> <- Wire.recv.words(s2, 65536, 30000)
        talk(~h, max, talk.next(max, sv, m))
    case TurnReq{req, rest, v11}:
      Req{+method, path, +headers, body} = req
      do IO<Unit>:
        res : Res <- h(Req{method, path, headers, body})
        next : Socket & Turn <- talk.send(max, s, rest, headers, v11, String.eq(method, "HEAD"), res)
        talk(~h, max, next)
    case TurnFail{status}:
      reply_fail(s, status)
    case TurnClose{}:
      Socket.close(s)

def conn(~h: Req -> IO(Res), +max: Nat, m: Listener & Result<&1, &1, U32 & String, Socket>) -> IO(Listener):
  (l, r) = m
  do IO<Listener>:
    s : Socket <- IO.pass(Socket, r)
    IO.spawn(Unit, talk(~h, max, (s, TurnMore{sv.none(), False{}})))
    return l

@unsafe
def loop(~h: Req -> IO(Res), +max: Nat, l: Listener) -> IO(Unit):
  do IO<Unit>:
    m  : Listener & Result<&1, &1, U32 & String, Socket> <- TCP.accept(l)
    l2 : Listener <- conn(~h, max, m)
    loop(~h, max, l2)

def serve.with(~h: Req -> IO(Res), +port: U32, +max: Nat) -> IO(Unit):
  do IO<Unit>:
    l : Listener <- IO.try(Listener, TCP.listen("127.0.0.1", port))
    IO.print("http://127.0.0.1:" ++ U32.show(port))
    loop(~h, max, l)

def serve(~h: Req -> IO(Res), +port: U32) -> IO(Unit):
  serve.with(~h, port, serve.max())
