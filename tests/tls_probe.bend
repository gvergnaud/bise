# tests/tls_trust.py's probe: argv <ip> <port> <host>. One HTTPS GET of /
# to host at ip:port through the runtime's TLS effects (Wire.tls.*), then
# one line: "ok <the reply's status line>", or "fail <step> <code> <why>".
import Base
import ../bend/vendor/http/wire.bend as Wire

def say(+step: String, +code: U32, why: String) -> String:
  String.concat(["fail ", step, " ", U32.show(code), " ", why])

def first(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case Con{h, _}:
      h

# one read: the status line comes in the reply's first record
def got(m: Socket & Result<&1, &1, U32 & String, String>) -> IO(Unit):
  (s, r) = m
  match r:
    case Fail{(+code, why)}:
      do IO<Unit>:
        Wire.tls.close(s)
        IO.print(say("recv", code, why))
    case Done{+text}:
      do IO<Unit>:
        Wire.tls.close(s)
        IO.print(String.concat(["ok ", first(String.split(text, '\r'))]))

def sent(m: Socket & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (s, r) = m
  match r:
    case Fail{(+code, why)}:
      do IO<Unit>:
        Wire.tls.close(s)
        IO.print(say("send", code, why))
    case Done{u}:
      do IO<Unit>:
        g : Socket & Result<&1, &1, U32 & String, String> <- Wire.tls.recv(s, 65536, 10000)
        got(g)

def shook(+host: String, m: Socket & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (s, r) = m
  match r:
    case Fail{(+code, why)}:
      do IO<Unit>:
        Wire.tls.close(s)
        IO.print(say("tls", code, why))
    case Done{u}:
      do IO<Unit>:
        w : Socket & Result<&1, &1, U32 & String, Unit> <- Wire.tls.send(s,
          String.concat(["GET / HTTP/1.1\r\nHost: ", host, "\r\nConnection: close\r\n\r\n"]))
        sent(w)

def made(+host: String, c: Result<&1, &1, U32 & String, Socket>) -> IO(Unit):
  match c:
    case Fail{(+code, why)}:
      IO.print(say("connect", code, why))
    case Done{s}:
      do IO<Unit>:
        hs : Socket & Result<&1, &1, U32 & String, Unit> <- Wire.tls.connect(s, host, 10000)
        shook(host, hs)

def go(xs: List<String>) -> IO(Unit):
  match xs:
    case Con{_, Con{+ip, Con{+port, Con{+host, Nil{}}}}}:
      do IO<Unit>:
        c : Result<&1, &1, U32 & String, Socket> <- Wire.connect(ip, Maybe.default(&2, U32, U32.read(port), 0), 10000)
        made(host, c)
    case _:
      IO.print("usage: tls_probe <ip> <port> <host>")

def main() -> IO(Unit):
  do IO<Unit>:
    xs : List<String> <- IO.args()
    go(xs)
