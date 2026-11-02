// Wire
// ====
// JS twins of wire.c: one char code per octet in, each char's low byte out.

function wire_text(b, n) {
  let s = "";
  for (let i = 0; i < n; i += 8192) {
    s += String.fromCharCode.apply(null, b.subarray(i, Math.min(i + 8192, n)));
  }
  return s;
}

// A char above 255 is not an octet; the caller gets EINVAL.
function wire_octets(data) {
  const b = new Uint8Array(data.length);
  for (let i = 0; i < data.length; i += 1) {
    const c = data.charCodeAt(i);
    if (c > 255) {
      return null;
    }
    b[i] = c;
  }
  return b;
}

// Twins of wire_words and wire_words_octets in wire.c: a Tuple (n, words), with
// byte i in bits 8*(i%4) of word i/4 and 2^d words, the fewest that hold n.
function wire_words(b, n) {
  const w = Math.ceil(n / 4);
  let size = 1;
  while (size < w) {
    size *= 2;
  }
  const a = Array(size).fill(0);
  for (let i = 0; i < n; i += 1) {
    a[i >> 2] = (a[i >> 2] | (b[i] << (8 * (i & 3)))) >>> 0;
  }
  return { $: "Tuple", fst: n, snd: a };
}

// null when n runs past the words.
function wire_words_octets(n, a) {
  n = Number(n);
  if (n > 4 * a.length) {
    return null;
  }
  const b = new Uint8Array(n);
  for (let i = 0; i < n; i += 1) {
    b[i] = (a[i >> 2] >>> (8 * (i & 3))) & 255;
  }
  return b;
}

// A deadline in performance.now() ms, or undefined for none.
function wire_deadline(ms) {
  return Number(ms) ? performance.now() + Number(ms) : undefined;
}

function wire_late(at) {
  return at !== undefined && performance.now() >= at;
}

function wire_timedout() {
  return io_sys().mac ? 60 : 110;
}

function recv(socket, max, ms, k) {
  return wire_recv(socket, max, ms, k, wire_text);
}

function recv_words(socket, max, ms, k) {
  return wire_recv(socket, max, ms, k, wire_words);
}

function wire_recv(socket, max, ms, k, out) {
  const sys = io_sys();
  const fd = socket;
  const b = new Uint8Array(Math.max(Number(max), 1));
  const again = sys.mac ? 35 : 11;
  const at = wire_deadline(ms);
  const go = () => {
    const n = Number(sys.recv(fd, sys.ptr(b), Number(max), 0));
    if (n < 0) {
      const code = sys.errno();
      if (code === again) {
        if (wire_late(at)) {
          return io_tup(socket, io_fail(wire_timedout()));
        }
        io_park_on(fd, false, k, go, at);
        return undefined;
      }
      return io_tup(socket, io_fail(code));
    }
    return io_tup(socket, io_done(out(b, n)));
  };
  return go();
}


function send(socket, data, k) {
  return wire_send(socket, wire_octets(data), k);
}

function send_words(socket, n, words, k) {
  return wire_send(socket, wire_words_octets(n, words), k);
}

// Twin of wire_file in wire.c: the file at path (one char code per octet),
// exactly size bytes of it, or null (the send fails with EINVAL).
function wire_file(path, size) {
  try {
    const b = new Uint8Array(require("fs").readFileSync(Buffer.from(path, "latin1")));
    return b.length === Number(size) ? b : null;
  } catch {
    return null;
  }
}

function send_file(socket, path, size, k) {
  return wire_send(socket, wire_file(path, size), k);
}

function wire_send(socket, b, k) {
  const sys = io_sys();
  const fd = socket;
  if (b === null) {
    return io_tup(socket, io_fail(22));
  }
  const again = sys.mac ? 35 : 11;
  const go = (at) => {
    while (at < b.length) {
      const part = b.subarray(at);
      const n = Number(sys.send(fd, sys.ptr(part), part.length, 0));
      if (n < 0) {
        const code = sys.errno();
        if (code === again) {
          io_park_on(fd, true, k, () => go(at));
          return undefined;
        }
        return io_tup(socket, io_fail(code));
      }
      at += n;
    }
    return io_tup(socket, io_done({ $: "Unit" }));
  };
  return go(0);
}

function recv_from(socket, max, ms, k) {
  const sys = io_sys();
  const fd = socket;
  const b = new Uint8Array(Math.max(Number(max), 1));
  const peer = new Uint8Array(16);
  const len = new Uint32Array([16]);
  const deadline = wire_deadline(ms);
  const go = () => {
    const n = Number(sys.recvfrom(fd, sys.ptr(b), Number(max), 0, sys.ptr(peer),
      sys.ptr(len)));
    if (n < 0) {
      const code = sys.errno();
      if (code === (sys.mac ? 35 : 11)) {
        if (wire_late(deadline)) {
          return io_tup(socket, io_fail(wire_timedout()));
        }
        io_park_on(fd, false, k, go, deadline);
        return undefined;
      }
      return io_tup(socket, io_fail(code));
    }
    const host = peer[4] + "." + peer[5] + "." + peer[6] + "." + peer[7];
    const port = (peer[2] << 8) | peer[3];
    return io_tup(socket, io_done(io_tup(host, port, wire_text(b, n))));
  };
  return go();
}


function send_to(socket, host, port, data, k) {
  const sys = io_sys();
  const fd = socket;
  const at = io_addr(host, Number(port));
  const b = wire_octets(data);
  if (at === null || b === null) {
    return io_tup(socket, io_fail(22));
  }
  const go = () => {
    const sent = sys.sendto(fd, sys.ptr(b), b.length, 0, sys.ptr(at), 16);
    if (Number(sent) < 0) {
      const code = sys.errno();
      if (code === (sys.mac ? 35 : 11)) {
        io_park_on(fd, true, k, go);
        return undefined;
      }
      return io_tup(socket, io_fail(code));
    }
    return io_tup(socket, io_done({ $: "Unit" }));
  };
  return go();
}

// TLS
// ===
// Twins of the TLS effects in wire.c: OpenSSL 3 through bun:ffi, the SSL
// object per fd in a Map, peer verification always on, TLS 1.2 floor.
// OpenSSL only, on every OS: no product ships Bend's JS target (releases
// ship the C-built repl-live and repl-scripted, whose macOS TLS is the
// system's), so the Homebrew paths below are a dev's convenience.

function wire_tls() {
  if (globalThis.BEND_TLS !== undefined) {
    return globalThis.BEND_TLS;
  }
  globalThis.BEND_TLS = null;
  const ffi = require("bun:ffi");
  const paths = [process.env.BEND_LIBSSL,
    "/opt/homebrew/opt/openssl@3/lib/libssl.3.dylib",
    "/usr/local/opt/openssl@3/lib/libssl.3.dylib", "libssl.3.dylib", "libssl.so.3"];
  const T = { i: "i32", l: "i64", U: "u64", p: "ptr", c: "cstring", v: "void" };
  const syms = Object.fromEntries(("TLS_client_method:>p SSL_CTX_new:p>p"
    + " SSL_CTX_set_default_verify_paths:p>i SSL_CTX_set_verify:pip>v"
    + " SSL_CTX_ctrl:pilp>l SSL_new:p>p SSL_set_fd:pi>i"
    + " SSL_ctrl:pilp>l SSL_set1_host:pp>i SSL_connect:p>i SSL_read:ppi>i"
    + " SSL_write:ppi>i SSL_get_error:pi>i SSL_shutdown:p>i SSL_free:p>v"
    + " SSL_get_verify_result:p>l X509_verify_cert_error_string:l>c").split(" ").map((s) => {
    const [name, args, ret] = s.split(/[:>]/);
    return [name, { args: [...args].map((a) => T[a]), returns: T[ret] }];
  }));
  for (const p of paths) {
    if (!p) {
      continue;
    }
    try {
      const s = ffi.dlopen(p, syms).symbols;
      const ctx = s.SSL_CTX_new(s.TLS_client_method());
      if (!ctx || s.SSL_CTX_set_default_verify_paths(ctx) !== 1) {
        return null;
      }
      s.SSL_CTX_set_verify(ctx, 1, null);
      s.SSL_CTX_ctrl(ctx, 123, 0x0303n, null);
      // A bare EOF is an error. close_notify is the only clean close.
      globalThis.BEND_TLS = { s, ctx, ffi, by: new Map() };
      return globalThis.BEND_TLS;
    } catch {
      continue;
    }
  }
  return null;
}

function wire_cstr(t, text) {
  const b = Buffer.from(text + "\0", "utf8");
  return { b, p: t.ffi.ptr(b) };
}

function tls_connect(socket, host, ms, k) {
  const at = wire_deadline(ms);
  const t = wire_tls();
  if (t === null) {
    return io_tup(socket, { $: "Fail", error: io_tup(2,
      "TLS needs OpenSSL 3 (libssl.3); set BEND_LIBSSL to its path") });
  }
  const s = t.s;
  const fd = socket;
  const ssl = s.SSL_new(t.ctx);
  if (!ssl) {
    return io_tup(socket, io_fail(12));
  }
  t.by.set(fd, ssl);
  const name = wire_cstr(t, host);
  const fail = (why) => {
    s.SSL_free(ssl);
    t.by.delete(fd);
    return io_tup(socket, { $: "Fail", error: io_tup(100, why) });
  };
  if (s.SSL_set_fd(ssl, fd) !== 1 || Number(s.SSL_ctrl(ssl, 55, 0n, name.p)) !== 1
    || s.SSL_set1_host(ssl, name.p) !== 1) {
    return fail("TLS setup failed");
  }
  const go = () => {
    const r = s.SSL_connect(ssl);
    if (r === 1) {
      return io_tup(socket, io_done({ $: "Unit" }));
    }
    const err = s.SSL_get_error(ssl, r);
    if (err === 2 || err === 3) {
      if (wire_late(at)) {
        s.SSL_free(ssl);
        t.by.delete(fd);
        return io_tup(socket, io_fail(wire_timedout()));
      }
      io_park_on(fd, err === 3, k, go, at);
      return undefined;
    }
    const v = s.SSL_get_verify_result(ssl);
    return fail(v !== 0n ? String(s.X509_verify_cert_error_string(v)) : "TLS handshake failed");
  };
  return go();
}

function tls_send(socket, data, k) {
  return wire_tls_send(socket, wire_octets(data), k);
}

function tls_send_words(socket, n, words, k) {
  return wire_tls_send(socket, wire_words_octets(n, words), k);
}

function tls_send_file(socket, path, size, k) {
  return wire_tls_send(socket, wire_file(path, size), k);
}

function wire_tls_send(socket, b, k) {
  const t = wire_tls();
  const ssl = t && t.by.get(socket);
  if (!ssl) {
    return io_tup(socket, io_fail(9));
  }
  if (b === null) {
    return io_tup(socket, io_fail(22));
  }
  const go = (at) => {
    while (at < b.length) {
      const n = t.s.SSL_write(ssl, t.ffi.ptr(b, at), b.length - at);
      if (n > 0) {
        at += n;
        continue;
      }
      const err = t.s.SSL_get_error(ssl, n);
      if (err === 2 || err === 3) {
        io_park_on(socket, err === 3, k, () => go(at));
        return undefined;
      }
      return io_tup(socket, io_fail(32));
    }
    return io_tup(socket, io_done({ $: "Unit" }));
  };
  return go(0);
}

function tls_recv(socket, max, ms, k) {
  return wire_tls_recv(socket, max, ms, k, wire_text);
}

function tls_recv_words(socket, max, ms, k) {
  return wire_tls_recv(socket, max, ms, k, wire_words);
}

function wire_tls_recv(socket, max, ms, k, out) {
  const at = wire_deadline(ms);
  const t = wire_tls();
  const ssl = t && t.by.get(socket);
  if (!ssl) {
    return io_tup(socket, io_fail(9));
  }
  const b = new Uint8Array(Math.max(Number(max), 1));
  const go = () => {
    const n = t.s.SSL_read(ssl, t.ffi.ptr(b), b.length);
    if (n > 0) {
      return io_tup(socket, io_done(out(b, n)));
    }
    const err = t.s.SSL_get_error(ssl, n);
    if (err === 2 || err === 3) {
      if (wire_late(at)) {
        return io_tup(socket, io_fail(wire_timedout()));
      }
      io_park_on(socket, err === 3, k, go, at);
      return undefined;
    }
    if (err === 6) {
      return io_tup(socket, io_done(out(b, 0)));
    }
    return io_tup(socket, { $: "Fail", error: io_tup(5, "TLS read failed") });
  };
  return go();
}


function tls_close(socket) {
  const t = wire_tls();
  const ssl = t && t.by.get(socket);
  if (ssl) {
    t.s.SSL_shutdown(ssl);
    t.s.SSL_free(ssl);
    t.by.delete(socket);
  }
  io_sys().close(socket);
  return { $: "Unit" };
}

// Twin of connect in wire.c: a TCP connect with a deadline.
function connect(host, port, ms, k) {
  const sys = io_sys();
  const addr = io_addr(host, Number(port));
  if (addr === null) {
    return io_fail(22);
  }
  const fd = sys.socket(2, 1, 0);
  if (fd < 0) {
    return io_fail(sys.errno());
  }
  const at = wire_deadline(ms);
  const end = (code) => {
    if (code !== 0) {
      sys.close(fd);
      return io_fail(code);
    }
    return io_done(fd);
  };
  const error = () => {
    const v = new Int32Array([0]);
    const l = new Uint32Array([4]);
    return sys.getsockopt(fd, sys.mac ? 0xffff : 1, sys.mac ? 0x1007 : 4,
      sys.ptr(v), sys.ptr(l)) < 0 ? sys.errno() : v[0];
  };
  if (sys.fcntl(fd, 4, sys.fcntl(fd, 3, 0) | (sys.mac ? 4 : 0x800)) < 0) {
    return end(sys.errno());
  }
  const pending = [sys.mac ? 36 : 115, sys.mac ? 37 : 114];
  const done = sys.mac ? 56 : 106;
  const go = () => {
    const failed = error();
    if (failed !== 0) {
      return end(failed);
    }
    const code = sys.connect(fd, sys.ptr(addr), 16) >= 0 ? 0 : sys.errno();
    if (code === 0 || code === done) {
      return end(0);
    }
    if (pending.includes(code)) {
      if (wire_late(at)) {
        return end(wire_timedout());
      }
      io_park_on(fd, true, k, go, at);
      return undefined;
    }
    return end(code);
  };
  return go();
}

// Lookup
// ======
// Twin of lookup in wire.c: getaddrinfo through bun:ffi (blocking: the JS lane
// waits, as Process.run does). struct addrinfo keeps ai_canonname before
// ai_addr on macOS and after it on Linux. A host without bun:ffi fails with 3.
function wire_gai() {
  if (globalThis.BEND_GAI === undefined) {
    globalThis.BEND_GAI = null;
    try {
      const ffi = require("bun:ffi");
      const mac = process.platform === "darwin";
      const s = ffi.dlopen(mac ? "libSystem.dylib" : "libc.so.6", {
        getaddrinfo: { args: ["ptr", "ptr", "ptr", "ptr"], returns: "i32" },
        freeaddrinfo: { args: ["ptr"], returns: "void" },
        gai_strerror: { args: ["i32"], returns: "cstring" },
        inet_ntop: { args: ["i32", "ptr", "ptr", "u32"], returns: "ptr" },
      }).symbols;
      globalThis.BEND_GAI = { s, ffi, mac };
    } catch (_) {
      globalThis.BEND_GAI = null;
    }
  }
  return globalThis.BEND_GAI;
}

function lookup(host) {
  const g = wire_gai();
  if (g === null || host.length === 0 || host.includes("\0")) {
    return { $: "Fail", error: io_tup(3, "no system resolver here") };
  }
  const { s, ffi, mac } = g;
  const name = Buffer.from(host + "\0", "utf8");
  const hint = new Int32Array(12);
  hint[2] = 1; // ai_socktype SOCK_STREAM; ai_family 0 is AF_UNSPEC
  const out = new BigUint64Array(1);
  const r = s.getaddrinfo(ffi.ptr(name), null, ffi.ptr(hint), ffi.ptr(out));
  const codes = mac ? { 8: 1, 7: 1, 1: 1, 2: 2, 4: 2 } : { [-2]: 1, [-5]: 1, [-9]: 1, [-3]: 2, [-4]: 2 };
  if (r !== 0) {
    return { $: "Fail", error: io_tup(codes[r] || 3, String(s.gai_strerror(r))) };
  }
  const v6 = mac ? 30 : 10;
  const buf = Buffer.alloc(46);
  const addrs = [];
  for (let a = Number(out[0]); a && addrs.length < 16; a = ffi.read.ptr(a, 40)) {
    const fam = ffi.read.i32(a, 4);
    const sa = ffi.read.ptr(a, mac ? 32 : 24);
    if ((fam !== 2 && fam !== v6) || !sa) {
      continue;
    }
    if (s.inet_ntop(fam, sa + (fam === 2 ? 4 : 8), ffi.ptr(buf), 46)) {
      const text = buf.toString("latin1", 0, buf.indexOf(0));
      if (!addrs.includes(text)) {
        addrs.push(text);
      }
    }
  }
  s.freeaddrinfo(Number(out[0]));
  if (addrs.length === 0) {
    return { $: "Fail", error: io_tup(1, String(s.gai_strerror(mac ? 8 : -2))) };
  }
  let xs = { $: "Nil" };
  for (let i = addrs.length - 1; i >= 0; i -= 1) {
    xs = { $: "Con", head: addrs[i], tail: xs };
  }
  return io_done(xs);
}

io_eff(CID(lookup), lookup);
