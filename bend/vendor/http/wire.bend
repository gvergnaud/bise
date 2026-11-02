# Byte-exact TCP, UDP, and TLS sockets.
import Base

# Socket IO on octets: one Char per byte (0..255). Base's TCP/UDP effects
# decode UTF-8, which breaks byte counts and binary protocols.
# ms arguments are deadlines in milliseconds (0: none); a miss is ETIMEDOUT.
#   import ./wire.bend as Wire

# TCP connect to an IPv4 address, with a deadline.
def connect(host: String, port: U32, ms: U32) ->
  IO(Result<&1, &1, U32 & String, Socket>):
  import "./effs/wire.c"
  import "./effs/wire.js"

# The addresses the OS resolver (getaddrinfo) gives for host: what curl and
# every other program see, /etc/hosts, CNAMEs and split DNS included. IPv4
# and IPv6 as text, in its order, never empty. Fail codes: 1 the name has
# no address, 2 the resolver did not answer, 3 another failure.
def lookup(host: String) ->
  IO(Result<&1, &1, U32 & String, List<&2, String>>):
  import "./effs/wire.c"
  import "./effs/wire.js"

def recv(sock: Socket, max: U32, ms: U32) ->
  IO(Socket & Result<&1, &1, U32 & String, String>):
  import "./effs/wire.c"
  import "./effs/wire.js"

def send(sock: Socket, data: String) ->
  IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effs/wire.c"
  import "./effs/wire.js"

def recv_from(sock: Socket, max: U32, ms: U32) ->
  IO(Socket & Result<&1, &1, U32 & String, String & U32 & String>):
  import "./effs/wire.c"
  import "./effs/wire.js"

def send_to(sock: Socket, host: String, port: U32, data: String) ->
  IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effs/wire.c"
  import "./effs/wire.js"

# TLS over a connected TCP socket: macOS's own TLS (Security.framework), or
# OpenSSL 3 on Linux and under BEND_LIBSSL, loaded at run time (effs/wire.c).
# The certificate chain and host name are always checked; BEND_TLS_CA adds
# trust anchors (a PEM file).
def tls.connect(sock: Socket, host: String, ms: U32) ->
  IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effs/wire.c"
  import "./effs/wire.js"

def tls.send(sock: Socket, data: String) ->
  IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effs/wire.c"
  import "./effs/wire.js"

# The bytes of the file at path, read at send time into a C buffer: they
# never enter the Bend heap (a request's images, http.bend file_part). The
# file must hold exactly size bytes, else EINVAL; a missing one: its errno.
def send.file(sock: Socket, path: String, size: U32) ->
  IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effs/wire.c"
  import "./effs/wire.js"

def tls.send.file(sock: Socket, path: String, size: U32) ->
  IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effs/wire.c"
  import "./effs/wire.js"

# "" once the peer sends close_notify. A bare EOF is an error.
def tls.recv(sock: Socket, max: U32, ms: U32) ->
  IO(Socket & Result<&1, &1, U32 & String, String>):
  import "./effs/wire.c"
  import "./effs/wire.js"

# Ends the TLS session and closes the socket.
def tls.close(sock: Socket) -> IO(Unit):
  import "./effs/wire.c"
  import "./effs/wire.js"

# The .words forms move (len, words): bytes packed four to a U32, laid out as
# bytes/bytes.bend packs them, so no String cell is made per byte.
def recv.words(sock: Socket, max: U32, ms: U32) ->
  IO(Socket & Result<&1, &1, U32 & String, U32 & Array<U32>>):
  import "./effs/wire.c"
  import "./effs/wire.js"

# len past the end of words fails with EINVAL.
def send.words(sock: Socket, len: U32, words: Array<U32>) ->
  IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effs/wire.c"
  import "./effs/wire.js"

# len 0 once the peer sends close_notify.
def tls.recv.words(sock: Socket, max: U32, ms: U32) ->
  IO(Socket & Result<&1, &1, U32 & String, U32 & Array<U32>>):
  import "./effs/wire.c"
  import "./effs/wire.js"

def tls.send.words(sock: Socket, len: U32, words: Array<U32>) ->
  IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effs/wire.c"
  import "./effs/wire.js"
