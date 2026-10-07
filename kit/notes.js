// bise kit · the note layer (docs/ambient-pages.md §2.7, docs/ambient-vision.md §3 C-D, §5).
// On a page the agent made, you leave notes: a pin, a typed note on a block or on a selection
// (anchored by the block's data-id and the quoted text), ♡ keep · ✗ drop · ~ not sure, an edit in
// place on prose (a note with before/after, undone with ⌘z). The side list shows them; nothing goes
// until `send` (the button, ⌘⏎). Drafts are saved to the page server, so a reload loses nothing.
// When kit.js swaps in the next version (`bise:swapped`), notes re-anchor (`moved` when their
// block or quote is gone) and show what happened to them (done, the agent's answer, open).
//
// Two halves: `BiseNotes.core` is pure (the words, anchoring, the notes model: node checks run it)
// and `mount()` is the DOM (the page's, under the shell's CSP: no
// inline style, only CSSOM for the pins' positions). Classes are `bn-*`; looks come from the kit's
// role tokens (notes.css).
(function (root) {
  "use strict";

  // ---- the words (ambient's; a new line goes through ambient)
  const NW = {
    listTitle: (page, n) => `notes for ${page} · ${n}`,
    sendBar: (n, agent) => `${n} ${n === 1 ? "note" : "notes"} for ${agent} · or just say "send"`,
    send: "send",
    count: (n) => (n ? `${n} ${n === 1 ? "note" : "notes"}` : ""),
    sent: (agent) => `sent. ${agent} is on it.`,
    updating: (agent) => `${agent} is updating this page…`,
    moved: "moved",
    done: "✓ done",
    open: "open",
    sentTag: "sent",
    // signed by ambient (m_5007): changed only when both sides have words; the place's words for
    // an addition or a removal (QA m_5008 (11); best guess, asked to ambient)
    changed: (before, after) => (!before ? `you added "${after}"` : !after ? `you removed "${before}"` : `you changed "${before}" → "${after}"`),
    addedAt: (words, side, ctx) => `you added "${words}" ${side} "${ctx}"`,
    removedAt: (words, side, ctx) => `you removed "${words}" ${side} "${ctx}"`,
    // an edit of yours the new version kept (QA m_5008 (12))
    kept: "✓ kept",
    // taste (roadmap §3.4): a note that states a preference is offered once for next time
    // (best guess, asked to ambient)
    taste: "keep this for next time?",
    tasteYes: "yes",
    tasteNo: "no",
    tasteKept: "kept for next time",
    // slice C: start an agent on a review's item (main starts it; best guess, asked to ambient)
    start: "start an agent",
    // roadmap B: the watching frame's 'stop' (best guesses, asked to ambient)
    stop: "stop watching",
    stopped: "stopped. bise won't check this page again.",
    marks: { keep: "♡ keep", drop: "✗ drop", unsure: "~ not sure" },
    // approve / skip on a review's item or an email (§4.3; kit.js's buttons say the same words)
    reacts: { approve: "approve", skip: "skip", pick: "picked", tick: "ticked" },
    // a voice note's pin while you talk (the mocks' `you · voice`; ambient, m_4877)
    voiceBy: "you · voice",
    // a drawing in the side list, and the hint while you draw (§5.4; asked to ambient)
    // signed by ambient (m_4926): esc still ends it, it just doesn't need saying
    drawing: "✎ drawing",
    drawHint: "draw on the page   d done",
    // the agent's answer to a note is a reply (ambient, m_4919 (3)): its name dim, its words, then yours
    replyBy: (agent) => `${agent}:`,
    replyField: "reply, or hold fn and say it",
    // a question's option you picked (ambient, m_4795)
    picked: (option, text) => (text ? `you picked ${option}: ${text}` : `you picked ${option}`),
    tools: { note: "note", keep: "♡", drop: "✗", unsure: "~", edit: "edit", pin: "pin", start: "start an agent" },
    // what VoiceOver says for the toolbar's buttons and the layer's parts (best guesses, asked to ambient)
    say: { note: "write a note", keep: "keep", drop: "drop", unsure: "not sure", edit: "edit the text", start: "start an agent", list: "your notes", tools: "note tools" },
    placeholder: "your note",
    // signed off by ambient (m_4786): 'block' is our word, not hers; keys like the TUI's key bar
    empty: "no notes yet. select some text, or hold fn and talk.",
    // on a page with a k-diff, the new gesture too (ambient m_7871)
    emptyLines: "no notes yet. select some text, click a line's number, or hold fn and talk.",
    // the page is being written, no version yet (ambient, m_5095 via amb-kit m_5106)
    writing: "notes open with the first version",
    keys: "t type   p pin   d draw   ⌘z undo   ⌘⏎ send",
    // the same without a mouse, on the block tab reached (ambient-lead m_5651; best guess, asked to ambient)
    keysTab: "tab next block   n note   h ♡   x ✗   e edit",
    remove: "remove",
    undo: "undo",
    failed: "couldn't save your notes. trying again.",
  };

  // agents' ids are kebab-case; on your surfaces they read with spaces (vision §3 B)
  const spaced = (id) => String(id || "").replace(/-/g, " ");

  // ---- anchoring: on the block's text with its whitespace collapsed (what you see)
  const norm = (s) => String(s || "").replace(/\s+/g, " ").trim();

  // where `quote` sits in `text` (both normalized): the occurrence nearest `hint` (its old offset)
  function locate(text, quote, hint) {
    const t = norm(text), q = norm(quote);
    if (!q) return null;
    let best = null, at = t.indexOf(q);
    while (at >= 0) {
      if (best == null || Math.abs(at - (hint || 0)) < Math.abs(best - (hint || 0))) best = at;
      at = t.indexOf(q, at + 1);
    }
    return best == null ? null : { start: best, end: best + q.length };
  }

  // a note against the blocks of a version ({id: text}): is it still where it was?
  function anchorOf(note, blocks) {
    if (!note.block) return "ok";                         // a page-wide note
    // a drawing holds while one of the blocks under it is still there
    if (note.kind === "draw") return (note.blocks || [note.block]).some((id) => id in blocks) ? "ok" : "moved";
    if (!(note.block in blocks)) return "moved";          // its block is gone
    const text = blocks[note.block];
    if (note.kind === "edit") {
      // your edit holds if your words are there (kept, as the agent should), or the old ones are
      if (note.after && locate(text, note.after)) return "ok";
      if (note.before && locate(text, note.before)) return "ok";
      return "moved";
    }
    if (note.quote && !locate(text, note.quote, note.offset)) return "moved";
    return "ok";
  }
  // every note, re-anchored on a new version's blocks
  // an edit is kept when its words are in the page, in its block or anywhere (the agent may have
  // moved or renamed the block): QA m_5008 (12)
  const keptEdit = (n, blocks) => !!(n.after && Object.values(blocks).some((t) => locate(t, n.after)));
  const reanchor = (notes, blocks) => notes.map((n) => {
    if (n.kind !== "edit") return { ...n, anchor: anchorOf(n, blocks) };
    const kept = keptEdit(n, blocks);
    return { ...n, anchor: kept ? "ok" : anchorOf(n, blocks), kept };
  });

  // ---- the words nearest a point in a block's text (a voice note's or a pin's anchor): the word
  // at `offset` and its neighbours, at most `n` words, never across a sentence's end
  function nearestQuote(text, offset, n = 5) {
    const t = norm(text);
    if (!t) return null;
    const words = [];
    const re = /\S+/g;
    let m;
    while ((m = re.exec(t))) words.push({ w: m[0], at: m.index });
    if (!words.length) return null;
    let i = words.findIndex((x) => x.at + x.w.length >= offset);
    if (i < 0) i = words.length - 1;
    let a = i, b = i;
    const ends = (k) => /[.!?:;]$/.test(words[k].w);
    while (b - a + 1 < n) {
      const grew = b - a;
      if (b + 1 < words.length && !ends(b)) b++;
      if (b - a + 1 < n && a > 0 && !ends(a - 1)) a--;
      if (b - a === grew) break;
    }
    return { quote: words.slice(a, b + 1).map((x) => x.w).join(" "), offset: words[a].at };
  }

  // ---- an edit: the smallest changed span, by words (`you changed "Q2" → "Q3"`)
  function diffWords(before, after) {
    const a = norm(before).split(" "), b = norm(after).split(" ");
    let p = 0;
    while (p < a.length && p < b.length && a[p] === b[p]) p++;
    let s = 0;
    while (s < a.length - p && s < b.length - p && a[a.length - 1 - s] === b[b.length - 1 - s]) s++;
    return { before: a.slice(p, a.length - s).join(" "), after: b.slice(p, b.length - s).join(" "), at: a.slice(0, p).join(" ").length + (p ? 1 : 0), p, s };
  }
  // an edit's note: the changed span; words only added or only removed carry the words around
  // them (QA m_5008 (11)), so the note reads `you added "(or not)" after "day 1"` and the agent
  // (and a new version's re-anchor) can find the place
  function editSpan(first, edited) {
    const d = diffWords(first, edited);
    if (d.before && d.after) return { before: d.before, after: d.after };
    if (!d.before && !d.after) return { before: "", after: "" };
    const a = norm(first).split(" ").filter(Boolean), b = norm(edited).split(" ").filter(Boolean);
    const k = 2;
    // the words just before the change, else (at the very start) the words just after it
    const lead = a.slice(Math.max(0, d.p - k), d.p);
    const tail = lead.length ? [] : a.slice(a.length - d.s, a.length - d.s + k);
    const ctx = (lead.length ? lead : tail).join(" ");
    const wrap = (mid) => [...lead, ...(mid ? [mid] : []), ...tail].join(" ");
    return {
      before: wrap(d.before), after: wrap(d.after),
      ...(d.after ? { added: d.after } : { removed: d.before }),
      ...(ctx ? { ctx, side: lead.length ? "after" : "before" } : {}),
    };
  }

  // ---- what happened to a note: the agent's outcome first, then where it sits
  function outcome(n) {
    if (n.status === "done") return { tag: "done", text: NW.done };
    if (n.status === "answered") return { tag: "answer", text: n.answer || "" };
    if (n.status === "sent") {
      const newer = n.version_seen && n.version_seen > n.version;
      // your edit, still in the new version: kept (QA m_5008 (12))
      if (newer && n.kind === "edit" && n.kept) return { tag: "done", text: NW.kept };
      return newer ? { tag: "open", text: NW.open } : { tag: "sent", text: NW.sentTag };
    }
    return { tag: "draft", text: "" };
  }
  // the side list's tag: an answered or done note always shows its outcome, anchored or not;
  // `moved` is only for a note still open that lost its place (QA m_5008 (10))
  function tagOf(n) {
    const o = outcome(n);
    if (n.anchor === "moved" && (n.status === "draft" || n.status === "sent") && o.text !== NW.kept) return { tag: "moved", text: NW.moved };
    return o;
  }

  // where a note sits, in the page's words, never a block id (ambient, m_4919 (2)): the quoted
  // words when there was a selection, else the block's name (its kicker or heading)
  // words when there was a selection, else the block's name (its kicker or heading); on a ui
  // block's rows, the lines (`lines −34–36 +34 · main`, notelines.js)
  const where = (n, name) => (root.BiseLines && root.BiseLines.where(n)) || (n.quote ? `"${n.quote}"` : name || n.where || "");
  // a block's name from what it shows: its own heading or kicker (a heading, a callout's h3), else
  // the heading of its section (the heading block above it), else its first words
  function blockName(own, section, text) {
    const cut = (s, k) => { const w = norm(s).split(" ").filter(Boolean); return w.length > k ? `${w.slice(0, k).join(" ")}…` : w.join(" "); };
    return norm(own) ? cut(own, 8) : norm(section) ? cut(section, 8) : cut(text, 6);
  }

  // a note's one line in the side list
  function label(n) {
    if (n.kind === "edit") {
      if (n.added && n.ctx) return NW.addedAt(n.added, n.side || "after", n.ctx);
      if (n.removed && n.ctx) return NW.removedAt(n.removed, n.side || "after", n.ctx);
      return NW.changed(n.before || "", n.after || "");
    }
    // a mark on a compare's variant (kit.js's reaction, item set): the place line is the variant's
    // label, the note reads just '♡ keep' (its words stay on the note for the agent; QA m_5280 (17))
    if (NW.marks[n.kind]) return n.text && !n.item ? `${NW.marks[n.kind]}: ${n.text}` : NW.marks[n.kind];
    if (n.kind === "pick" && n.option != null) return NW.picked(n.option, n.text);
    if (n.kind === "draw") return n.text ? `${NW.drawing}: ${n.text}` : NW.drawing;
    // the item's own words are long (a whole mail): its place line names it, the note says the verdict
    if (NW.reacts[n.kind]) return NW.reacts[n.kind];
    if (n.kind === "start") return NW.start;
    if (n.kind === "stop") return NW.stop;
    return n.text || "";
  }

  // ---- taste (roadmap §3.4): a note that states a preference ('never …', 'too long', 'no emoji',
  // 'jamais', 'trop long'...): the layer offers once to keep it for next time
  const TASTE = /\b(never|always|too (long|short|much|many|formal|casual)|no emojis?|shorter|longer|jamais|toujours|trop (long|court|formel)|plus court|pas d'?emojis?)\b/i;
  const tastes = (text) => TASTE.test(String(text || ""));
  // offered under a draft whose words state a preference, until you say yes or no (taste set)
  const offerTaste = (n) => n.status === "draft" && !n.listening && n.taste == null && tastes(n.text);

  // ---- the notes model: drafts are yours until `send`; the server's statuses win for the rest
  const STATUS = ["draft", "sent", "done", "answered"];
  // kit.js's reactions (bise:react) and which ones replace each other on an item
  const GROUP = { approve: "verdict", skip: "verdict", keep: "mark", drop: "mark", pick: "pick", tick: "tick" };
  const REACTS = Object.keys(GROUP);
  // every note gets its own id here: the server numbers id-less notes n<max+1>, so a removed
  // draft's number could come back (ambient-lead, m_4805). `n` + time + random, base 36.
  const newId = (now) => `n${Math.floor(now || Date.now()).toString(36)}${Math.random().toString(36).slice(2, 6)}`;
  function model(init, opts) {
    let notes = (init || []).map((n) => ({ ...n }));
    const undo = [];          // edits and removals, newest last: {type, note}
    const idOf = (opts && opts.id) || newId;
    const api = {
      all: () => notes.slice(),
      // a voice note still listening is not a draft yet (not counted, not saved)
      drafts: () => notes.filter((n) => n.status === "draft" && !n.listening),
      listening: () => notes.find((n) => n.listening) || null,
      // a voice note at the mouse (§4.1): start pins it listening, heard fills it, end saves it as a
      // draft (empty words: it goes), cancel removes it; one at a time
      voiceStart(anchor, version, now) {
        api.voiceCancel();
        return api.add({ version, ...anchor, kind: "voice", text: "", listening: true }, now);
      },
      voiceHeard(text) {
        const v = api.listening();
        if (v) notes = notes.map((n) => (n === v ? { ...n, text: text || "" } : n));
        return api.listening();
      },
      voiceEnd(text, now) {
        const v = api.listening();
        if (!v) return null;
        const words = norm(text != null ? text : v.text);
        if (!words) { notes = notes.filter((n) => n !== v); return null; }
        const done = { ...v, text: words, at_ms: now || v.at_ms };
        delete done.listening;
        notes = notes.map((n) => (n === v ? done : n));
        return done;
      },
      voiceCancel() {
        const v = api.listening();
        if (v) notes = notes.filter((n) => n !== v);
        return v;
      },
      get: (id) => notes.find((n) => n.id === id) || null,
      nextId: (now) => { let id; do id = idOf(now); while (api.get(id)); return id; },
      add(n, now) {
        const note = { id: api.nextId(now), status: "draft", at_ms: now || 0, ...n };
        notes.push(note);
        if (note.kind === "edit") undo.push({ type: "add", note });
        return note;
      },
      update(id, patch) {
        notes = notes.map((n) => (n.id === id && n.status === "draft" ? { ...n, ...patch } : n));
        return api.get(id);
      },
      remove(id) {
        const n = api.get(id);
        if (!n || n.status !== "draft") return null;
        notes = notes.filter((x) => x.id !== id);
        undo.push({ type: "remove", note: n });
        return n;
      },
      // ⌘z: the last edit or removal; returns what to redo on the page
      undo() {
        const u = undo.pop();
        if (!u) return null;
        if (u.type === "add") notes = notes.filter((x) => x.id !== u.note.id);
        else notes.push(u.note);
        return u;
      },
      // an edit on a block (or a review's item) already edited: one note per block or item, its
      // before kept from the first
      // `span` is editSpan's (before/after, and added/removed with the words around them): from the
      // block's first text each time, so a later edit replaces the whole span
      edit(block, before, after, now, item, span) {
        const prev = notes.find((n) => n.status === "draft" && n.kind === "edit" && n.block === block && (n.item || null) === (item || null));
        const extra = span ? { added: span.added, removed: span.removed, ctx: span.ctx, side: span.side, ...(span.version ? { version: span.version } : {}) } : {};
        if (prev) {
          if (norm(prev.before) === norm(after) || norm(before) === norm(after)) { api.remove(prev.id); undo.pop(); return null; }
          return api.update(prev.id, { ...(span ? { before } : {}), after, ...extra, at_ms: now || prev.at_ms });
        }
        if (norm(before) === norm(after)) return null;
        return api.add({ kind: "edit", block, ...(item ? { item } : {}), before, after, ...extra }, now);
      },
      // approve / skip on a review's item or an email (kit.js's bise:react, §4.3): one per item,
      // the latest wins; kind null (the same again) takes it back
      // (amb-kit m_5018: also keep / drop and pick on a compare's variants, tick on a checklist's
      // items.) One reaction per item and group: approve/skip, keep/drop, tick; a pick is one per
      // block (the last wins, on any variant). kind null takes back the item's reaction.
      react(block, item, kind, text, version, now) {
        const same = (n) => n.status === "draft" && REACTS.includes(n.kind) && n.block === block;
        const onItem = (n) => same(n) && (n.item || null) === (item || null);
        if (!REACTS.includes(kind)) { notes = notes.filter((n) => !(onItem(n) && n.kind !== "pick") && !(onItem(n) && kind == null && n.kind === "pick")); return null; }
        const prev = kind === "pick" ? notes.find((n) => same(n) && n.kind === "pick") : notes.find((n) => onItem(n) && GROUP[n.kind] === GROUP[kind]);
        if (prev) return api.update(prev.id, { kind, ...(item ? { item } : {}), text: text || undefined, at_ms: now || prev.at_ms });
        return api.add({ version, block, ...(item ? { item } : {}), kind, text: text || undefined }, now);
      },
      // start an agent on a review's item (slice C): one per item, the same again takes it back;
      // amb-core routes start notes to main, the only one that starts agents
      start(block, item, text, version, now) {
        const prev = notes.find((n) => n.status === "draft" && n.kind === "start" && n.block === block && (n.item || null) === (item || null));
        if (prev) { notes = notes.filter((x) => x !== prev); return null; }
        return api.add({ version, block, ...(item ? { item } : {}), kind: "start", text: text || undefined }, now);
      },
      // the same start note from a button that only starts (bise:start): never a take-back; a draft
      // start already on that item is the one sent
      startNow(block, item, text, version, now) {
        const prev = notes.find((n) => n.status === "draft" && n.kind === "start" && n.block === block && (n.item || null) === (item || null));
        return prev || api.add({ version, block, ...(item ? { item } : {}), kind: "start", text: text || undefined }, now);
      },
      // stop a watching page (roadmap B, kit.js's bise:stop {page, timer}; amb-home m_5458): one
      // stop note, to main, on any block of the page (the hub stops the page's timers); a second
      // click while it's still a draft is the same note
      stop(block, timer, version, now) {
        const prev = notes.find((n) => n.status === "draft" && n.kind === "stop");
        if (prev) return prev;
        return api.add({ version, block, kind: "stop", ...(timer != null ? { timer } : {}) }, now);
      },
      // a question's option picked on the page (kit.js's bise:pick): one pick per block, the
      // latest wins; picking the same option again takes it back
      pick(block, option, text, version, now) {
        const prev = notes.find((n) => n.status === "draft" && n.kind === "pick" && n.block === block);
        // option null: kit.js took the pick back
        if (option == null || (prev && String(prev.option) === String(option))) { if (prev) notes = notes.filter((x) => x !== prev); return null; }
        if (prev) return api.update(prev.id, { option, text, at_ms: now || prev.at_ms });
        return api.add({ version, block, kind: "pick", option, text }, now);
      },
      // a question answered elsewhere (the capsule, the TUI: §4.2): one card, one answer, so a
      // draft pick on that block goes
      answered(block) {
        const had = notes.some((n) => n.status === "draft" && n.kind === "pick" && n.block === block);
        notes = notes.filter((n) => !(n.status === "draft" && n.kind === "pick" && n.block === block));
        return had;
      },
      // a checklist row ticked by its agent (SSE ticked, kit.js's bise:ticked; roadmap D, amb-kit
      // 74539129): it's done, so a draft tick of ours on that row has nothing left to say
      ticked(block, item) {
        const mine = (n) => n.status === "draft" && n.kind === "tick" && n.block === block && (n.item || null) === (item || null);
        const had = notes.some(mine);
        notes = notes.filter((n) => !mine(n));
        return had;
      },
      // the server's notes (meta, SSE): its statuses and answers win; drafts it doesn't know stay
      merge(server) {
        const by = Object.fromEntries((server || []).map((n) => [n.id, n]));
        notes = notes.map((n) => (by[n.id] && STATUS.indexOf(by[n.id].status) >= STATUS.indexOf(n.status) ? { ...n, ...by[n.id], anchor: n.anchor } : n));
        for (const s of server || []) if (!notes.some((n) => n.id === s.id)) notes.push({ ...s });
      },
      // sent: the drafts are now the agent's
      sent(now) { notes = notes.map((n) => (n.status === "draft" ? { ...n, status: "sent", sent_ms: now || 0 } : n)); undo.length = 0; },
      // one note sent alone (the watching frame's stop: a button does only what it names); the
      // other drafts stay drafts, and their undo too
      sentOne(id, now) { notes = notes.map((n) => (n.id === id && n.status === "draft" ? { ...n, status: "sent", sent_ms: now || 0 } : n)); },
      reanchor(blocks, version) { notes = reanchor(notes, blocks).map((n) => (n.status === "sent" ? { ...n, version_seen: version } : n)); },
      // what the server keeps: the drafts, without the page's own fields
      payload: (only) => ({ notes: api.drafts().filter((n) => !only || n.id === only).map(({ anchor, version_seen, listening, ...n }) => n) }),
    };
    return api;
  }

  // ---- drawing (§5.4): strokes in the page's own coordinates (from #bise-page's top left, so
  // scrolling never moves them), one note per drawing: its path, its box, the blocks under it
  // one stroke as an SVG path: points closer than 1.5 px to the last kept one are dropped
  function strokePath(points) {
    const kept = [];
    for (const [x, y] of points || []) {
      const l = kept[kept.length - 1];
      if (!l || Math.hypot(x - l[0], y - l[1]) >= 1.5) kept.push([x, y]);
    }
    if (!kept.length) return "";
    const f = (v) => String(Math.round(v * 10) / 10);
    if (kept.length === 1) kept.push([kept[0][0] + 0.1, kept[0][1]]);   // a dot
    return kept.map(([x, y], i) => `${i ? "L" : "M"}${f(x)} ${f(y)}`).join(" ");
  }
  // the box around every stroke
  function strokesBox(strokes) {
    let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (const s of strokes || []) for (const [x, y] of s) { x0 = Math.min(x0, x); y0 = Math.min(y0, y); x1 = Math.max(x1, x); y1 = Math.max(y1, y); }
    return x0 === Infinity ? null : { x: Math.round(x0), y: Math.round(y0), w: Math.round(x1 - x0), h: Math.round(y1 - y0) };
  }
  // the blocks a box touches ({id: {x, y, w, h}}), top to bottom
  function blocksIn(box, rects) {
    if (!box) return [];
    return Object.entries(rects || {})
      .filter(([, r]) => r.x < box.x + box.w && box.x < r.x + r.w && r.y < box.y + box.h && box.y < r.y + r.h)
      .sort((a, b) => a[1].y - b[1].y || a[1].x - b[1].x)
      .map(([id]) => id);
  }

  const core = { NW, spaced, norm, locate, anchorOf, reanchor, diffWords, editSpan, outcome, tagOf, label, where, blockName, tastes, offerTaste, model, nearestQuote, strokePath, strokesBox, blocksIn };

  // ======================================================================================
  // the DOM: only on a page the shell served (#bise-page)
  // ======================================================================================

  // the text nodes of a block, and a Range for [start, end) of its normalized text
  function textNodes(el) {
    const out = [];
    const walk = (n) => {
      if (n.nodeType === 3) out.push(n);
      // our layer and the kit's own controls (data-kit-ui: approve / skip...) are not the agent's text
      else if (n.nodeType === 1 && !(n.classList && n.classList.contains("bn-ui")) && !(n.hasAttribute && n.hasAttribute("data-kit-ui"))) for (const c of n.childNodes) walk(c);
    };
    walk(el);
    return out;
  }
  // an element's text as the agent wrote it (what anchors and quotes read)
  const textOf = (el) => (el ? textNodes(el).map((t) => t.nodeValue).join("") : "");
  // a fragment's text without the kit's controls (a selection's)
  function fragText(frag) {
    if (frag.querySelectorAll) for (const u of frag.querySelectorAll("[data-kit-ui], .bn-ui")) u.remove();
    return frag.textContent || "";
  }
  // normalized offset -> (node, offset): walk the raw text, counting collapsed whitespace once
  function rawPoints(el) {
    const pts = [];   // pts[i] = [node, offset] of the i-th normalized char
    let prevSpace = true;
    for (const node of textNodes(el)) {
      const s = node.nodeValue;
      for (let i = 0; i < s.length; i++) {
        const sp = /\s/.test(s[i]);
        if (sp && prevSpace) continue;
        pts.push([node, i]);
        prevSpace = sp;
      }
    }
    // the trailing space norm() trims
    while (pts.length && /\s/.test(pts[pts.length - 1][0].nodeValue[pts[pts.length - 1][1]])) pts.pop();
    return pts;
  }
  function rangeFor(doc, el, start, end) {
    const pts = rawPoints(el);
    if (!pts.length || start >= pts.length) return null;
    const r = doc.createRange();
    const a = pts[start], b = pts[Math.min(end, pts.length) - 1];
    r.setStart(a[0], a[1]);
    r.setEnd(b[0], b[1] + 1);
    return r;
  }
  // a selection inside one block -> {block, quote, offset}, and on a ui block's rows (a k-diff,
  // a k-term) the lines and the part they are in (notelines.js: {lines, more, part})
  function fromSelection(sel, page) {
    if (!sel || sel.rangeCount === 0 || sel.isCollapsed) return null;
    return fromRange(sel.getRangeAt(0), page);
  }
  function fromRange(r, page) {
    const start = r.startContainer.nodeType === 1 ? r.startContainer : r.startContainer.parentElement;
    const block = start && start.closest("[data-kit][data-id]");
    if (!block || !page.contains(block) || !block.contains(r.endContainer)) return null;
    const quote = norm(fragText(r.cloneContents()));
    const lines = block.dataset.kit === "ui" && root.BiseLines ? root.BiseLines.at(r, block, textOf) : {};
    // an empty line picked by its number has no words: its line is the place
    if (!quote && !(lines.lines && lines.lines.length)) return null;
    // the offset: the normalized text before the selection
    const pre = r.cloneRange();
    pre.selectNodeContents(block);
    pre.setEnd(r.startContainer, r.startOffset);
    const before = fragText(pre.cloneContents());
    const offset = norm(before).length + (/\s$/.test(before) && before.trim() ? 1 : 0);
    return { block: block.dataset.id, quote, offset, ...lines };
  }

  function mount(doc) {
    const win = doc.defaultView;
    const page = doc.getElementById("bise-page");
    if (!page || page.dataset.biseNotes) return null;
    page.dataset.biseNotes = "on";
    const pageId = page.dataset.page;
    let version = +page.dataset.version || 1;
    const token = (doc.querySelector('meta[name="bise-token"]') || {}).content || "";
    const meta = { title: doc.title || spaced(pageId), agent: "" };
    const M = model([]);
    const now = () => Date.now();
    const el = (tag, cls, text) => { const e = doc.createElement(tag); if (cls) e.className = cls; if (text != null) e.textContent = text; return e; };
    const blocks = () => [...page.querySelectorAll("[data-kit][data-id]")];
    const blockById = (id) => page.querySelector(`[data-kit][data-id="${CSS.escape(id)}"]`);
    const blockTexts = () => Object.fromEntries(blocks().map((b) => [b.dataset.id, norm(textOf(b))]));
    // kit.js's bise:state; a page still being written (#bise-page[data-state=writing], no version
    // yet) has the note layer off: no toolbar, no pin, no drawing, no keys (amb-kit m_5106)
    let state = page.dataset.state || "ready";
    const writing = () => state === "writing";

    // ---- the server
    const api = (path, body) => win.fetch(path, {
      method: body ? "POST" : "GET",
      headers: body ? { "Content-Type": "application/json", "X-Bise-Token": token } : {},
      body: body ? JSON.stringify(body) : undefined,
      credentials: "same-origin",
    }).then((r) => (r.ok ? r.json() : Promise.reject(new Error(String(r.status)))));
    let saveT = 0, saving = false, dirty = false;
    function save() {
      clearTimeout(saveT);
      saveT = win.setTimeout(async () => {
        if (saving) { dirty = true; return; }
        saving = true;
        try { await api(`/api/p/${pageId}/notes`, M.payload()); bar.classList.remove("bn-error"); }
        catch { bar.classList.add("bn-error"); barMsg(NW.failed); win.setTimeout(save, 3000); }
        saving = false;
        if (dirty) { dirty = false; save(); }
      }, 400);
    }

    // ---- the layer: the side list, the send bar, the toolbar, the composer, the pins
    const list = el("aside", "bn-ui bn-list");
    const listHead = el("div", "bn-head");
    const listBody = el("ol", "bn-items");
    // the keys, each pair whole, flowing in rows: they wrap between pairs, never clipped (ambient m_7871)
    const listKeys = el("div", "bn-keys");
    const keyRow = el("div", "bn-keyrow");
    for (const p of `${NW.keys}   ${NW.keysTab}`.split(/ {3}/)) keyRow.append(el("span", "bn-key", p));
    listKeys.append(keyRow);
    // the composer is the list's last row: a note you type never covers the page (QA m_4929: it
    // opened over the next block)
    const composer = el("form", "bn-composer");
    const composerWhere = el("div", "bn-where");
    const input = el("textarea", "bn-input");
    input.rows = 2; input.placeholder = NW.placeholder;
    composer.append(composerWhere, input);
    list.append(listHead, listBody, composer, listKeys);
    const bar = el("div", "bn-ui bn-bar");
    const barText = el("span", "bn-bar-text");
    const barSend = el("button", "bn-send", NW.send);
    barSend.type = "button";
    bar.append(barText, barSend);
    const tools = el("div", "bn-ui bn-tools");
    const pins = el("div", "bn-ui bn-pins");
    // VoiceOver: the list is a region named 'your notes' with a real list in it, the toolbar a
    // toolbar, the send bar's line read politely when it changes
    list.setAttribute("role", "region"); list.setAttribute("aria-label", NW.say.list);
    tools.setAttribute("role", "toolbar"); tools.setAttribute("aria-label", NW.say.tools);
    barText.setAttribute("aria-live", "polite");
    doc.body.append(list, bar, tools, pins);
    // a line whose leading ✎ or ✓ keeps its own color (the pen's, the olive's): ambient, m_4926, m_4919 (4)
    const glyphed = (tag, cls, text) => {
      const m = /^([✎✓])(\s.*)$/su.exec(text || "");
      if (!m) return el(tag, cls, text);
      const e = el(tag, cls);
      e.append(el("span", "bn-glyph", m[1]), m[2]);
      return e;
    };
    // a block's name, as the page shows it (never its id)
    const heads = "h1, h2, h3, h4";
    function nameOf(id, item) {
      const b = id && blockById(id);
      if (!b) return "";
      // a review's item goes by its first line (the mail's sender and subject), never its id
      const x = item && editEl(id, item);
      if (x) { const first = x.querySelector("p, h3, h4"); return blockName(first ? textOf(first) : "", "", textOf(x)); }
      const own = b.querySelector(heads);
      let section = null;
      for (let p = b.previousElementSibling; p && !section; p = p.previousElementSibling) if (p.dataset && p.dataset.kit === "heading") section = p.querySelector(heads);
      const kit = b.dataset.kit;
      // a heading block is its own name; a callout's kicker is its first h2/h3; a table or a
      // prose block goes by its section
      return blockName(kit === "heading" || kit === "callout" ? own && own.textContent : "", section ? section.textContent : "", textOf(b));
    }
    const frameCount = () => doc.querySelector("[data-bise-notes-count]");
    const frameSend = () => doc.querySelector("[data-bise-send]");

    // ---- highlights: the CSS Custom Highlight API (no change to the agent's DOM)
    const HL = win.CSS && win.CSS.highlights && win.Highlight ? win.CSS.highlights : null;
    function paintHighlights() {
      if (!HL) return;
      const mine = new win.Highlight(), moved = new win.Highlight();
      for (const n of M.all()) {
        if (!n.quote || n.kind === "edit") continue;
        const b = blockById(n.block);
        const at = b && locate(textOf(b), n.quote, n.offset);
        const r = at && rangeFor(doc, b, at.start, at.end);
        if (r) (n.status === "draft" ? mine : moved).add(r);
      }
      HL.set("bise-note", mine);
      HL.set("bise-note-sent", moved);
    }

    // ---- the side list and the counts
    let barNote = "";
    function barMsg(s) { barNote = s; render(); }
    function render() {
      const all = M.all(), drafts = M.drafts().length;
      listHead.textContent = NW.listTitle(meta.title, all.length);
      listBody.replaceChildren(...all.map((n) => {
        const li = el("li", `bn-item bn-${n.status}${n.anchor === "moved" ? " bn-moved" : ""}`);
        li.dataset.note = n.id;
        li.append(el("div", "bn-where", where(n, nameOf(n.block, n.item))), glyphed("div", "bn-what", label(n)));
        const o = tagOf(n);
        if (o.tag === "moved") li.append(el("div", "bn-tag bn-tag-moved", NW.moved));
        else if (o.tag === "answer") {
          // the agent's answer is a reply, and the note stays open: its name dim, its words, then
          // a field for yours (ambient, m_4919 (3))
          const rep = el("div", "bn-reply");
          rep.append(el("span", "bn-reply-by", NW.replyBy(spaced(meta.agent || "main"))), el("span", "bn-reply-text", ` ${o.text}`));
          const field = el("input", "bn-reply-field");
          field.type = "text"; field.placeholder = NW.replyField; field.dataset.replyTo = n.id;
          li.append(rep, field);
        } else if (o.text) li.append(glyphed("div", `bn-tag bn-tag-${o.tag}`, o.text));
        // taste: offered once under a note that states a preference; yes keeps it for next time
        if (offerTaste(n)) {
          const ask = el("div", "bn-taste", NW.taste);
          const b = (v, t) => { const x = el("button", "bn-taste-btn", t); x.type = "button"; x.dataset.taste = v; x.dataset.note = n.id; return x; };
          ask.append(" ", b("yes", NW.tasteYes), b("no", NW.tasteNo));
          li.append(ask);
        } else if (n.taste === true) li.append(el("div", "bn-tag", NW.tasteKept));
        if (n.status === "draft") {
          const x = el("button", "bn-x", "×");
          x.type = "button"; x.title = NW.remove; x.setAttribute("aria-label", NW.remove); x.dataset.remove = n.id;
          li.append(x);
        }
        return li;
      }));
      if (!all.length) listBody.replaceChildren(el("li", "bn-empty", page.querySelector(".k-dl") ? NW.emptyLines : NW.empty));
      // the page is being written (no version yet): notes wait for the first version (ambient, m_5095)
      if (writing()) listBody.replaceChildren(el("li", "bn-empty", NW.writing));
      list.classList.toggle("bn-on", all.length > 0 || composer.classList.contains("bn-on") || writing());
      list.classList.toggle("bn-writing", writing());
      const who = spaced(meta.agent || "main");
      barText.textContent = barNote || (state === "updating" ? NW.updating(who) : NW.sendBar(drafts, who));
      bar.classList.toggle("bn-on", drafts > 0 || !!barNote || state === "updating");
      barSend.disabled = drafts === 0 || state === "updating";
      // the frame's count: nothing at 0 (a lone 0 is a riddle), else `1 note` / `3 notes` (ambient, m_4868)
      const fc = frameCount(); if (fc) fc.textContent = NW.count(drafts);
      const fs = frameSend(); if (fs) fs.disabled = drafts === 0;
      renderPins();
      paintHighlights();
      for (const b of blocks()) b.classList.toggle("bn-has", M.all().some((n) => n.block === b.dataset.id && n.status === "draft"));
    }
    function renderPins() {
      pins.replaceChildren();
      const sx = win.scrollX, sy = win.scrollY;
      M.all().filter((n) => n.pin).forEach((n, i) => {
        const b = blockById(n.block);
        if (!b) return;
        const r = b.getBoundingClientRect();
        const p = el("div", `bn-pin bn-${n.status}${n.listening ? " bn-listening" : ""}`, String(i + 1));
        p.dataset.note = n.id;
        // CSSOM, not style= (the shell's CSP)
        const x = r.left + sx + n.pin.x * r.width, y = r.top + sy + n.pin.y * r.height;
        p.style.left = `${x}px`;
        p.style.top = `${y}px`;
        pins.append(p);
        // a voice note while you talk: your words land beside its pin
        if (n.listening) {
          const bub = el("div", "bn-bubble");
          bub.append(el("div", "bn-by", NW.voiceBy), el("div", "bn-said", n.text || "…"));
          bub.style.left = `${x + 18}px`;
          bub.style.top = `${y - 8}px`;
          pins.append(bub);
        }
      });
    }

    // ---- the toolbar: over a selection, or over the block under the mouse
    // (QA m_4929: on a block too, not only on a selection, so ♡ ✗ ~ work on a whole block or row
    // and `edit` is reachable on prose)
    let target = null;   // {block, quote?, offset?, pin?}
    function showTools(t, rect) {
      if (writing()) return;
      target = t;
      const b = blockById(t.block);
      // VoiceOver: each button says its action in words (♡ is 'keep'), the bar is a toolbar
      const btn = (k, text) => { const e = el("button", "bn-tool", text); e.type = "button"; e.dataset.tool = k; e.setAttribute("aria-label", NW.say[k] || text); return e; };
      // edit on prose, a review's item, an email
      tools.replaceChildren(btn("note", NW.tools.note), btn("keep", NW.tools.keep), btn("drop", NW.tools.drop), btn("unsure", NW.tools.unsure), ...(editable(b, t.item) ? [btn("edit", NW.tools.edit)] : []),
        // a review's item can start an agent (slice C)
        ...(b && b.dataset.kit === "review" && t.item ? [btn("start", NW.tools.start)] : []));
      tools.style.left = `${rect.left + win.scrollX}px`;
      tools.style.top = `${rect.top + win.scrollY - 40}px`;
      tools.dataset.on = t.picked ? "lines" : t.quote || t.lines ? "selection" : "block";
      tools.classList.add("bn-on");
    }
    // a diff's rows picked by their numbers (notelines.js) stay marked until the toolbar goes
    const hideTools = () => { tools.classList.remove("bn-on"); delete tools.dataset.on; if (gutter && !composer.classList.contains("bn-on")) gutter.clear(); };
    const gutter = root.BiseLines ? root.BiseLines.bindGutter(doc, page, (rows, b) => {
      const r = doc.createRange();
      r.setStartBefore(rows[0]); r.setEndAfter(rows[rows.length - 1]);
      const t = fromRange(r, page);
      if (!t) return;
      clearTimeout(hoverT);
      // above the run's first row, at the diff's right end: never over the lines (ambient m_7871)
      const box = (rows[0].closest(".k-diff") || b).getBoundingClientRect(), top = rows[0].getBoundingClientRect().top;
      showTools({ ...t, picked: true }, { left: box.right, top });
      tools.style.left = `${box.right + win.scrollX - tools.offsetWidth}px`;
    }, writing) : null;
    doc.addEventListener("mousedown", (e) => { if (tools.dataset.on === "lines" && !tools.contains(e.target) && !(e.target.closest && e.target.closest(".k-dl"))) hideTools(); });
    // the note you type: in the list's last row, under where it goes
    function compose(t) {
      target = t;
      composer.classList.add("bn-on");   // first: the picked rows stay marked while you type
      hideTools();
      composerWhere.textContent = t.drawing ? NW.drawing : where(t, nameOf(t.block, t.item));
      input.value = "";
      render();
      input.focus();
    }
    const closeComposer = () => { composer.classList.remove("bn-on"); if (gutter) gutter.clear(); input.blur(); render(); };
    function addNote(kind, text, extra) {
      if (!target) return;
      // words for a drawing just made: they go on its note
      if (target.drawing) { M.update(target.drawing, { text: text || undefined }); target = null; save(); render(); return; }
      M.add({ version, block: target.block, item: target.item, quote: target.quote, offset: target.offset, pin: target.pin, lines: target.lines, more: target.more, part: target.part, kind, text: text || undefined, where: nameOf(target.block, target.item) || undefined, ...extra }, now());
      barNote = "";
      save(); render();
    }

    const selecting = () => { const s = win.getSelection(); return !!(s && !s.isCollapsed && s.toString().trim()); };
    doc.addEventListener("mouseup", () => {
      win.setTimeout(() => {
        const s = fromSelection(win.getSelection(), page);
        if (!s) return;
        clearTimeout(hoverT);
        showTools(s, win.getSelection().getRangeAt(0).getBoundingClientRect());
      }, 0);
    });
    // on a block under the mouse, after a short rest; a selection's toolbar stays until it goes
    let hover = null, hoverT = 0;
    const busy = () => selecting() || drawing || pinning || editing || composer.classList.contains("bn-on") || tools.dataset.on === "selection" || tools.dataset.on === "lines";
    let hoverOn = null;   // the block, or a review's item under the mouse
    page.addEventListener("mouseover", (e) => {
      const b = e.target.closest && e.target.closest("[data-kit][data-id]");
      // the kit's own buttons are not a place for a note
      if (e.target.closest && e.target.closest("[data-kit-ui]")) return;
      const li = b && b.dataset.kit === "review" ? e.target.closest("li[data-id]") : null;
      const on = li || b;
      hover = b;
      if (on === hoverOn) return;
      hoverOn = on;
      clearTimeout(hoverT);
      if (!b) return;
      const t = li ? { block: b.dataset.id, item: li.dataset.id } : { block: b.dataset.id };
      hoverT = win.setTimeout(() => { if (hoverOn === on && !busy()) showTools(t, on.getBoundingClientRect()); }, 250);
    });
    page.addEventListener("mouseleave", () => {
      hover = null; hoverOn = null; clearTimeout(hoverT);
      hoverT = win.setTimeout(() => { if (tools.dataset.on === "block" && !tools.matches(":hover")) hideTools(); }, 300);
    });
    tools.addEventListener("mouseleave", () => { if (tools.dataset.on === "block" && !hover) hideTools(); });
    doc.addEventListener("selectionchange", () => { if (tools.dataset.on === "selection" && !selecting()) hideTools(); });
    tools.addEventListener("mousedown", (e) => e.preventDefault());   // keep the selection
    tools.addEventListener("click", (e) => {
      const k = e.target.dataset && e.target.dataset.tool;
      if (!k || !target) return;
      const b = blockById(target.block);
      if (k === "note") return compose(target);
      if (k === "edit") { hideTools(); return startEdit(b, target.item); }
      // start an agent on this item (slice C): the item's words go with it; main starts it
      if (k === "start") { const x = editEl(target.block, target.item); M.start(target.block, target.item, norm(textOf(x)), version, now()); hideTools(); barNote = ""; save(); render(); return; }
      addNote(k);
      hideTools();
      win.getSelection().removeAllRanges();
    });
    // your reply to the agent's answer: a note on the same place, kept as a draft until send
    list.addEventListener("keydown", (e) => {
      const f = e.target;
      if (!f.dataset || !f.dataset.replyTo) return;
      e.stopPropagation();
      if (e.key === "Escape") { f.value = ""; f.blur(); return; }
      if (e.key !== "Enter" || e.metaKey) return;
      e.preventDefault();
      const n = M.get(f.dataset.replyTo), text = f.value.trim();
      if (!n || !text) return;
      target = { block: n.block, quote: n.quote, offset: n.offset, pin: n.pin };
      addNote("note", text, { reply_to: n.id });
      target = null;
    });
    composer.addEventListener("submit", (e) => { e.preventDefault(); });
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && !e.shiftKey && !e.metaKey) { e.preventDefault(); if (input.value.trim()) addNote(target && target.pin ? "pin" : "note", input.value.trim()); closeComposer(); }
      else if (e.key === "Escape") { e.preventDefault(); closeComposer(); }
      e.stopPropagation();
    });

    // ---- pin mode: p, then a click on a block places it
    let pinning = false;
    page.addEventListener("click", (e) => {
      if (!pinning || writing()) return;
      const b = e.target.closest("[data-kit][data-id]");
      if (!b) return;
      e.preventDefault();
      pinning = false; doc.documentElement.classList.remove("bn-pinning");
      const r = b.getBoundingClientRect();
      compose({ block: b.dataset.id, pin: { x: +((e.clientX - r.left) / r.width).toFixed(3), y: +((e.clientY - r.top) / r.height).toFixed(3) } });
    }, true);

    // ---- edit in place: a prose block, a review's item (§4.3) or an email becomes editable;
    // leaving it makes the note. A review item or an email you edited carries data-edited, so
    // kit.js's "approve N" leaves it out (it goes with send, as your edit)
    const originals = {};   // block id (#item) -> its HTML and text before your first edit (undo)
    const editKey = (block, item) => (item ? `${block}#${item}` : block);
    const editable = (b, item) => !!b && (b.dataset.kit === "prose" || b.dataset.kit === "message" || b.dataset.kit === "email" || (b.dataset.kit === "review" && !!item));
    function editEl(block, item) {
      const b = blockById(block);
      if (!b || !item) return b;
      return [...b.querySelectorAll("li[data-id]")].find((li) => li.dataset.id === item) || null;
    }
    // an edited review item or email carries data-edited, and kit.js hears bise:edited to recount
    // its 'approve N' (QA m_5260 (15), agreed with amb-kit m_5284)
    function markEdited(e, on) {
      e.classList.toggle("bn-edited", on);
      if (e.matches("li[data-id]") || e.dataset.kit === "email") {
        if (on) e.setAttribute("data-edited", ""); else e.removeAttribute("data-edited");
        const b = e.closest("[data-kit][data-id]");
        doc.dispatchEvent(new win.CustomEvent("bise:edited", { detail: { block: b && b.dataset.id, item: e.matches("li[data-id]") ? e.dataset.id : null, on } }));
      }
    }
    // what you may edit (QA m_5260 (14)): the parts kit.js marks data-editable (a review item's
    // drafted reply, an email's subject and body, a message's text), never a header line; a block
    // with none (prose) is edited whole
    const editRoots = (e) => { const r = [...e.querySelectorAll("[data-editable]")]; return r.length ? r : [e]; };
    const rootsText = (roots) => norm(roots.map(textOf).join(" "));
    let editing = null;   // {el, roots, block, item}
    function startEdit(b, item) {
      if (!editable(b, item)) return;
      const e = editEl(b.dataset.id, item);
      if (!e) return;
      const roots = editRoots(e);
      const key = editKey(b.dataset.id, item);
      if (!(key in originals)) originals[key] = { html: e.innerHTML, text: rootsText(roots) };
      editing = { el: e, roots, block: b.dataset.id, item: item || null };
      for (const r of roots) {
        // the kit's own buttons inside stay buttons
        for (const u of r.querySelectorAll("[data-kit-ui]")) u.contentEditable = "false";
        r.contentEditable = "true";
      }
      e.classList.add("bn-editing");
      roots[0].focus();
    }
    function endEdit() {
      if (!editing) return;
      const { el: e, roots, block, item } = editing;
      for (const r of roots) r.contentEditable = "false";
      e.classList.remove("bn-editing");
      const first = originals[editKey(block, item)].text, after = rootsText(roots);
      const d = editSpan(first, after);
      M.edit(block, d.before, d.after, now(), item, { ...d, version });
      markEdited(e, norm(first) !== after);
      editing = null;
      save(); render();
    }
    const inEdit = (n) => !!(editing && n && editing.roots.includes(n));
    // leaving the last editable part ends the edit (moving between the subject and the body doesn't)
    page.addEventListener("focusout", (e) => { if (inEdit(e.target) && !inEdit(e.relatedTarget)) endEdit(); });
    page.addEventListener("keydown", (e) => {
      if (editing && e.key === "Escape") { e.preventDefault(); if (doc.activeElement && doc.activeElement.blur) doc.activeElement.blur(); if (editing) endEdit(); }
    });

    // ---- keys: t type · p pin · ⌘z undo · ⌘⏎ send
    doc.addEventListener("keydown", (e) => {
      if (writing()) return;
      if (e.target === input || (inEdit(e.target) && !(e.metaKey && e.key === "Enter"))) return;
      if (e.metaKey && e.key === "Enter") { e.preventDefault(); if (editing) { if (doc.activeElement && doc.activeElement.blur) doc.activeElement.blur(); if (editing) endEdit(); } return send(); }
      if (e.metaKey && e.key === "z" && !e.shiftKey) {
        const u = M.undo();
        if (!u) return;
        e.preventDefault();
        // an edit undone: the text as it was, and no longer marked edited
        if (u.note.kind === "edit") {
          const key = editKey(u.note.block, u.note.item), x = editEl(u.note.block, u.note.item), o = originals[key];
          if (u.type === "add" && x && o) { x.innerHTML = o.html; markEdited(x, false); delete originals[key]; }
        }
        save(); render();
        return;
      }
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key === "t" && hover) { e.preventDefault(); compose({ block: hover.dataset.id }); }
      else if (e.key === "p") { e.preventDefault(); pinning = !pinning; doc.documentElement.classList.toggle("bn-pinning", pinning); }
      else if (e.key === "d") { e.preventDefault(); drawing ? stopDraw() : startDraw(); }
      else if (e.key === "Escape") { hideTools(); pinning = false; doc.documentElement.classList.remove("bn-pinning"); if (drawing) stopDraw(); }
    });

    // ---- without a mouse (ambient-lead m_5651): tab reaches every block (and a review's items),
    // the toolbar comes with the focus, and on the focused one n note, h ♡, x ✗, ~ not sure,
    // e edit, s start an agent, ⏎ the toolbar's first button; ⌘⏎ sends as everywhere
    function focusable() {
      for (const b of page.querySelectorAll("[data-kit][data-id]")) {
        if (!b.hasAttribute("tabindex")) b.tabIndex = 0;
        if (b.dataset.kit === "review") for (const li of b.querySelectorAll("li[data-id]")) if (!li.hasAttribute("tabindex")) li.tabIndex = 0;
      }
    }
    const focusTarget = (node) => {
      if (!node || !node.closest || node.closest("[data-kit-ui]")) return null;
      const b = node.matches("[data-kit][data-id]") ? node : node.matches("li[data-id][tabindex]") ? node.closest("[data-kit][data-id]") : null;
      if (!b || !page.contains(b)) return null;
      return node === b ? { t: { block: b.dataset.id }, el: b } : { t: { block: b.dataset.id, item: node.dataset.id }, el: node };
    };
    let pointerAt = -1e9;
    doc.addEventListener("pointerdown", () => { pointerAt = win.performance.now(); }, true);
    page.addEventListener("focusin", (e) => {
      // a focus from the keyboard (not a click) brings the toolbar
      if (win.performance.now() - pointerAt < 300 || busy()) return;
      const f = focusTarget(e.target);
      if (f) showTools(f.t, f.el.getBoundingClientRect());
    });
    page.addEventListener("keydown", (e) => {
      if (e.metaKey || e.ctrlKey || e.altKey || writing() || editing) return;
      const f = focusTarget(e.target);
      if (!f) return;
      const k = { n: "note", t: "note", h: "keep", x: "drop", "~": "unsure", e: "edit", s: "start" }[e.key];
      if (e.key === "Enter") { e.preventDefault(); showTools(f.t, f.el.getBoundingClientRect()); const first = tools.querySelector("button"); if (first) first.focus(); return; }
      if (!k) return;
      e.preventDefault(); e.stopPropagation();
      target = f.t;
      const b = blockById(f.t.block);
      if (k === "note") return compose(f.t);
      if (k === "edit") { if (editable(b, f.t.item)) { hideTools(); startEdit(b, f.t.item); } return; }
      if (k === "start") { if (b && b.dataset.kit === "review" && f.t.item) { const x = editEl(f.t.block, f.t.item); M.start(f.t.block, f.t.item, norm(textOf(x)), version, now()); barNote = ""; save(); render(); } return; }
      addNote(k);
    });
    // the toolbar's keys: ← → between its buttons, esc back to the block
    tools.addEventListener("keydown", (e) => {
      const bs = [...tools.querySelectorAll("button")], i = bs.indexOf(doc.activeElement);
      if (i < 0) return;
      if (e.key === "ArrowRight" || e.key === "ArrowLeft") { e.preventDefault(); bs[(i + (e.key === "ArrowRight" ? 1 : bs.length - 1)) % bs.length].focus(); }
      else if (e.key === "Escape") { e.preventDefault(); const b = target && blockById(target.block); hideTools(); if (b) b.focus(); }
    });
    focusable();

    // ---- the list's clicks: go to the note, remove a draft
    list.addEventListener("click", (e) => {
      const x = e.target.dataset && e.target.dataset.remove;
      if (x) { M.remove(x); save(); render(); return; }
      // taste: yes keeps the note for next time (taste: true, the agent appends it to taste.md); no stops asking
      const t = e.target.dataset && e.target.dataset.taste;
      if (t) { M.update(e.target.dataset.note, { taste: t === "yes" }); save(); render(); return; }
      const li = e.target.closest(".bn-item");
      const n = li && M.get(li.dataset.note);
      const b = n && blockById(n.block);
      if (b) b.scrollIntoView({ behavior: "smooth", block: "center" });
    });

    // ---- send: the button, the frame's button, ⌘⏎
    async function send() {
      if (!M.drafts().length || state === "updating") return;
      clearTimeout(saveT);
      // start notes go to main, never the page's agent: the page doesn't update for them alone
      const toMain = M.drafts().every((n) => n.kind === "start" || n.kind === "stop");
      try {
        await api(`/api/p/${pageId}/notes`, M.payload());
        await api(`/api/p/${pageId}/send`, {});
        M.sent(now());
        barNote = NW.sent(spaced(meta.agent || "main"));
        if (!toMain) state = "updating";
        render();
        win.setTimeout(() => { barNote = ""; render(); }, 4000);
      } catch { barMsg(NW.failed); }
    }
    // the watching frame's stop goes alone (ambient-lead m_5502: a button does only what it
    // names): the server sends every draft it holds, so it holds only the stop for the send, then
    // gets his other drafts back (they never left notes.js; merge keeps drafts it doesn't know)
    // (the same for a report's 'start an agent', bise:start: one start note, alone, at once)
    async function sendAlone(id, said) {
      clearTimeout(saveT);
      try {
        await api(`/api/p/${pageId}/notes`, M.payload(id));
        await api(`/api/p/${pageId}/send`, {});
        M.sentOne(id, now());
        barNote = said;
        render();
        win.setTimeout(() => { barNote = ""; render(); }, 4000);
      } catch { barMsg(NW.failed); }
      // his drafts back on the server, sent or not
      save();
    }
    barSend.addEventListener("click", send);
    doc.addEventListener("click", (e) => { if (e.target.closest && e.target.closest("[data-bise-send]")) send(); });

    // ---- kit.js's events: a new version swapped in, the notes' outcomes, the page's state
    doc.addEventListener("bise:swapped", (e) => {
      version = (e.detail && e.detail.version) || +page.dataset.version || version;
      for (const id of Object.keys(originals)) delete originals[id];   // the new text is the agent's
      M.reanchor(blockTexts(), version);
      focusable();
      render();
    });
    doc.addEventListener("bise:notes", (e) => { M.merge((e.detail && e.detail.notes) || []); M.reanchor(blockTexts(), version); render(); });
    // ---- a voice note at the mouse (§4.1): fn held on this page; the core's words arrive as
    // kit.js's bise:voice {phase, text}; the pin goes where the mouse last was over a block
    let mouse = null;
    doc.addEventListener("mousemove", (e) => { mouse = { x: e.clientX, y: e.clientY }; }, { passive: true });
    function anchorAt(pt) {
      let b = pt && doc.elementFromPoint(pt.x, pt.y);
      b = b && b.closest && b.closest("[data-kit][data-id]");
      if (!b || !page.contains(b)) {
        // no block under the mouse: the block nearest the middle of the window
        const mid = win.innerHeight / 2;
        b = blocks().reduce((best, x) => { const r = x.getBoundingClientRect(), d = Math.abs((r.top + r.bottom) / 2 - mid); return !best || d < best.d ? { x, d } : best; }, null);
        b = b && b.x;
        pt = null;
      }
      if (!b) return null;
      const r = b.getBoundingClientRect();
      const at = { block: b.dataset.id, pin: pt ? { x: +((pt.x - r.left) / r.width).toFixed(3), y: +((pt.y - r.top) / r.height).toFixed(3) } : { x: 0, y: 0 } };
      // the nearest words: the text before the caret under the mouse
      const caret = pt && (doc.caretRangeFromPoint ? doc.caretRangeFromPoint(pt.x, pt.y) : doc.caretPositionFromPoint && (() => { const p = doc.caretPositionFromPoint(pt.x, pt.y); if (!p) return null; const r2 = doc.createRange(); r2.setStart(p.offsetNode, p.offset); return r2; })());
      let offset = 0;
      if (caret && b.contains(caret.startContainer)) {
        const pre = doc.createRange();
        pre.selectNodeContents(b);
        pre.setEnd(caret.startContainer, caret.startOffset);
        offset = norm(fragText(pre.cloneContents())).length;
      }
      const q = nearestQuote(textOf(b), offset);
      if (q) { at.quote = q.quote; at.offset = q.offset; }
      return at;
    }
    doc.addEventListener("bise:voice", (e) => {
      const d = e.detail || {};
      if (d.phase === "start") { const at = anchorAt(mouse); if (at) M.voiceStart(at, version, now()); }
      else if (d.phase === "heard") M.voiceHeard(d.text);
      else if (d.phase === "end") { if (M.voiceEnd(d.text, now())) save(); }
      else if (d.phase === "cancel") M.voiceCancel();
      else if (d.phase === "send") { M.voiceCancel(); return send(); }
      render();
    });

    // a question answered in the capsule or the TUI: kit.js's question shows the answer; a draft
    // pick of ours on it goes (one card, one answer)
    doc.addEventListener("bise:answered", (e) => {
      if (M.answered((e.detail || {}).block)) { save(); render(); }
    });
    // the watching frame's 'stop' (roadmap B): a stop note to main, sent at once and alone; his
    // other drafts stay drafts until he sends them (⌘⏎)
    doc.addEventListener("bise:stop", (e) => {
      const d = e.detail || {};
      // block 'frame': amb-home's hub (3f22c3bc) ends the page's timers, marks it done, no model turn
      const n = M.stop("frame", d.timer, version, now());
      render(); sendAlone(n.id, NW.stopped);
    });
    // a report's 'start an agent' (kit.js's bise:start {block, item, text}, amb-kit d418d973): the
    // toolbar's start note, sent at once and alone to main; kit.js shows 'starting…' on the item
    doc.addEventListener("bise:start", (e) => {
      const d = e.detail || {};
      if (!d.block) return;
      const n = M.startNow(d.block, d.item || null, d.text, version, now());
      render(); sendAlone(n.id, NW.sent("bise"));
    });
    // a checklist row ticked by its agent: a draft tick of ours on that row goes
    doc.addEventListener("bise:ticked", (e) => {
      const d = e.detail || {};
      if (d.block && M.ticked(d.block, d.item || null)) { save(); render(); }
    });

    // a question's option picked (kit.js marks it on the page; the note is ours)
    doc.addEventListener("bise:pick", (e) => {
      const d = e.detail || {};
      if (!d.block) return;
      M.pick(d.block, d.option, d.text, version, now());
      barNote = ""; save(); render();
    });
    // approve / skip on a review's item or an email (kit.js's bise:react, §4.3): a note of that
    // kind, one per item, the latest wins; kind null takes it back
    doc.addEventListener("bise:react", (e) => {
      const d = e.detail || {};
      if (!d.block) return;
      M.react(d.block, d.item || null, d.kind, d.text, version, now());
      barNote = ""; save(); render();
    });
    doc.addEventListener("bise:state", (e) => {
      state = (e.detail && e.detail.state) || "ready";
      if (writing()) { hideTools(); closeComposer(); }
      render();
    });
    win.addEventListener("resize", renderPins);

    // ---- drawing (§5.4): d, draw with the mouse or the pen, d or esc again: one drawing note
    // (its path in the page's coordinates, its box, the blocks under it) and the PNG of that area
    // the page draws itself with a canvas, sent with the note
    const SVGNS = "http://www.w3.org/2000/svg";
    const ink = doc.createElementNS(SVGNS, "svg");
    ink.setAttribute("class", "bn-ui bn-ink");
    const pad = el("div", "bn-ui bn-drawpad");
    const hint = el("div", "bn-ui bn-drawhint", NW.drawHint);
    doc.body.append(ink, pad, hint);
    let drawing = null;   // {strokes: [[[x, y]...]], live: [[x, y]...] | null}
    const pageXY = (e) => { const r = page.getBoundingClientRect(); return [e.clientX - r.left, e.clientY - r.top]; };
    function placeInk() {
      // the ink layer covers the page, in the page's coordinates (CSSOM, not style=)
      const r = page.getBoundingClientRect();
      ink.style.left = `${r.left + win.scrollX}px`; ink.style.top = `${r.top + win.scrollY}px`;
      ink.setAttribute("width", Math.ceil(r.width)); ink.setAttribute("height", Math.ceil(r.height));
    }
    function renderInk() {
      placeInk();
      const paths = [];
      for (const n of M.all()) if (n.kind === "draw" && n.path) paths.push([n.path, `bn-stroke bn-${n.status}`]);
      if (drawing) for (const s of [...drawing.strokes, drawing.live || []]) if (s.length) paths.push([strokePath(s), "bn-stroke bn-live"]);
      ink.replaceChildren(...paths.map(([d, cls]) => { const p = doc.createElementNS(SVGNS, "path"); p.setAttribute("d", d); p.setAttribute("class", cls); return p; }));
    }
    function startDraw() {
      if (writing()) return;
      hideTools(); closeComposer();
      drawing = { strokes: [], live: null };
      doc.documentElement.classList.add("bn-drawing");
    }
    pad.addEventListener("pointerdown", (e) => {
      if (!drawing) return;
      pad.setPointerCapture(e.pointerId);
      drawing.live = [pageXY(e)];
    });
    pad.addEventListener("pointermove", (e) => {
      if (!drawing || !drawing.live) return;
      for (const c of e.getCoalescedEvents ? e.getCoalescedEvents() : [e]) drawing.live.push(pageXY(c));
      renderInk();
    });
    const endStroke = () => { if (drawing && drawing.live) { if (drawing.live.length) drawing.strokes.push(drawing.live); drawing.live = null; renderInk(); } };
    pad.addEventListener("pointerup", endStroke);
    pad.addEventListener("pointercancel", endStroke);
    function stopDraw() {
      endStroke();
      const d = drawing;
      drawing = null;
      doc.documentElement.classList.remove("bn-drawing");
      if (!d || !d.strokes.length) return renderInk();
      const box = strokesBox(d.strokes);
      const pr = page.getBoundingClientRect();
      const rects = Object.fromEntries(blocks().map((b) => { const r = b.getBoundingClientRect(); return [b.dataset.id, { x: r.left - pr.left, y: r.top - pr.top, w: r.width, h: r.height }]; }));
      const under = blocksIn(box, rects);
      const note = M.add({ version, kind: "draw", block: under[0], blocks: under, box, path: d.strokes.map(strokePath).join(" ") }, now());
      save(); render(); renderInk();
      // words for it, if you want (Enter keeps them, Esc keeps the drawing alone)
      target = { block: under[0], drawing: note.id };
      compose(target);
      areaPng(box, note.path).then((png) => { if (png && M.get(note.id)) { M.update(note.id, { png }); save(); } });
    }
    // the area as the page shows it, with the drawing on top: the page's HTML and CSS in an SVG
    // foreignObject drawn on a canvas; where the browser won't let a canvas read that back, the
    // strokes alone on the page's paper
    async function areaPng(box, path) {
      const m = 16, w = Math.max(1, Math.min(1600, box.w + 2 * m)), h = Math.max(1, Math.min(1600, box.h + 2 * m));
      const x0 = box.x - m, y0 = box.y - m;
      const cv = doc.createElement("canvas");
      cv.width = w; cv.height = h;
      const g = cv.getContext("2d");
      const paper = win.getComputedStyle(doc.body).backgroundColor || "#fff";
      const strokes = () => {
        g.save(); g.translate(-x0, -y0);
        g.strokeStyle = win.getComputedStyle(doc.documentElement).getPropertyValue("--pen").trim() || "#c8264a";
        g.lineWidth = 3; g.lineCap = "round"; g.lineJoin = "round";
        g.stroke(new win.Path2D(path));
        g.restore();
      };
      try {
        let css = "";
        for (const s of doc.styleSheets) { try { for (const r of s.cssRules) css += r.cssText + "\n"; } catch {} }
        const clone = page.cloneNode(true);
        const pw = page.getBoundingClientRect().width;
        const xhtml = new win.XMLSerializer().serializeToString(clone);
        const svg = `<svg xmlns="${SVGNS}" width="${w}" height="${h}"><style>${css.replace(/</g, "\\3c ")}</style>` +
          `<rect width="100%" height="100%" fill="${paper}"/><foreignObject x="${-x0}" y="${-y0}" width="${pw}" height="${y0 + h + 10}">` +
          `<div xmlns="http://www.w3.org/1999/xhtml" class="${doc.body.className}">${xhtml}</div></foreignObject></svg>`;
        const img = new win.Image();
        img.src = "data:image/svg+xml;charset=utf-8," + encodeURIComponent(svg);
        await img.decode();
        g.drawImage(img, 0, 0);
        strokes();
        return cv.toDataURL("image/png");
      } catch {
        g.clearRect(0, 0, w, h);
        g.fillStyle = paper; g.fillRect(0, 0, w, h);
        strokes();
        try { return cv.toDataURL("image/png"); } catch { return null; }
      }
    }
    const _render = render;
    render = function () { _render(); renderInk(); };
    win.addEventListener("resize", renderInk);
    renderInk();

    // ---- the drafts and the outcomes already on the server
    api(`/p/${pageId}/meta`).then((r) => {
      // flat {id, title, agent, state, versions, url, notes} (amb-core, ecb6ff9); {meta, notes} read too
      const m = r.meta || r || {};
      meta.title = m.title || meta.title; meta.agent = m.agent || ""; state = m.state || state;
      M.merge(r.notes || m.notes || []);
      M.reanchor(blockTexts(), version);
      // your edits in place, back on the page after a reload
      for (const n of M.drafts().filter((x) => x.kind === "edit")) reapplyEdit(n);
      render();
    }).catch(() => render());
    function reapplyEdit(n) {
      const x = editEl(n.block, n.item);
      if (!x || !n.before) return;
      const roots = editRoots(x);
      for (const t of roots.flatMap(textNodes)) {
        const i = t.nodeValue.indexOf(n.before);
        if (i >= 0) {
          originals[editKey(n.block, n.item)] = { html: x.innerHTML, text: rootsText(roots) };
          t.nodeValue = t.nodeValue.slice(0, i) + n.after + t.nodeValue.slice(i + n.before.length);
          markEdited(x, true);
          return;
        }
      }
    }
    // ---- the frame's pearl (round 8; amb-kit m_5181, m_5276): kit.js's [data-bise-pearl=N] gets
    // bise's body at N px from /kit/pearl.js (the capsule's own shader), at rest, turning while
    // the page is being written or updated, still under reduced motion, paused in a hidden tab;
    // the ':*' text stays as the fallback until the canvas has drawn
    function framePearls() {
      const P = win.AmbPearl;
      if (!P) return;
      const reduce = !!(win.matchMedia && win.matchMedia("(prefers-reduced-motion: reduce)").matches);
      // WebGL only once the spot can be seen and the page has painted (speed, roadmap §3.5): the
      // ':*' text holds the place until then, so the first paint never waits on a shader compile
      const later = (f) => (win.requestIdleCallback ? win.requestIdleCallback(f, { timeout: 500 }) : win.setTimeout(f, 50));
      const whenSeen = (spot, f) => {
        if (!win.IntersectionObserver) return later(f);
        const io = new win.IntersectionObserver((es) => { if (es.some((e) => e.isIntersecting)) { io.disconnect(); later(f); } });
        io.observe(spot);
      };
      for (const spot of doc.querySelectorAll("[data-bise-pearl]")) {
        if (spot.dataset.pearlOn) continue;
        spot.dataset.pearlOn = "on";
        whenSeen(spot, () => startPearl(spot));
      }
      function startPearl(spot) {
        if (!doc.contains(spot)) return;
        const n = +spot.dataset.bisePearl || 36;
        const cv = doc.createElement("canvas");
        cv.className = "bn-pearl";
        cv.style.width = cv.style.height = `${n}px`;   // CSSOM, not style= (the shell's CSP)
        const p = P.create(cv);
        if (p.failed) return;
        let shown = false;
        const tick = () => {
          if (!doc.contains(spot)) return;
          const mood = state === "writing" || state === "updating" ? "work" : "rest";
          const moving = doc.hidden ? true : p.frame({ mood, lvl: 0, still: reduce }, win.performance.now());
          if (!shown && !doc.hidden) { spot.replaceChildren(cv); shown = true; }
          win.setTimeout(tick, doc.hidden ? 1000 : moving ? 33 : 250);
        };
        tick();
      }
    }
    framePearls();

    render();
    return { model: M, render };
  }

  root.BiseNotes = { core, mount, rangeFor, rawPoints, fromSelection, fromRange };
  if (root.document && root.document.getElementById) {
    // the page's layer, kept for a look from the console (BiseNotes.live.model.all())
    const go = () => { root.BiseNotes.live = mount(root.document); };
    if (root.document.readyState === "loading") root.document.addEventListener("DOMContentLoaded", go); else go();
  }
})(typeof window !== "undefined" ? window : globalThis);
