// ⌘F inside an artifact (artifact-find, his m_52682): the find agent put in each artifact frame,
// next to guard.ts's ESC_FORWARD and pinch.ts's PINCH_FORWARD. Every artifact is a pages-server
// frame (a bise page, a site, the viewer of a doc or a markdown file, a text file as the browser
// draws it), most of them on an opaque origin, so the window cannot read their words: the window's
// find bar (artifacts/FindBar.tsx) says the needle and the current match, this answers the count.
//   - {bise: find, q, at, seq} from its parent: the matches of q (case-insensitive, the visible
//     words only, a match may span text nodes) in its own document first, then in each of its own
//     frames (the viewer frames a site: it asks them the same, they answer {bise: found}); match
//     `at` is the current one (-1: none), shown and scrolled to, in this document or in the frame
//     that holds it; it answers {bise: found, seq, n} to its parent;
//   - q '' clears. The matches are CSS highlights (the page's DOM stays as drawn), styled by a
//     constructed sheet (no inline style: a page's CSP may forbid one).
// Once per document; a newer find wins over one still waiting for its frames. Owner: artifact-find.
import { FRAME_KINDS } from '../shared/frame';

/** How long a frame waits for one of its own frames' count (one with no agent counts 0). */
export const KID_WAIT_MS = 400;

export const FIND_AGENT = `(() => {
  if (window.__biseFind) return; window.__biseFind = true;
  const K = ${JSON.stringify({ find: FRAME_KINDS.find, found: FRAME_KINDS.found })};
  const SKIP = new Set(['SCRIPT', 'STYLE', 'NOSCRIPT', 'TEMPLATE', 'IFRAME', 'OBJECT', 'TITLE', 'HEAD']);
  const reg = typeof CSS !== 'undefined' && CSS.highlights !== undefined && typeof Highlight === 'function' ? CSS.highlights : null;
  let styled = false;
  const style = () => {
    if (styled) return;
    styled = true;
    try {
      const s = new CSSStyleSheet();
      s.replaceSync('::highlight(bise-find){background-color:#ffe066;color:#1d1d1f}::highlight(bise-find-at){background-color:#ff9f0a;color:#1d1d1f}');
      document.adoptedStyleSheets = [...document.adoptedStyleSheets, s];
    } catch (_) {}
  };
  const ranges = (q) => {
    const root = document.body;
    if (q === '' || root === null) return [];
    const seen = new Map();
    const visible = (el) => {
      let v = seen.get(el);
      if (v === undefined) {
        v = typeof el.checkVisibility === 'function' ? el.checkVisibility() : true;
        seen.set(el, v);
      }
      return v;
    };
    const nodes = [], starts = [];
    let text = '';
    const walk = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    for (let n = walk.nextNode(); n !== null; n = walk.nextNode()) {
      const p = n.parentElement;
      if (p === null || SKIP.has(p.tagName.toUpperCase()) || p.closest('script,style,noscript,template') !== null || !visible(p)) continue;
      starts.push(text.length);
      nodes.push(n);
      text += n.data;
    }
    const hay = text.toLowerCase(), needle = q.toLowerCase(), out = [];
    // the node and offset at a place in the joined words (an end: the node it ends in)
    const at = (pos, end) => {
      let lo = 0, hi = nodes.length - 1;
      while (lo < hi) {
        const m = (lo + hi + 1) >> 1;
        if (end ? starts[m] < pos : starts[m] <= pos) lo = m; else hi = m - 1;
      }
      return [nodes[lo], pos - starts[lo]];
    };
    for (let k = hay.indexOf(needle); k !== -1 && out.length < 10000; k = hay.indexOf(needle, k + needle.length)) {
      const a = at(k, false), b = at(k + needle.length, true);
      const r = document.createRange();
      r.setStart(a[0], a[1]);
      r.setEnd(b[0], b[1]);
      out.push(r);
    }
    return out;
  };
  const paint = (rs, cur) => {
    if (reg === null) return;
    style();
    const all = new Highlight(), now = new Highlight();
    rs.forEach((r, i) => { (i === cur ? now : all).add(r); });
    reg.set('bise-find', all);
    reg.set('bise-find-at', now);
  };
  // the current match in the middle of each box that scrolls it, then of the page
  const reveal = (r, clear) => {
    const el = r.startContainer.parentElement;
    if (el === null) return;
    for (let p = el; p !== null && p !== document.body && p !== document.documentElement; p = p.parentElement) {
      const cs = getComputedStyle(p), b = r.getBoundingClientRect(), pb = p.getBoundingClientRect();
      if (p.scrollHeight > p.clientHeight + 1 && /auto|scroll/.test(cs.overflowY) && (b.top < pb.top || b.bottom > pb.bottom)) p.scrollTop += b.top - pb.top - p.clientHeight / 2;
      if (p.scrollWidth > p.clientWidth + 1 && /auto|scroll/.test(cs.overflowX) && (b.left < pb.left || b.right > pb.right)) p.scrollLeft += b.left - pb.left - p.clientWidth / 2;
    }
    const b = r.getBoundingClientRect();
    if (b.top < 0 || b.bottom > innerHeight) scrollBy(0, b.top - innerHeight / 2);
    if (b.left < 0 || b.right > innerWidth) scrollBy(b.left - innerWidth / 2, 0);
    // under the window's bar (clear: its bottom and left in this frame's px): 8 px below it
    if (clear === null) return;
    const c = r.getBoundingClientRect();
    if (c.top < clear.y + 8 && c.bottom > 0 && c.right > clear.x) scrollBy(0, c.top - clear.y - 8);
  };
  // where the window's bar sits over this frame ({x, y}: its left and bottom), null: nowhere
  const clearOf = (v) => (v !== null && typeof v === 'object' && typeof v.x === 'number' && typeof v.y === 'number' ? { x: v.x, y: v.y } : null);
  // the bottom of the page's own head pinned at its top (a bise page's frame: its title, 'v6', 'send'), 0: none
  const headOf = () => {
    const h = document.querySelector('.bise-frame');
    if (h === null) return 0;
    const b = h.getBoundingClientRect();
    return b.height > 0 && b.top < 40 ? Math.round(b.bottom) : 0;
  };
  // one of its own frames' count for q (no answer in time: 0)
  const ask = (w, q, seq) => new Promise((done) => {
    const on = (e) => {
      const d = e.data;
      if (e.source !== w || d === null || typeof d !== 'object' || d.bise !== K.found || d.seq !== seq) return;
      clearTimeout(t);
      removeEventListener('message', on);
      done(typeof d.n === 'number' && d.n >= 0 ? d.n : 0);
    };
    const t = setTimeout(() => { removeEventListener('message', on); done(0); }, ${String(KID_WAIT_MS)});
    addEventListener('message', on);
    w.postMessage({ bise: K.find, q, at: -1, seq }, '*');
  });
  let latest = 0;
  addEventListener('message', async (e) => {
    const d = e.data;
    if (e.source !== parent || d === null || typeof d !== 'object' || d.bise !== K.find) return;
    const q = typeof d.q === 'string' ? d.q : '';
    const cur = Number.isInteger(d.at) ? d.at : -1;
    const seq = d.seq;
    const clear = clearOf(d.clear);
    const mine = ++latest;
    const rs = ranges(q);
    const frames = Array.from(document.querySelectorAll('iframe')).filter((f) => f.contentWindow !== null);
    const ns = await Promise.all(frames.map((f) => ask(f.contentWindow, q, seq)));
    if (mine !== latest) return;
    paint(rs, cur);
    if (cur >= 0 && cur < rs.length) reveal(rs[cur], clear);
    let k = cur - rs.length;
    frames.forEach((f, i) => {
      const n = ns[i];
      if (k >= 0 && k < n) {
        f.scrollIntoView({ block: 'nearest', inline: 'nearest' });
        const fb = f.getBoundingClientRect();
        const inner = clear === null ? null : { x: clear.x - fb.left, y: clear.y - fb.top };
        if (f.contentWindow !== null) f.contentWindow.postMessage({ bise: K.find, q, at: k, seq, clear: inner }, '*');
      }
      k -= n;
    });
    parent.postMessage({ bise: K.found, seq, n: rs.length + ns.reduce((a, b) => a + b, 0), head: headOf() }, '*');
  });
})();`;
