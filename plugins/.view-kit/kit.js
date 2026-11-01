// bise view kit · the shared script of the gallery's built-in plugins' MCP Apps views.
// Inlined as-is into each view by its server at resources/read (plugins/.view-kit/inline.mjs);
// the host injects nothing. One global, `kit`:
//   kit.app         the MCP Apps bridge (SEP-1865, spec 2026-01-26): JSON-RPC 2.0 over
//                   postMessage to the host, no SDK. on(name, fn) for "tool-input",
//                   "tool-input-partial", "tool-result", "tool-cancelled", "context",
//                   "teardown"; start(name), call(tool, args), read(uri), message(text),
//                   setContext(text, structured), openLink(url), download(contents),
//                   displayMode(mode).
//   kit.esc(s)      a string escaped for innerHTML (check.py fails an unescaped ${...})
//   kit.series(i)   the i-th series color (0-based, wraps at 8), resolved for the theme
//   kit.tip(el, html | fn)  a tooltip on hover and focus that never takes the pointer
//   kit.saveSvg(svg, name)  kit.savePng(svg, name, scale)  a download through the host
//   kit.fmt(n)      a number, short and in the host's locale (1.2k, 3.4M)
//   kit.menu(button, [{label, run, testid?}])  a few actions under a button (.k-menu):
//                   below it or flipped up, ↑ ↓ ⏎ esc, closed by an outside click
// The theme comes from the host context (html[data-theme], the spec's CSS variables on
// :root), the display mode as html[data-mode]; kit.css reads both.
(() => {
  "use strict";
  const handlers = {};
  const pending = new Map();
  let nextId = 1;
  let ctx = {};
  const post = (m) => window.parent.postMessage({ jsonrpc: "2.0", ...m }, "*");
  const request = (method, params) => new Promise((resolve, reject) => {
    const id = nextId++;
    pending.set(id, { resolve, reject });
    post({ id, method, params });
  });
  const notify = (method, params) => post({ method, params });

  function applyContext(c) {
    if (!c) return;
    ctx = { ...ctx, ...c };
    const root = document.documentElement;
    if (c.theme === "light" || c.theme === "dark") root.dataset.theme = c.theme;
    const vars = c.styles && c.styles.variables;
    if (vars) for (const [k, v] of Object.entries(vars)) if (v && k.startsWith("--")) root.style.setProperty(k, v);
    if (c.displayMode) root.dataset.mode = c.displayMode;
    if (handlers.context) handlers.context(ctx);
  }

  window.addEventListener("message", (e) => {
    if (e.source !== window.parent) return;
    const m = e.data;
    if (!m || m.jsonrpc !== "2.0") return;
    if (m.method && m.id !== undefined) {
      if (m.method === "ui/resource-teardown") {
        try { if (handlers.teardown) handlers.teardown(); } finally { post({ id: m.id, result: {} }); }
      } else if (m.method === "ping") post({ id: m.id, result: {} });
      else post({ id: m.id, error: { code: -32601, message: "method not found" } });
      return;
    }
    if (m.method) {
      if (m.method === "ui/notifications/host-context-changed") { applyContext(m.params); return; }
      const h = handlers[m.method.replace("ui/notifications/", "")];
      if (h) h(m.params || {});
      return;
    }
    const p = pending.get(m.id);
    if (!p) return;
    pending.delete(m.id);
    if (m.error) p.reject(Object.assign(new Error(m.error.message || "error"), { code: m.error.code, data: m.error.data }));
    else p.resolve(m.result);
  });

  const app = {
    on(name, fn) { handlers[name] = fn; },
    get context() { return ctx; },
    async call(name, args) {
      const r = await request("tools/call", { name, arguments: args || {} });
      if (r && r.isError) throw new Error(((r.content || [])[0] || {}).text || name + " failed");
      return r;
    },
    read: (uri) => request("resources/read", { uri }),
    message: (text) => request("ui/message", { role: "user", content: [{ type: "text", text }] }),
    setContext: (text, structuredContent) => request("ui/update-model-context", { content: [{ type: "text", text }], structuredContent }),
    openLink: (url) => request("ui/open-link", { url }),
    download: (contents) => request("ui/download-file", { contents }),
    displayMode: (mode) => request("ui/request-display-mode", { mode }),
    async start(name, appCapabilities) {
      const r = await request("ui/initialize", {
        appInfo: { name, version: "1.0.0" }, appCapabilities: appCapabilities || {}, protocolVersion: "2026-01-26",
      });
      applyContext(r && r.hostContext);
      notify("ui/notifications/initialized", {});
      // the body's own size: the document's scrollHeight never drops below the frame's
      const size = () => {
        const b = document.body.getBoundingClientRect();
        notify("ui/notifications/size-changed", { width: Math.ceil(b.width), height: Math.ceil(b.height) });
      };
      new ResizeObserver(size).observe(document.body);
      return r;
    },
  };

  const ESC = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" };
  const esc = (s) => String(s == null ? "" : s).replace(/[&<>"']/g, (c) => ESC[c]);

  function series(i) {
    const n = ((i % 8) + 8) % 8 + 1;
    return getComputedStyle(document.documentElement).getPropertyValue("--k-series-" + n).trim() || "currentColor";
  }

  let tipEl = null;
  function tip(el, content) {
    const show = (x, y) => {
      const html = typeof content === "function" ? content() : content;
      if (!html) return hide();
      if (!tipEl) { tipEl = document.createElement("div"); tipEl.className = "k-tip"; tipEl.setAttribute("role", "tooltip"); document.body.appendChild(tipEl); }
      tipEl.innerHTML = html;
      const r = tipEl.getBoundingClientRect();
      const left = Math.min(Math.max(4, x + 12), window.innerWidth - r.width - 4);
      const top = y + 16 + r.height > window.innerHeight ? y - r.height - 8 : y + 16;
      tipEl.style.left = left + "px";
      tipEl.style.top = Math.max(4, top) + "px";
      tipEl.classList.add("k-on");
    };
    const hide = () => { if (tipEl) tipEl.classList.remove("k-on"); };
    el.addEventListener("pointermove", (e) => show(e.clientX, e.clientY));
    el.addEventListener("pointerleave", hide);
    el.addEventListener("focus", () => { const r = el.getBoundingClientRect(); show(r.left, r.bottom - 12); });
    el.addEventListener("blur", hide);
    return { show, hide };
  }

  // an SVG with its colors resolved (the CSS variables a file outside the view cannot read)
  function standalone(svg) {
    const copy = svg.cloneNode(true);
    const src = svg.querySelectorAll("*");
    const dst = copy.querySelectorAll("*");
    const props = ["fill", "stroke", "stroke-width", "stroke-dasharray", "opacity", "fill-opacity", "stroke-opacity", "font-family", "font-size", "font-weight", "text-anchor", "dominant-baseline"];
    src.forEach((n, i) => {
      const cs = getComputedStyle(n);
      for (const p of props) { const v = cs.getPropertyValue(p); if (v) dst[i].setAttribute(p, v); }
    });
    const box = svg.getBoundingClientRect();
    copy.setAttribute("xmlns", "http://www.w3.org/2000/svg");
    if (!copy.getAttribute("width")) copy.setAttribute("width", String(Math.ceil(box.width)));
    if (!copy.getAttribute("height")) copy.setAttribute("height", String(Math.ceil(box.height)));
    const bg = getComputedStyle(document.body).backgroundColor;
    const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
    rect.setAttribute("width", "100%"); rect.setAttribute("height", "100%"); rect.setAttribute("fill", bg);
    copy.insertBefore(rect, copy.firstChild);
    return { text: new XMLSerializer().serializeToString(copy), width: Math.ceil(box.width), height: Math.ceil(box.height) };
  }

  const file = (name, mimeType, blob) => [{ type: "resource", resource: { uri: "file:///" + name, mimeType, ...blob } }];

  function saveSvg(svg, name) {
    const { text } = standalone(svg);
    return app.download(file(name.endsWith(".svg") ? name : name + ".svg", "image/svg+xml", { text }));
  }

  function savePng(svg, name, scale) {
    const { text, width, height } = standalone(svg);
    const k = scale || 2;
    return new Promise((resolve, reject) => {
      const img = new Image();
      img.onload = () => {
        const c = document.createElement("canvas");
        c.width = width * k; c.height = height * k;
        const g = c.getContext("2d");
        g.scale(k, k);
        g.drawImage(img, 0, 0, width, height);
        const blob = c.toDataURL("image/png").split(",")[1];
        app.download(file(name.endsWith(".png") ? name : name + ".png", "image/png", { blob })).then(resolve, reject);
      };
      img.onerror = () => reject(new Error("the image could not be drawn"));
      img.src = "data:image/svg+xml;charset=utf-8," + encodeURIComponent(text);
    });
  }

  function fmt(n, opts) {
    if (typeof n !== "number" || !isFinite(n)) return String(n);
    const locale = ctx.locale || undefined;
    return new Intl.NumberFormat(locale, { notation: Math.abs(n) >= 10000 ? "compact" : "standard", maximumFractionDigits: 2, ...opts }).format(n);
  }

  // a menu of a few actions under a button: opens below it (flips up at the bottom edge),
  // ↑ ↓ ⏎ esc, closes on an outside click or a pick. Items are text (textContent, never html).
  let openMenu = null;
  function menu(button, items) {
    button.setAttribute("aria-haspopup", "menu");
    button.setAttribute("aria-expanded", "false");
    const close = (refocus) => {
      if (!openMenu || openMenu.button !== button) return;
      openMenu.el.remove();
      document.removeEventListener("pointerdown", openMenu.outside, true);
      openMenu = null;
      button.setAttribute("aria-expanded", "false");
      if (refocus) button.focus();
    };
    const open = () => {
      if (openMenu) openMenu.close(false);
      const el = document.createElement("div");
      el.className = "k-menu";
      el.setAttribute("role", "menu");
      const rows = items.map((it) => {
        const b = document.createElement("button");
        b.type = "button";
        b.setAttribute("role", "menuitem");
        b.tabIndex = -1;
        b.textContent = it.label;
        if (it.testid) b.dataset.testid = it.testid;
        b.addEventListener("click", () => { close(true); it.run(); });
        el.appendChild(b);
        return b;
      });
      el.addEventListener("keydown", (e) => {
        const i = rows.indexOf(document.activeElement);
        if (e.key === "ArrowDown") rows[(i + 1) % rows.length].focus();
        else if (e.key === "ArrowUp") rows[(i - 1 + rows.length) % rows.length].focus();
        else if (e.key === "Escape" || e.key === "Tab") close(true);
        else if (e.key === "Home") rows[0].focus();
        else if (e.key === "End") rows[rows.length - 1].focus();
        else return;
        e.preventDefault();
      });
      document.body.appendChild(el);
      const r = button.getBoundingClientRect(), m = el.getBoundingClientRect();
      const gap = 4;
      const below = r.bottom + gap + m.height <= window.innerHeight || r.top - gap - m.height < 0;
      el.style.top = (below ? r.bottom + gap : r.top - gap - m.height) + "px";
      el.style.left = Math.max(gap, Math.min(r.right - m.width, window.innerWidth - m.width - gap)) + "px";
      const outside = (e) => { if (!el.contains(e.target) && !button.contains(e.target)) close(false); };
      document.addEventListener("pointerdown", outside, true);
      openMenu = { button, el, outside, close };
      button.setAttribute("aria-expanded", "true");
      rows[0].focus();
    };
    button.addEventListener("click", () => (openMenu && openMenu.button === button ? close(true) : open()));
    button.addEventListener("keydown", (e) => { if (e.key === "ArrowDown" || e.key === "ArrowUp") { e.preventDefault(); open(); } });
    return { open, close: () => close(false) };
  }

  window.kit = { app, esc, series, tip, saveSvg, savePng, fmt, menu };
})();
