// tray-badge.node: the in-process AppKit addon (Electron's main), for what Electron has no API
// for. Two calls; it decides nothing, src/main/traybadge.ts and its callers do.
//
// setBadge(dot | null): a dot over bise's menu bar icon, the way a native app badges its status
// item: the icon stays a template image (macOS tints it for the bar, which follows the
// wallpaper), and one named CALayer draws the dot over it in its own colour. Where the dot goes,
// its size and its colour come from src/main/traylook.ts (trayDot). It finds the button of each
// status item of this process (bise has one tray) and returns whether it found one: false lets
// the caller fall back (the tray's title).
//
// stillWindow(handle): the window of that view (BrowserWindow.getNativeWindowHandle(), an
// NSView*) shows and hides with no AppKit animation (NSWindowAnimationBehaviorNone). The
// capsule's: without it macOS pops it open (its frame 98% -> 101% -> 100%, ~170 ms) and fades it
// out (~250 ms) on every show and hide, which made the tray's dropdown feel slow (tray-instant).
// Returns false, touching nothing, when the handle isn't a pointer to a view in a window.
//
// N-API only (ABI stable, no node-gyp, no Electron headers): the few functions it needs are
// declared below as node_api.h declares them, resolved from the host at load
// (-undefined dynamic_lookup). Must run on the main thread: Electron's main process JS does.
#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>

typedef struct napi_env__ *napi_env;
typedef struct napi_value__ *napi_value;
typedef struct napi_callback_info__ *napi_callback_info;
typedef int napi_status;
typedef napi_value (*napi_callback)(napi_env env, napi_callback_info info);
extern napi_status napi_get_cb_info(napi_env, napi_callback_info, size_t *, napi_value *, napi_value *, void **);
extern napi_status napi_get_named_property(napi_env, napi_value, const char *, napi_value *);
extern napi_status napi_get_value_double(napi_env, napi_value, double *);
extern napi_status napi_get_boolean(napi_env, bool, napi_value *);
extern napi_status napi_create_function(napi_env, const char *, size_t, napi_callback, void *, napi_value *);
extern napi_status napi_set_named_property(napi_env, napi_value, const char *, napi_value);
extern napi_status napi_typeof(napi_env, napi_value, int *);
extern napi_status napi_get_buffer_info(napi_env, napi_value, void **, size_t *);

static NSString *const kBadge = @"bise.tray.badge";
enum { napi_object_type = 6 };

typedef struct {
  double x, y, d;        // the dot's centre and diameter, in points, from the image's top left
  double r, g, b;        // its colour, 0..1
  double ring;           // the clear ring around it, in points (the template image has the hole)
} Dot;

static void collectButtons(NSView *v, NSMutableArray<NSButton *> *out) {
  if ([v isKindOfClass:[NSButton class]] && [NSStringFromClass([v class]) containsString:@"StatusBar"]) [out addObject:(NSButton *)v];
  for (NSView *s in v.subviews) collectButtons(s, out);
}

static NSArray<NSButton *> *statusButtons(void) {
  NSMutableArray<NSButton *> *out = [NSMutableArray array];
  for (NSWindow *w in NSApp.windows) {
    if (![NSStringFromClass([w class]) containsString:@"StatusBarWindow"]) continue;
    NSView *root = w.contentView.superview ?: w.contentView;
    if (root != nil) collectButtons(root, out);
  }
  return out;
}

static void clear(NSButton *b) {
  for (CALayer *l in [b.layer.sublayers copy]) if ([l.name isEqualToString:kBadge]) [l removeFromSuperlayer];
}

static void put(NSButton *b, Dot dot) {
  b.wantsLayer = YES;
  clear(b);
  NSSize img = b.image ? b.image.size : NSMakeSize(0, 0);
  NSRect frame = [b.cell imageRectForBounds:b.bounds];
  // the cell's rect can be wider than the image: centre the image in it, as the cell draws it
  double ox = NSMinX(frame) + (NSWidth(frame) - img.width) / 2;
  double oy = NSMinY(frame) + (NSHeight(frame) - img.height) / 2;
  double cx = ox + dot.x;
  double cy = b.isFlipped ? oy + dot.y : oy + img.height - dot.y;
  CALayer *l = [CALayer layer];
  l.name = kBadge;
  l.frame = CGRectMake(cx - dot.d / 2, cy - dot.d / 2, dot.d, dot.d);
  l.cornerRadius = dot.d / 2;
  l.backgroundColor = [NSColor colorWithSRGBRed:dot.r green:dot.g blue:dot.b alpha:1].CGColor;
  l.contentsScale = b.window.backingScaleFactor;
  l.zPosition = 10;
  [b.layer addSublayer:l];
}

static bool num(napi_env env, napi_value o, const char *k, double *out) {
  napi_value v;
  return napi_get_named_property(env, o, k, &v) == 0 && napi_get_value_double(env, v, out) == 0;
}

static napi_value setBadge(napi_env env, napi_callback_info info) {
  size_t argc = 1;
  napi_value argv[1] = {0};
  napi_value result;
  napi_get_cb_info(env, info, &argc, argv, NULL, NULL);
  int type = 0;
  bool on = argc >= 1 && napi_typeof(env, argv[0], &type) == 0 && type == napi_object_type;
  Dot dot = {0};
  if (on && !(num(env, argv[0], "x", &dot.x) && num(env, argv[0], "y", &dot.y) && num(env, argv[0], "d", &dot.d) &&
              num(env, argv[0], "r", &dot.r) && num(env, argv[0], "g", &dot.g) && num(env, argv[0], "b", &dot.b))) {
    napi_get_boolean(env, false, &result);
    return result;
  }
  NSArray<NSButton *> *buttons = statusButtons();
  for (NSButton *b in buttons) {
    if (on) put(b, dot); else clear(b);
  }
  napi_get_boolean(env, buttons.count > 0, &result);
  return result;
}

static napi_value stillWindow(napi_env env, napi_callback_info info) {
  size_t argc = 1;
  napi_value argv[1] = {0};
  napi_value result;
  napi_get_cb_info(env, info, &argc, argv, NULL, NULL);
  void *data = NULL;
  size_t len = 0;
  bool ok = argc >= 1 && napi_get_buffer_info(env, argv[0], &data, &len) == 0 && data != NULL && len == sizeof(void *);
  void *ptr = ok ? *(void **)data : NULL;
  ok = ok && ptr != NULL;
  if (ok) {
    id obj = (__bridge id)ptr;
    NSWindow *w = [obj isKindOfClass:[NSView class]] ? ((NSView *)obj).window : nil;
    ok = w != nil;
    if (ok) w.animationBehavior = NSWindowAnimationBehaviorNone;
  }
  napi_get_boolean(env, ok, &result);
  return result;
}

__attribute__((visibility("default"))) int32_t node_api_module_get_api_version_v1(void) { return 8; }

__attribute__((visibility("default"))) napi_value napi_register_module_v1(napi_env env, napi_value exports) {
  napi_value fn;
  napi_create_function(env, "setBadge", (size_t)-1, setBadge, NULL, &fn);
  napi_set_named_property(env, exports, "setBadge", fn);
  napi_create_function(env, "stillWindow", (size_t)-1, stillWindow, NULL, &fn);
  napi_set_named_property(env, exports, "stillWindow", fn);
  return exports;
}
