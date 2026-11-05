// Wire
// ====
// Base's socket effects decode and encode UTF-8. These move octets as they
// are: one Char per byte (0..255) in, and each Char's low byte out.

#ifndef WIRE_BYTES
#define WIRE_BYTES

static Term wire_bytes(Env e, const char* p, u64 n) {
  Term s    = term_pak(CID(SNil), 0);
  u64  hole = 0;
  for (u64 i = 0; i < n; i += 1) {
    u64  l = heap_alloc(e, 1);
    Term t = term_ctr(CID(SCon), l);
    e.mem[l] = (uint8_t)p[i];
    if (hole == 0) {
      s = t;
    } else {
      e.mem[hole] = io_seal(e, t, CID(SCon));
    }
    hole = l + 1;
  }
  if (hole != 0) {
    e.mem[hole] = io_seal(e, term_pak(CID(SNil), 0), CID(SCon));
  }
  return s;
}

// A Char above 255 is not an octet; the send fails with EINVAL.
static char* wire_octets(Env e, Term s, u64* len, bool* bad) {
  u64   cap = 64;
  u64   n   = 0;
  char* buf = io_mem(malloc(cap));
  *bad = false;
  while (term_aux(s) == CID(SCon)) {
    Term fb[2];
    spare_free(e, cls_fit(2), ctr_take(e, s, 2, fb));
    if (n + 1 > cap) {
      cap *= 2;
      buf = io_mem(realloc(buf, cap));
    }
    *bad = *bad || (u64)fb[0] > 255;
    buf[n++] = (char)(fb[0] & 0xFF);
    s = fb[1];
  }
  *len = n;
  return buf;
}

// Bytes as bytes/bytes.bend packs them: byte i in bits 8*(i%4) of word i/4,
// 2^d words (the fewest that hold n), zero past n. Leans on the runtime's BUF
// block (blk_new, blk_loc, blk_read, blk_write); recheck on a Bend upgrade.
static Term wire_words(Env e, const char* p, u64 n) {
  u64  w    = (n + 3) / 4;
  u64 d    = 0;
  Term zero = 0;
  while ((1ull << d) < w) {
    d += 1;
  }
  Term a = blk_new(e, false, d, 0, 1, &zero);
  u64  l = blk_loc(e.mem, a);
  for (u64 k = 0; k < w; k += 1) {
    u32 x = 0;
    for (u64 j = 0; j < 4 && 4 * k + j < n; j += 1) {
      x |= (u32)(uint8_t)p[4 * k + j] << (8 * j);
    }
    blk_write(e.mem, false, l, (u32)k, x);
  }
  return io_tup(e, (Term)n, a);
}

// The first n bytes of a U32 BUF; n past its end fails with EINVAL.
static char* wire_words_octets(Env e, Term a, u64 n, u64* len, bool* bad) {
  u64* H = e.mem;
  *bad     = term_tag(a) != TAG_BUF || n > (4ull << blk_cls(a));
  *len     = *bad ? 0 : n;
  char* buf = io_mem(malloc(*len + 1));
  u64   l   = *bad ? 0 : blk_loc(H, a);
  for (u64 i = 0; i < *len; i += 1) {
    buf[i] = (char)(blk_read(H, false, l, (u32)(i / 4)) >> (8 * (i % 4)));
  }
  term_drop(e, a);
  return buf;
}

// A deadline in io_tick() nanoseconds, or 0 for none.
static u64 wire_deadline(u64 ms) {
  return ms != 0 ? io_tick() + ms * 1000000ull : 0;
}

static bool wire_late(u64 at) {
  return at != 0 && io_tick() >= at;
}

#endif

#if defined(CID(recv)) || defined(CID(recv.words))

// Not IO_READ: that would wait for readability before run, with no deadline.
// w->size holds the deadline until the read lands.
static Term wire_recv_go(Env e, IoWork* w, IoPack more, bool words) {
  int     fd = (int)w->hand;
  ssize_t n  = io_sys_end(w, recv(fd, w->data, (size_t)w->made, 0));
  if (w->code == EAGAIN) {
    if (!wire_late(w->size)) {
      return io_wait_on(w, fd, POLLIN, w->size, more);
    }
    w->code = ETIMEDOUT;
  }
  Term r = w->code ? io_fail(e, w->code, NULL)
    : io_done(e, words ? wire_words(e, w->data, (u64)n) : wire_bytes(e, w->data, (u64)n));
  free(w->data);
  return io_tup(e, io_hand(w->hand), r);
}

static void wire_recv_init(Term* f, IoWork* w) {
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->made = f[1] < INT32_MAX ? (intptr_t)f[1] : INT32_MAX;
  w->data = io_mem(malloc((size_t)w->made + 1));
  w->size = wire_deadline((u64)f[2]);
}

#endif

#ifdef CID(recv)

static Term wire_recv_more(Env e, IoWork* w) {
  return wire_recv_go(e, w, wire_recv_more, false);
}

Term wire_recv_run(Env e, Term* f, IoWork* w) {
  wire_recv_init(f, w);
  return wire_recv_more(e, w);
}

static void __attribute__((constructor)) wire_recv_use(void) {
  io_eff(CID(recv), wire_recv_run);
}

#endif

#ifdef CID(recv.words)

static Term wire_recv_words_more(Env e, IoWork* w) {
  return wire_recv_go(e, w, wire_recv_words_more, true);
}

Term wire_recv_words_run(Env e, Term* f, IoWork* w) {
  wire_recv_init(f, w);
  return wire_recv_words_more(e, w);
}

static void __attribute__((constructor)) wire_recv_words_use(void) {
  io_eff(CID(recv.words), wire_recv_words_run);
}

#endif

#if defined(CID(send)) || defined(CID(send.words)) || defined(CID(send.file))

static Term wire_send_more(Env e, IoWork* w) {
  int fd = (int)w->hand;
  while (w->code == 0 && (u64)w->made < w->size) {
    ssize_t n = send(fd, w->data + w->made, w->size - (u64)w->made, 0);
    if (n < 0 && errno == EAGAIN) {
      return io_wait_on(w, fd, POLLOUT, 0, wire_send_more);
    }
    w->made += io_sys_end(w, n);
  }
  Term r = w->code != 0 ? io_fail(e, w->code, NULL)
    : io_done(e, term_pak(CID(Unit), 0));
  free(w->data);
  return io_tup(e, io_hand(w->hand), r);
}

#endif

#ifdef CID(send)

Term wire_send_run(Env e, Term* f, IoWork* w) {
  bool bad;
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->data = wire_octets(e, f[1], &w->size, &bad);
  w->made = 0;
  w->code = bad ? EINVAL : 0;
  return wire_send_more(e, w);
}

static void __attribute__((constructor)) wire_send_use(void) {
  io_eff(CID(send), wire_send_run);
}

#endif

#ifdef CID(send.words)

Term wire_send_words_run(Env e, Term* f, IoWork* w) {
  bool bad;
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->data = wire_words_octets(e, f[2], (u64)f[1], &w->size, &bad);
  w->made = 0;
  w->code = bad ? EINVAL : 0;
  return wire_send_more(e, w);
}

static void __attribute__((constructor)) wire_send_words_use(void) {
  io_eff(CID(send.words), wire_send_words_run);
}

#endif

#if defined(CID(send.file)) || defined(CID(tls.send.file))

// The file at path (octets, one Char per byte), exactly size bytes of it, in a
// malloc'd buffer: a body part read at send time never enters the Bend heap.
// A path that is not octets or a file of another size: EINVAL; else errno.
static char* wire_file(Env e, Term path, u64 size, u64* len, int* code) {
  bool  bad;
  u64   n;
  char* p   = wire_octets(e, path, &n, &bad);
  p         = io_mem(realloc(p, n + 1));
  p[n]      = 0;
  char* buf = io_mem(malloc(size > 0 ? size : 1));
  *code     = bad ? EINVAL : 0;
  *len      = size;
  if (*code == 0) {
    errno    = 0;
    FILE* fp = fopen(p, "rb");
    if (fp == NULL) {
      *code = errno != 0 ? errno : ENOENT;
    } else {
      u64 got = (u64)fread(buf, 1, size, fp);
      if (got != size || fgetc(fp) != EOF) {
        *code = EINVAL;
      }
      fclose(fp);
    }
  }
  free(p);
  return buf;
}

#endif

#ifdef CID(send.file)

Term wire_send_file_run(Env e, Term* f, IoWork* w) {
  int code;
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->data = wire_file(e, f[1], (u64)f[2], &w->size, &code);
  w->made = 0;
  w->code = code;
  return wire_send_more(e, w);
}

static void __attribute__((constructor)) wire_send_file_use(void) {
  io_eff(CID(send.file), wire_send_file_run);
}

#endif

#ifdef CID(recv_from)

static Term wire_recv_from_more(Env e, IoWork* w) {
  struct sockaddr_in at = { 0 };
  socklen_t alen = sizeof(at);
  char      host[16];
  int       fd = (int)w->hand;
  ssize_t   n  = io_sys_end(w, recvfrom(fd, w->data, (size_t)w->made, 0,
    (struct sockaddr*)&at, &alen));
  if (w->code == EAGAIN) {
    if (!wire_late(w->size)) {
      return io_wait_on(w, fd, POLLIN, w->size, wire_recv_from_more);
    }
    w->code = ETIMEDOUT;
  }
  inet_ntop(AF_INET, &at.sin_addr, host, 16);
  Term r = w->code ? io_fail(e, w->code, NULL)
    : io_done(e, io_tup(e, io_str(e, host, strlen(host)),
      io_tup(e, ntohs(at.sin_port), wire_bytes(e, w->data, (u64)n))));
  free(w->data);
  return io_tup(e, io_hand(w->hand), r);
}

Term wire_recv_from_run(Env e, Term* f, IoWork* w) {
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->made = f[1] < INT32_MAX ? (intptr_t)f[1] : INT32_MAX;
  w->data = io_mem(malloc((size_t)w->made + 1));
  w->size = wire_deadline((u64)f[2]);
  return wire_recv_from_more(e, w);
}

static void __attribute__((constructor)) wire_recv_from_use(void) {
  io_eff(CID(recv_from), wire_recv_from_run);
}

#endif

#ifdef CID(send_to)

static Term wire_send_to_more(Env e, IoWork* w) {
  struct sockaddr_in at;
  int     fd = (int)w->hand;
  ssize_t n  = -1;
  errno      = EINVAL;
  if (w->code == 0 && io_sys_addr(w->text, (u32)w->made, &at) == 0) {
    n = sendto(fd, w->data, w->size, 0, (struct sockaddr*)&at, sizeof(at));
  }
  io_sys_end(w, n);
  if (w->code == EAGAIN) {
    return io_wait_on(w, fd, POLLOUT, 0, wire_send_to_more);
  }
  Term r = w->code != 0 ? io_fail(e, w->code, NULL)
    : io_done(e, term_pak(CID(Unit), 0));
  free(w->text);
  free(w->data);
  return io_tup(e, io_hand(w->hand), r);
}

Term wire_send_to_run(Env e, Term* f, IoWork* w) {
  uint64_t hn = 0;
  bool     bad;
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->text = io_cstr(e, f[1], &hn);
  w->made = (intptr_t)f[2];
  w->data = wire_octets(e, f[3], &w->size, &bad);
  w->code = bad || io_nul(w->text, hn) ? EINVAL : 0;
  return wire_send_to_more(e, w);
}

static void __attribute__((constructor)) wire_send_to_use(void) {
  io_eff(CID(send_to), wire_send_to_run);
}

#endif

#ifdef CID(connect)

// Base's TCP.connect waits as long as the kernel does (~75 s); this one has a
// deadline. On each wake SO_ERROR reports a failure, and connect() again says
// EALREADY (still going) or EISCONN (done).
static Term wire_connect_end(Env e, IoWork* w, int err) {
  int fd = (int)w->hand;
  if (err != 0 && fd >= 0) {
    close(fd);
  }
  free(w->data);
  return err != 0 ? io_fail(e, (u32)err, NULL) : io_done(e, io_hand(fd));
}

static Term wire_connect_more(Env e, IoWork* w) {
  struct sockaddr_in at;
  int       fd  = (int)w->hand;
  int       err = 0;
  socklen_t len = sizeof(err);
  if (getsockopt(fd, SOL_SOCKET, SO_ERROR, &err, &len) != 0) {
    err = errno;
  }
  if (err == 0) {
    io_sys_addr(w->data, (u32)w->made, &at);
    err = connect(fd, (struct sockaddr*)&at, sizeof(at)) == 0 ? 0 : errno;
  }
  if (err == 0 || err == EISCONN) {
    return wire_connect_end(e, w, 0);
  }
  if (err == EINPROGRESS || err == EALREADY) {
    if (!wire_late(w->size)) {
      return io_wait_on(w, fd, POLLOUT, w->size, wire_connect_more);
    }
    err = ETIMEDOUT;
  }
  return wire_connect_end(e, w, err);
}

Term connect_run(Env e, Term* f, IoWork* w) {
  struct sockaddr_in at;
  u64 hn  = 0;
  w->data = io_cstr(e, f[0], &hn);
  w->made = (intptr_t)f[1];
  w->hand = -1;
  if (io_nul(w->data, hn) || io_sys_addr(w->data, (u32)w->made, &at) != 0) {
    return wire_connect_end(e, w, EINVAL);
  }
  int fd = socket(AF_INET, SOCK_STREAM, 0);
  if (fd < 0) {
    return wire_connect_end(e, w, errno);
  }
  w->hand = fd;
  if (fcntl(fd, F_SETFL, fcntl(fd, F_GETFL) | O_NONBLOCK) < 0) {
    return wire_connect_end(e, w, errno);
  }
  w->size = wire_deadline((u64)f[2]);
  return wire_connect_more(e, w);
}

static void __attribute__((constructor)) connect_use(void) {
  io_eff(CID(connect), connect_run);
}

#endif

// TLS
// ===
// Peer verification (chain + host name) is always on; TLS 1.2 is the floor.
// The session of a socket lives in a table keyed by its fd. Two backends,
// both loaded at run time (bend links no extra libraries):
// - macOS: the system's TLS, Security.framework's SecureTransport, with the
//   Keychain's roots (a company's own root included). Nothing to install:
//   a Mac without Homebrew's OpenSSL made no HTTPS call at all before.
//   Its ceiling is TLS 1.2. When a provider we call stops serving TLS 1.2,
//   move macOS to Network.framework or a bundled OpenSSL. HTTP/1.1 only, so
//   no ALPN.
// - Linux, and macOS when BEND_LIBSSL names a libssl.3 (a dev build's
//   override: a signed release's library validation refuses a foreign
//   library): OpenSSL 3, roots from its default verify paths. No guessed
//   paths on macOS: one build, one TLS, whatever the Mac has installed.
// BEND_TLS_CA, a PEM file, adds trust anchors to the system's, on both: a
// company's root, or the tests' own CA.

#if defined(CID(tls.connect)) || defined(CID(tls.send)) || defined(CID(tls.recv)) || defined(CID(tls.close)) \
  || defined(CID(tls.send.words)) || defined(CID(tls.recv.words))
#ifndef WIRE_TLS
#define WIRE_TLS
#include <dlfcn.h>
#include <stdio.h>
#include <sys/stat.h>

// What a step of a session needs next (OpenSSL's SSL_get_error codes).
#define WIRE_TLS_DONE   0
#define WIRE_TLS_FAIL   1
#define WIRE_TLS_READ   2
#define WIRE_TLS_WRITE  3
#define WIRE_TLS_CLOSED 6

// SecureTransport's codes (Security/SecureTransport.h, SecBase.h).
#define WIRE_ST_WOULD_BLOCK   (-9803)  // errSSLWouldBlock
#define WIRE_ST_CLOSED        (-9805)  // errSSLClosedGraceful: close_notify
#define WIRE_ST_ABORT         (-9806)  // errSSLClosedAbort
#define WIRE_ST_NO_NOTIFY     (-9816)  // errSSLClosedNoNotify: a bare EOF
#define WIRE_ST_AUTH_DONE     (-9841)  // errSSLServerAuthCompleted

typedef struct {
  int         state;  // 0 not loaded, 1 loaded, -1 failed
  bool        apple;  // SecureTransport, else OpenSSL
  const char* why;    // why the load failed
  // OpenSSL
  void* ctx;
  void* (*ssl_new)(void*);
  int   (*set_fd)(void*, int);
  long  (*ctrl)(void*, int, long, void*);
  int   (*set1_host)(void*, const char*);
  int   (*connect)(void*);
  int   (*read)(void*, void*, int);
  int   (*write)(void*, const void*, int);
  int   (*get_error)(const void*, int);
  int   (*shutdown)(void*);
  void  (*ssl_free)(void*);
  long  (*verify_result)(const void*);
  const char* (*verify_text)(long);
  // SecureTransport (Security.framework) and the CoreFoundation it needs
  void* (*st_new)(const void*, int, int);
  int   (*st_io)(void*, void*, void*);
  int   (*st_conn)(void*, const void*);
  int   (*st_peer)(void*, const char*, size_t);
  int   (*st_min)(void*, int);
  int   (*st_option)(void*, int, unsigned char);
  int   (*st_shake)(void*);
  int   (*st_read)(void*, void*, size_t, size_t*);
  int   (*st_write)(void*, const void*, size_t, size_t*);
  int   (*st_close)(void*);
  int   (*st_trust)(void*, void**);
  int   (*sec_anchors)(void*, const void*);
  int   (*sec_anchors_only)(void*, unsigned char);
  bool  (*sec_evaluate)(void*, void**);
  void* (*sec_message)(int, void*);
  void* (*cf_error_text)(void*);
  unsigned char (*cf_cstr)(const void*, char*, long, unsigned);
  void  (*cf_release)(const void*);
  const void* anchors;  // BEND_TLS_CA's certificates (a CFArray), or NULL
} WireTls;

// A socket's session: an SSL* or an SSLContextRef.
typedef struct {
  void*  ssl;
  size_t held;  // SecureTransport: bytes SSLWrite took, sent once its queue drains
  short  want;  // SecureTransport: what its IO last waited for, POLLIN or POLLOUT
} WireTlsFd;

#define WIRE_TLS_FDS 65536
static WireTls   wire_tls;
static WireTlsFd wire_tls_fd[WIRE_TLS_FDS];
static char      wire_tls_text[512];

static const char* wire_tls_env(const char* name) {
  const char* v = getenv(name);
  return v != NULL && v[0] != 0 ? v : NULL;
}

static const char* wire_tls_ca_why(const char* path) {
  snprintf(wire_tls_text, sizeof(wire_tls_text), "BEND_TLS_CA (%s) holds no PEM certificate", path);
  return wire_tls_text;
}

// Linux, or macOS with BEND_LIBSSL.
static bool wire_tls_openssl(void) {
  const char* lib = wire_tls_env("BEND_LIBSSL");
#ifdef __APPLE__
  const char* names[] = { lib };
  wire_tls.why = "BEND_LIBSSL does not load OpenSSL 3 (a libssl.3.dylib): unset it to use macOS's own TLS";
#else
  const char* names[] = { lib, "libssl.so.3" };
  wire_tls.why = "TLS needs OpenSSL 3 (libssl.so.3): install it with the system's package manager, or set BEND_LIBSSL to its path";
#endif
  void* h = NULL;
  for (u64 i = 0; i < sizeof(names) / sizeof(names[0]) && h == NULL; i += 1) {
    h = names[i] != NULL ? dlopen(names[i], RTLD_NOW | RTLD_LOCAL) : NULL;
  }
  if (h == NULL) {
    return false;
  }
  void* (*method)(void)                 = dlsym(h, "TLS_client_method");
  void* (*ctx_new)(void*)               = dlsym(h, "SSL_CTX_new");
  int   (*paths)(void*)                 = dlsym(h, "SSL_CTX_set_default_verify_paths");
  int   (*locations)(void*, const char*, const char*) = dlsym(h, "SSL_CTX_load_verify_locations");
  void  (*verify)(void*, int, void*)    = dlsym(h, "SSL_CTX_set_verify");
  long  (*ctx_ctrl)(void*, int, long, void*) = dlsym(h, "SSL_CTX_ctrl");
  wire_tls.ssl_new       = dlsym(h, "SSL_new");
  wire_tls.set_fd        = dlsym(h, "SSL_set_fd");
  wire_tls.ctrl          = dlsym(h, "SSL_ctrl");
  wire_tls.set1_host     = dlsym(h, "SSL_set1_host");
  wire_tls.connect       = dlsym(h, "SSL_connect");
  wire_tls.read          = dlsym(h, "SSL_read");
  wire_tls.write         = dlsym(h, "SSL_write");
  wire_tls.get_error     = dlsym(h, "SSL_get_error");
  wire_tls.shutdown      = dlsym(h, "SSL_shutdown");
  wire_tls.ssl_free      = dlsym(h, "SSL_free");
  wire_tls.verify_result = dlsym(h, "SSL_get_verify_result");
  wire_tls.verify_text   = dlsym(h, "X509_verify_cert_error_string");
  if (!method || !ctx_new || !paths || !locations || !verify || !ctx_ctrl
    || !wire_tls.ssl_new || !wire_tls.set_fd || !wire_tls.ctrl
    || !wire_tls.set1_host || !wire_tls.connect || !wire_tls.read
    || !wire_tls.write || !wire_tls.get_error || !wire_tls.shutdown
    || !wire_tls.ssl_free || !wire_tls.verify_result || !wire_tls.verify_text) {
    return false;
  }
  void* ctx = ctx_new(method());
  if (ctx == NULL || paths(ctx) != 1) {
    return false;
  }
  const char* ca = wire_tls_env("BEND_TLS_CA");
  if (ca != NULL && locations(ctx, ca, NULL) != 1) {
    wire_tls.why = wire_tls_ca_why(ca);
    return false;
  }
  verify(ctx, 1, NULL);              // SSL_VERIFY_PEER
  ctx_ctrl(ctx, 123, 0x0303, NULL);  // SSL_CTRL_SET_MIN_PROTO_VERSION, TLS 1.2
  // A bare EOF is an error. close_notify is the only clean close.
  wire_tls.ctx = ctx;
  return true;
}

#ifdef __APPLE__

static void* wire_tls_sym(void* h, const char* name, bool* ok) {
  void* p = h != NULL ? dlsym(h, name) : NULL;
  *ok = *ok && p != NULL;
  return p;
}

// The text of a CFString, in wire_tls_text; released.
static const char* wire_tls_cf_text(void* s, const char* none) {
  bool ok = s != NULL && wire_tls.cf_cstr(s, wire_tls_text, sizeof(wire_tls_text), 0x08000100) != 0;  // UTF-8
  if (s != NULL) {
    wire_tls.cf_release(s);
  }
  return ok ? wire_tls_text : none;
}

static const char* wire_tls_st_why(int s) {
  const char* t = wire_tls_cf_text(wire_tls.sec_message(s, NULL), NULL);
  if (t == NULL) {
    snprintf(wire_tls_text, sizeof(wire_tls_text), "TLS handshake failed (OSStatus %d)", s);
  }
  return wire_tls_text;
}

// BEND_TLS_CA's certificates as a CFArray, or NULL.
static const void* wire_tls_st_anchors(void* sec, void* cf, const char* path) {
  bool ok = true;
  void* (*data)(const void*, const unsigned char*, long) = wire_tls_sym(cf, "CFDataCreate", &ok);
  long  (*count)(const void*) = wire_tls_sym(cf, "CFArrayGetCount", &ok);
  int   (*import)(const void*, const void*, unsigned*, unsigned*, unsigned, const void*, void*, const void**)
    = wire_tls_sym(sec, "SecItemImport", &ok);
  int fd = ok ? open(path, O_RDONLY | O_CLOEXEC) : -1;
  struct stat st;
  const void* items = NULL;
  if (fd >= 0 && fstat(fd, &st) == 0 && st.st_size > 0 && st.st_size < (1 << 24)) {
    char* buf = io_mem(malloc((size_t)st.st_size));
    if (read(fd, buf, (size_t)st.st_size) == st.st_size) {
      const void* d     = data(NULL, (const unsigned char*)buf, (long)st.st_size);
      unsigned    fmt   = 0;  // kSecFormatUnknown: PEM, found by SecItemImport
      unsigned    type  = 0;  // kSecItemTypeUnknown
      if (d != NULL && (import(d, NULL, &fmt, &type, 0, NULL, NULL, &items) != 0 || items == NULL || count(items) == 0)) {
        items = NULL;
      }
      if (d != NULL) {
        wire_tls.cf_release(d);
      }
    }
    free(buf);
  }
  if (fd >= 0) {
    close(fd);
  }
  return items;
}

// macOS without BEND_LIBSSL.
static bool wire_tls_apple(void) {
  wire_tls.why = "macOS's TLS (Security.framework) did not load";
  void* sec = dlopen("/System/Library/Frameworks/Security.framework/Security", RTLD_NOW | RTLD_LOCAL);
  void* cf  = dlopen("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation", RTLD_NOW | RTLD_LOCAL);
  bool  ok  = true;
  wire_tls.st_new           = wire_tls_sym(sec, "SSLCreateContext", &ok);
  wire_tls.st_io            = wire_tls_sym(sec, "SSLSetIOFuncs", &ok);
  wire_tls.st_conn          = wire_tls_sym(sec, "SSLSetConnection", &ok);
  wire_tls.st_peer          = wire_tls_sym(sec, "SSLSetPeerDomainName", &ok);
  wire_tls.st_min           = wire_tls_sym(sec, "SSLSetProtocolVersionMin", &ok);
  wire_tls.st_option        = wire_tls_sym(sec, "SSLSetSessionOption", &ok);
  wire_tls.st_shake         = wire_tls_sym(sec, "SSLHandshake", &ok);
  wire_tls.st_read          = wire_tls_sym(sec, "SSLRead", &ok);
  wire_tls.st_write         = wire_tls_sym(sec, "SSLWrite", &ok);
  wire_tls.st_close         = wire_tls_sym(sec, "SSLClose", &ok);
  wire_tls.st_trust         = wire_tls_sym(sec, "SSLCopyPeerTrust", &ok);
  wire_tls.sec_anchors      = wire_tls_sym(sec, "SecTrustSetAnchorCertificates", &ok);
  wire_tls.sec_anchors_only = wire_tls_sym(sec, "SecTrustSetAnchorCertificatesOnly", &ok);
  wire_tls.sec_evaluate     = wire_tls_sym(sec, "SecTrustEvaluateWithError", &ok);
  wire_tls.sec_message      = wire_tls_sym(sec, "SecCopyErrorMessageString", &ok);
  wire_tls.cf_error_text    = wire_tls_sym(cf, "CFErrorCopyDescription", &ok);
  wire_tls.cf_cstr          = wire_tls_sym(cf, "CFStringGetCString", &ok);
  wire_tls.cf_release       = wire_tls_sym(cf, "CFRelease", &ok);
  if (!ok) {
    return false;
  }
  const char* ca = wire_tls_env("BEND_TLS_CA");
  if (ca != NULL) {
    wire_tls.anchors = wire_tls_st_anchors(sec, cf, ca);
    if (wire_tls.anchors == NULL) {
      wire_tls.why = wire_tls_ca_why(ca);
      return false;
    }
  }
  wire_tls.apple = true;
  return true;
}

// SecureTransport's IO on the non-blocking socket fd: as many bytes as
// asked, or errSSLWouldBlock with the count so far and the wait it needs.
static int wire_tls_st_in(const void* conn, void* data, size_t* len) {
  int    fd  = (int)(intptr_t)conn;
  size_t got = 0;
  int    r   = 0;
  while (got < *len && r == 0) {
    ssize_t n = recv(fd, (char*)data + got, *len - got, 0);
    if (n > 0) {
      got += (size_t)n;
    } else if (n == 0) {
      r = WIRE_ST_NO_NOTIFY;
    } else if (errno == EAGAIN || errno == EWOULDBLOCK) {
      wire_tls_fd[fd].want = POLLIN;
      r = WIRE_ST_WOULD_BLOCK;
    } else if (errno != EINTR) {
      r = WIRE_ST_ABORT;
    }
  }
  *len = got;
  return r;
}

static int wire_tls_st_out(const void* conn, const void* data, size_t* len) {
  int    fd  = (int)(intptr_t)conn;
  size_t put = 0;
  int    r   = 0;
  while (put < *len && r == 0) {
    ssize_t n = send(fd, (const char*)data + put, *len - put, 0);
    if (n > 0) {
      put += (size_t)n;
    } else if (n < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
      wire_tls_fd[fd].want = POLLOUT;
      r = WIRE_ST_WOULD_BLOCK;
    } else if (n == 0 || errno != EINTR) {
      r = WIRE_ST_ABORT;
    }
  }
  *len = put;
  return r;
}

// With BEND_TLS_CA, the handshake stops at the server's chain: checked
// here against the system's roots plus those anchors (host name included,
// SSLSetPeerDomainName's policy).
static bool wire_tls_st_trusted(void* ssl, const char** why) {
  void* trust = NULL;
  void* err   = NULL;
  bool  ok    = wire_tls.st_trust(ssl, &trust) == 0 && trust != NULL
    && wire_tls.sec_anchors(trust, wire_tls.anchors) == 0
    && wire_tls.sec_anchors_only(trust, 0) == 0
    && wire_tls.sec_evaluate(trust, &err);
  if (!ok) {
    *why = wire_tls_cf_text(err != NULL ? wire_tls.cf_error_text(err) : NULL, "certificate not trusted");
  }
  if (err != NULL) {
    wire_tls.cf_release(err);
  }
  if (trust != NULL) {
    wire_tls.cf_release(trust);
  }
  return ok;
}

static int wire_tls_st_need(int fd) {
  return wire_tls_fd[fd].want == POLLOUT ? WIRE_TLS_WRITE : WIRE_TLS_READ;
}

#endif

static bool wire_tls_load(void) {
  if (wire_tls.state != 0) {
    return wire_tls.state > 0;
  }
#ifdef __APPLE__
  bool ok = wire_tls_env("BEND_LIBSSL") != NULL ? wire_tls_openssl() : wire_tls_apple();
#else
  bool ok = wire_tls_openssl();
#endif
  wire_tls.state = ok ? 1 : -1;
  return ok;
}

static void* wire_tls_of(int fd) {
  return fd >= 0 && fd < WIRE_TLS_FDS ? wire_tls_fd[fd].ssl : NULL;
}

static void wire_tls_drop(int fd) {
  void* ssl = wire_tls_of(fd);
  if (ssl != NULL) {
#ifdef __APPLE__
    if (wire_tls.apple) {
      wire_tls.cf_release(ssl);
    } else
#endif
    wire_tls.ssl_free(ssl);
    wire_tls_fd[fd].ssl = NULL;
  }
}

// A new session on fd (in range) for host: 0, or an errno.
static u32 wire_tls_begin(int fd, const char* host) {
  WireTlsFd* t = &wire_tls_fd[fd];
  t->held = 0;
  t->want = POLLIN;
#ifdef __APPLE__
  if (wire_tls.apple) {
    t->ssl = wire_tls.st_new(NULL, 1, 0);  // kSSLClientSide, kSSLStreamType
    if (t->ssl == NULL) {
      return ENOMEM;
    }
    bool ok = wire_tls.st_io(t->ssl, (void*)wire_tls_st_in, (void*)wire_tls_st_out) == 0
      && wire_tls.st_conn(t->ssl, (const void*)(intptr_t)fd) == 0
      && wire_tls.st_peer(t->ssl, host, strlen(host)) == 0  // SNI and host name check
      && wire_tls.st_min(t->ssl, 8) == 0                    // kTLSProtocol12
      && (wire_tls.anchors == NULL || wire_tls.st_option(t->ssl, 0, 1) == 0);  // kSSLSessionOptionBreakOnServerAuth
    return ok ? 0 : EPROTO;
  }
#endif
  t->ssl = wire_tls.ssl_new(wire_tls.ctx);
  if (t->ssl == NULL) {
    return ENOMEM;
  }
  // SNI (SSL_CTRL_SET_TLSEXT_HOSTNAME, host_name) and host name verification.
  bool ok = wire_tls.set_fd(t->ssl, fd) == 1 && wire_tls.ctrl(t->ssl, 55, 0, (void*)host) == 1
    && wire_tls.set1_host(t->ssl, host) == 1;
  return ok ? 0 : EPROTO;
}

// One handshake step: DONE, READ or WRITE (wait, then again), or FAIL with why.
static int wire_tls_shake(int fd, const char** why) {
  void* ssl = wire_tls_of(fd);
  *why = "TLS handshake failed";
#ifdef __APPLE__
  if (wire_tls.apple) {
    for (;;) {
      wire_tls_fd[fd].want = POLLIN;
      int s = wire_tls.st_shake(ssl);
      if (s == 0) {
        return WIRE_TLS_DONE;
      }
      if (s == WIRE_ST_WOULD_BLOCK) {
        return wire_tls_st_need(fd);
      }
      if (s != WIRE_ST_AUTH_DONE) {
        *why = wire_tls_st_why(s);
        return WIRE_TLS_FAIL;
      }
      if (!wire_tls_st_trusted(ssl, why)) {
        return WIRE_TLS_FAIL;
      }
    }
  }
#endif
  int r = wire_tls.connect(ssl);
  if (r == 1) {
    return WIRE_TLS_DONE;
  }
  int err = wire_tls.get_error(ssl, r);
  if (err == WIRE_TLS_READ || err == WIRE_TLS_WRITE) {
    return err;
  }
  long v = wire_tls.verify_result(ssl);
  if (v != 0) {
    *why = wire_tls.verify_text(v);
  }
  return WIRE_TLS_FAIL;
}

// Read at most n bytes: DONE with *got > 0, CLOSED (close_notify), READ or
// WRITE (wait, then again), or FAIL (a bare EOF too).
static int wire_tls_get(int fd, void* buf, int n, int* got) {
  void* ssl = wire_tls_of(fd);
  *got = 0;
#ifdef __APPLE__
  if (wire_tls.apple) {
    size_t k = 0;
    wire_tls_fd[fd].want = POLLIN;
    int s = wire_tls.st_read(ssl, buf, (size_t)n, &k);
    *got = (int)k;
    return k > 0 ? WIRE_TLS_DONE
      : s == WIRE_ST_WOULD_BLOCK ? wire_tls_st_need(fd)
      : s == WIRE_ST_CLOSED ? WIRE_TLS_CLOSED : WIRE_TLS_FAIL;
  }
#endif
  int r = wire_tls.read(ssl, buf, n);
  if (r > 0) {
    *got = r;
    return WIRE_TLS_DONE;
  }
  int err = wire_tls.get_error(ssl, r);
  return err == WIRE_TLS_READ || err == WIRE_TLS_WRITE || err == WIRE_TLS_CLOSED ? err : WIRE_TLS_FAIL;
}

// Write some of the n bytes: DONE with *put > 0, READ or WRITE (wait, then
// again with the same bytes), or FAIL. SecureTransport takes a whole write
// before it can send it: a blocked SSLWrite keeps it (held), and the next
// call only drains its queue.
static int wire_tls_put(int fd, const void* buf, int n, int* put) {
  void* ssl = wire_tls_of(fd);
  *put = 0;
#ifdef __APPLE__
  if (wire_tls.apple) {
    WireTlsFd* t = &wire_tls_fd[fd];
    size_t     k = 0;
    int        s;
    t->want = POLLOUT;
    if (t->held > 0) {
      s = wire_tls.st_write(ssl, NULL, 0, &k);
      if (s == 0) {
        *put    = (int)t->held;
        t->held = 0;
        return WIRE_TLS_DONE;
      }
    } else {
      size_t m = n > (1 << 20) ? (size_t)1 << 20 : (size_t)n;
      s = wire_tls.st_write(ssl, buf, m, &k);
      if (s == 0) {
        *put = (int)k;
        return k > 0 ? WIRE_TLS_DONE : WIRE_TLS_FAIL;
      }
      if (s == WIRE_ST_WOULD_BLOCK) {
        t->held = m;
      }
    }
    return s == WIRE_ST_WOULD_BLOCK ? wire_tls_st_need(fd) : WIRE_TLS_FAIL;
  }
#endif
  int r = wire_tls.write(ssl, buf, n);
  if (r > 0) {
    *put = r;
    return WIRE_TLS_DONE;
  }
  int err = wire_tls.get_error(ssl, r);
  return err == WIRE_TLS_READ || err == WIRE_TLS_WRITE ? err : WIRE_TLS_FAIL;
}

// One non-blocking close_notify attempt, then the session is freed.
static void wire_tls_end(int fd) {
  void* ssl = wire_tls_of(fd);
  if (ssl == NULL) {
    return;
  }
#ifdef __APPLE__
  if (wire_tls.apple) {
    wire_tls.st_close(ssl);
  } else
#endif
  wire_tls.shutdown(ssl);
  wire_tls_drop(fd);
}

#endif
#endif

#ifdef CID(tls.connect)

static Term wire_tls_connect_end(Env e, IoWork* w, Term r) {
  free(w->text);
  return io_tup(e, io_hand(w->hand), r);
}

static Term wire_tls_connect_fail(Env e, IoWork* w, u32 code, const char* why) {
  wire_tls_drop((int)w->hand);
  return wire_tls_connect_end(e, w, io_fail(e, code, why));
}

static Term wire_tls_connect_more(Env e, IoWork* w) {
  int         fd  = (int)w->hand;
  const char* why = NULL;
  int         r   = wire_tls_shake(fd, &why);
  if (r == WIRE_TLS_DONE) {
    return wire_tls_connect_end(e, w, io_done(e, term_pak(CID(Unit), 0)));
  }
  if (r == WIRE_TLS_READ || r == WIRE_TLS_WRITE) {
    if (!wire_late(w->size)) {
      return io_wait_on(w, fd, r == WIRE_TLS_READ ? POLLIN : POLLOUT, w->size, wire_tls_connect_more);
    }
    return wire_tls_connect_fail(e, w, ETIMEDOUT, NULL);
  }
  return wire_tls_connect_fail(e, w, EPROTO, why);
}

// ENOENT: no TLS library (the only failure that is not the network's).
Term tls_connect_run(Env e, Term* f, IoWork* w) {
  uint64_t hn = 0;
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->text = io_cstr(e, f[1], &hn);
  w->size = wire_deadline((u64)f[2]);
  int fd  = (int)w->hand;
  if (!wire_tls_load()) {
    return wire_tls_connect_end(e, w, io_fail(e, ENOENT, wire_tls.why));
  }
  if (fd < 0 || fd >= WIRE_TLS_FDS || io_nul(w->text, hn)) {
    return wire_tls_connect_end(e, w, io_fail(e, EINVAL, NULL));
  }
  u32 code = wire_tls_begin(fd, w->text);
  if (code != 0) {
    return wire_tls_connect_fail(e, w, code, code == ENOMEM ? NULL : "TLS setup failed");
  }
  return wire_tls_connect_more(e, w);
}

static void __attribute__((constructor)) tls_connect_use(void) {
  io_eff(CID(tls.connect), tls_connect_run);
}

#endif

#if defined(CID(tls.send)) || defined(CID(tls.send.words)) || defined(CID(tls.send.file))

// A write is retried with the same buffer, as both backends require.
static Term wire_tls_send_more(Env e, IoWork* w) {
  int fd = (int)w->hand;
  while (w->code == 0 && (u64)w->made < w->size) {
    if (wire_tls_of(fd) == NULL) {
      w->code = EBADF;
      break;
    }
    u64 left = w->size - (u64)w->made;
    int n    = 0;
    int r    = wire_tls_put(fd, w->data + w->made, left > INT32_MAX ? INT32_MAX : (int)left, &n);
    if (r == WIRE_TLS_DONE) {
      w->made += n;
      continue;
    }
    if (r == WIRE_TLS_READ || r == WIRE_TLS_WRITE) {
      return io_wait_on(w, fd, r == WIRE_TLS_READ ? POLLIN : POLLOUT, 0, wire_tls_send_more);
    }
    w->code = EPIPE;
  }
  Term r = w->code != 0 ? io_fail(e, w->code, NULL)
    : io_done(e, term_pak(CID(Unit), 0));
  free(w->data);
  return io_tup(e, io_hand(w->hand), r);
}

#endif

#ifdef CID(tls.send)

Term tls_send_run(Env e, Term* f, IoWork* w) {
  bool bad;
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->data = wire_octets(e, f[1], &w->size, &bad);
  w->made = 0;
  w->code = bad ? EINVAL : 0;
  return wire_tls_send_more(e, w);
}

static void __attribute__((constructor)) tls_send_use(void) {
  io_eff(CID(tls.send), tls_send_run);
}

#endif

#ifdef CID(tls.send.words)

Term tls_send_words_run(Env e, Term* f, IoWork* w) {
  bool bad;
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->data = wire_words_octets(e, f[2], (u64)f[1], &w->size, &bad);
  w->made = 0;
  w->code = bad ? EINVAL : 0;
  return wire_tls_send_more(e, w);
}

static void __attribute__((constructor)) tls_send_words_use(void) {
  io_eff(CID(tls.send.words), tls_send_words_run);
}

#endif

#ifdef CID(tls.send.file)

Term tls_send_file_run(Env e, Term* f, IoWork* w) {
  int code;
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->data = wire_file(e, f[1], (u64)f[2], &w->size, &code);
  w->made = 0;
  w->code = code;
  return wire_tls_send_more(e, w);
}

static void __attribute__((constructor)) tls_send_file_use(void) {
  io_eff(CID(tls.send.file), tls_send_file_run);
}

#endif

#if defined(CID(tls.recv)) || defined(CID(tls.recv.words))

// Empty means close_notify. A bare EOF is a read error, not a clean close.
static Term wire_tls_recv_go(Env e, IoWork* w, IoPack more, bool words) {
  int   fd  = (int)w->hand;
  void* ssl = wire_tls_of(fd);
  int   n   = 0;
  int   err = ssl != NULL ? wire_tls_get(fd, w->data, (int)w->made, &n) : WIRE_TLS_FAIL;
  Term  r;
  if (err == WIRE_TLS_DONE) {
    r = io_done(e, words ? wire_words(e, w->data, (u64)n) : wire_bytes(e, w->data, (u64)n));
  } else {
    if (err == WIRE_TLS_READ || err == WIRE_TLS_WRITE) {
      if (!wire_late(w->size)) {
        return io_wait_on(w, fd, err == WIRE_TLS_READ ? POLLIN : POLLOUT, w->size, more);
      }
      err = -1;
    }
    r = err == WIRE_TLS_CLOSED ? io_done(e, words ? wire_words(e, w->data, 0) : term_pak(CID(SNil), 0))
      : err == -1 ? io_fail(e, ETIMEDOUT, NULL)
      : io_fail(e, ssl != NULL ? EIO : EBADF, ssl != NULL ? "TLS read failed" : NULL);
  }
  free(w->data);
  return io_tup(e, io_hand(w->hand), r);
}

static void wire_tls_recv_init(Term* f, IoWork* w) {
  w->hand = (intptr_t)io_hand_v(f[0]);
  w->made = f[1] < INT32_MAX ? (intptr_t)f[1] : INT32_MAX;
  w->data = io_mem(malloc((size_t)w->made + 1));
  w->size = wire_deadline((u64)f[2]);
}

#endif

#ifdef CID(tls.recv)

static Term wire_tls_recv_more(Env e, IoWork* w) {
  return wire_tls_recv_go(e, w, wire_tls_recv_more, false);
}

Term tls_recv_run(Env e, Term* f, IoWork* w) {
  wire_tls_recv_init(f, w);
  return wire_tls_recv_more(e, w);
}

static void __attribute__((constructor)) tls_recv_use(void) {
  io_eff(CID(tls.recv), tls_recv_run);
}

#endif

#ifdef CID(tls.recv.words)

static Term wire_tls_recv_words_more(Env e, IoWork* w) {
  return wire_tls_recv_go(e, w, wire_tls_recv_words_more, true);
}

Term tls_recv_words_run(Env e, Term* f, IoWork* w) {
  wire_tls_recv_init(f, w);
  return wire_tls_recv_words_more(e, w);
}

static void __attribute__((constructor)) tls_recv_words_use(void) {
  io_eff(CID(tls.recv.words), tls_recv_words_run);
}

#endif

#ifdef CID(tls.close)

// ponytail: one non-blocking close_notify attempt; no wait for the peer's
Term tls_close_run(Env e, Term* f, IoWork* w) {
  int fd = (int)io_hand_v(f[0]);
  wire_tls_end(fd);
  close(fd);
  return term_pak(CID(Unit), 0);
}

static void __attribute__((constructor)) tls_close_use(void) {
  io_eff(CID(tls.close), tls_close_run);
}

#endif

#ifdef CID(lookup)
#ifndef WIRE_LOOKUP
#define WIRE_LOOKUP
#include <netdb.h>

// Lookup
// ======
// The OS resolver (getaddrinfo), on an IO helper thread: the addresses curl
// and every other program get, /etc/hosts, CNAMEs, scoped and split DNS
// included. IPv4 and IPv6, in its order, as text; at most WIRE_ADDRS.
// Fail codes are this effect's own, the same on every OS: 1 the name has no
// address, 2 the resolver did not answer, 3 another failure.

#define WIRE_ADDRS 16
#define WIRE_ADDR  46

static u32 wire_lookup_code(int r) {
  switch (r) {
    case EAI_NONAME:
#if defined(EAI_NODATA) && EAI_NODATA != EAI_NONAME
    case EAI_NODATA:
#endif
#if defined(EAI_ADDRFAMILY)
    case EAI_ADDRFAMILY:
#endif
      return 1;
    case EAI_AGAIN:
    case EAI_FAIL:
      return 2;
    default:
      return 3;
  }
}

static void wire_lookup_call(IoWork* w) {
  struct addrinfo  hint = { 0 };
  struct addrinfo* res  = NULL;
  hint.ai_family   = AF_UNSPEC;
  hint.ai_socktype = SOCK_STREAM;
  int r = getaddrinfo(w->data, NULL, &hint, &res);
  w->made = 0;
  if (r != 0) {
    w->code = wire_lookup_code(r);
    w->word = (u32)r;
    w->size = r == EAI_SYSTEM ? (u64)errno : 0;
    return;
  }
  w->text = io_mem(calloc(WIRE_ADDRS, WIRE_ADDR));
  for (struct addrinfo* a = res; a != NULL && w->made < WIRE_ADDRS; a = a->ai_next) {
    char* at = w->text + w->made * WIRE_ADDR;
    const void* src = a->ai_family == AF_INET
      ? (const void*)&((struct sockaddr_in*)a->ai_addr)->sin_addr
      : a->ai_family == AF_INET6
      ? (const void*)&((struct sockaddr_in6*)a->ai_addr)->sin6_addr : NULL;
    if (src == NULL || inet_ntop(a->ai_family, src, at, WIRE_ADDR) == NULL) {
      continue;
    }
    bool seen = false;
    for (intptr_t i = 0; i < w->made; i += 1) {
      seen = seen || strcmp(w->text + i * WIRE_ADDR, at) == 0;
    }
    w->made += seen ? 0 : 1;
  }
  freeaddrinfo(res);
  w->code = w->made == 0 ? 1 : 0;
  w->word = (u32)EAI_NONAME;
}

static Term wire_lookup_pack(Env e, IoWork* w) {
  Term r;
  if (w->code != 0) {
    int         g    = (int)w->word;
    const char* text = g == EAI_SYSTEM ? strerror((int)w->size) : gai_strerror(g);
    r = io_fail(e, w->code, text);
  } else {
    Term xs = term_pak(CID(Nil), 0);
    for (intptr_t i = w->made; i > 0; i -= 1) {
      const char* at = w->text + (i - 1) * WIRE_ADDR;
      xs = io_node(e, CID(Con), io_str(e, at, strlen(at)), xs);
    }
    r = io_done(e, xs);
  }
  free(w->text);
  free(w->data);
  return r;
}

Term wire_lookup_run(Env e, Term* f, IoWork* w) {
  u64 hn  = 0;
  w->data = io_cstr(e, f[0], &hn);
  w->text = NULL;
  if (io_nul(w->data, hn) || hn == 0) {
    w->code = 3;
    w->word = (u32)EAI_NONAME;
    return wire_lookup_pack(e, w);
  }
  return io_work(w, wire_lookup_call, wire_lookup_pack);
}

static void __attribute__((constructor)) wire_lookup_use(void) {
  io_eff(CID(lookup), wire_lookup_run);
}

#endif
#endif
