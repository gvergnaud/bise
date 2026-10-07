// bise kit v0 · kit.js (docs/ambient-pages.md §2.5, §2.7): the frame, the components' behavior, the
// one SSE client of the page, the in-place swap of a new version by data-id (scroll kept, then a
// `bise:swapped` event), and "what changed" from the blocks' hashes in meta. notes.js (amb-web) is
// the note layer on top; the two talk through document events only:
//   bise:swapped {version, changed, removed}   a new version is in (changed = new or different ids)
//   bise:notes   {notes}                       the SSE notes event (all notes of the page)
//   bise:state   {state}                       the SSE state event ("ready" | "updating")
//   bise:pick    {block, option, text}         a question's option picked as a note (null: taken
//                                              back), when the server has no /answer route (slice 1)
//   bise:answered {block, reply}               the question's card was answered (SSE answered)
//   bise:voice   {phase, text}                 a voice note at the mouse (SSE voice, slice 2)
//   (a message block's `copy` puts its Slack markup on the clipboard; no event)
//   bise:react   {block, item, kind, text}     approve / skip (review item, email: item null),
//                                              keep / drop / pick (compare), tick (checklist);
//                                              kind null: taken back
// The kit's own controls inside blocks carry data-kit-ui (never part of a quote or anchor).
// notes.js marks a review item the user edited with data-edited: "approve N" leaves it out.
// and the frame's [data-bise-notes-count] and [data-bise-send] (notes.js writes and binds them).
// No style= anywhere: looks hang on data- attributes kit.css reads. The pure half (BiseKit.*) runs
// in node for the laws (kit/check.mjs) with a fake DOM.
(function (root) {
  "use strict";

  // the theme the page was opened with (?theme=light|dark: the desktop window passes his pref) on
  // <html data-theme>, which tokens.css follows over the system's; none: the system's
  const themeOf = (search) => {
    const m = /(?:^\?|&)theme=(light|dark)(?:&|$)/.exec(String(search || ""));
    return m ? m[1] : null;
  };
  {
    const t = themeOf(root.location && root.location.search);
    const html = root.document && root.document.documentElement;
    if (t && html && html.setAttribute) html.setAttribute("data-theme", t);
  }

  // an agent's id as people read it (vision §3): hyphens become spaces, the id itself never changes
  // an agent's name as the user reads it: spaces, not hyphens; main speaks as bise on the ambient
  // surfaces (ambient m_5473, amb-web 4139b4fe): 'by bise', an agent's page keeps its own name
  const said = (agent) => (agent === "main" ? "bise" : String(agent || "").replace(/-/g, " "));

  // ---- the words (ambient's) ----
  const W = {
    by: (agent) => `by ${said(agent)}`,
    updating: (agent) => `${said(agent)} is updating this page…`,
    writing: (agent) => `${said(agent)} is writing this page…`,
    stopped: (agent) => `${said(agent)} stopped before writing anything here.`,
    // the writing state (ambient m_5095)
    writingFrame: "writing",
    asked: (ask) => `you asked: “${ask}”`,
    onIt: "on it.",
    earlier: (n) => `+${n} earlier`,
    failed: (reason) => (reason ? `▲ couldn't finish: ${reason}. main is on it.` : "▲ couldn't finish. main is on it."),
    tabWriting: (title) => `${title} · writing`,
    changed: "what changed",
    send: "send",
    latest: "see the latest",
    older: (n, latest) => `v${n} of ${latest}`,
    forYou: "for you",
    approve: "approve",
    skip: "skip",
    approveAll: (n) => (n === 2 ? "approve both" : `approve ${n}`),
    keep: "♡",
    drop: "✗",
    pick: "pick",
    tick: "done",
    copy: "copy",
    copied: "copied",
    copyFailed: "couldn't copy: select the text and copy it",
    open: "open",
    openIn: (ch) => `open ${ch}`,
    postedIn: (ch) => `posted in ${ch}`,
    send: "send",
    sendAll: (n) => (n === 2 ? "send both" : `send all ${n}`),
    openInPlace: (place) => `open in ${place}`,
    yours: "you",
    inThePage: "in the page",
    inTheQuestion: "pick in the question",
    versions: "versions of this page",
    watching: "watching",
    stop: "stop",
    putInDrafts: "put it in my drafts",
    followingTaste: "following your taste",
    // feedback into work (roadmap C)
    start: "start an agent",
    starting: "starting…",
    answer: (who) => (who ? `answer ${who}` : "answer"),
    // a reply on a read-only place (ambient m_5493): where it goes, then what's left to do
    openReply: "open with the reply",
    openOn: (site) => `open on ${site}`,
    openedOn: (site) => `opened on ${site} · your reply is in the box`,
    copiedOpen: "copied",
    openThread: "paste it in the thread",
    keepRule: "keep",
    strike: "strike",
    replyOn: (site) => `reply on ${site}`,
    copyOpen: (site) => `copy + open ${site}`,
  };

  // ---- the writing state: the agent's intents, newest at the bottom, the last 5 shown ----
  // `list`: the progress lines so far; `working`: the agent is still at it (the last one is
  // current). Returns {earlier: how many fold into "+N earlier", lines: [{text, current}]}.
  function intentLines(list, working, max = 5) {
    const all = (list || []).filter((t) => String(t).trim() !== "");
    const shown = all.slice(-max);
    return {
      earlier: all.length - shown.length,
      lines: shown.map((text, i) => ({ text: String(text), current: !!working && i === shown.length - 1 })),
    };
  }
  // the time since the page started, counting up: "12 s", "3 min"
  function since(startMs, nowMs) {
    const s = Math.max(0, Math.floor((nowMs - startMs) / 1000));
    return s < 60 ? `${s} s` : `${Math.floor(s / 60)} min`;
  }

  // ---- time ----
  function ago(atMs, nowMs) {
    const s = Math.max(0, Math.round((nowMs - atMs) / 1000));
    if (s < 45) return "just now";
    const m = Math.round(s / 60);
    if (m < 60) return `${m} min ago`;
    const h = Math.round(m / 60);
    if (h < 24) return `${h} h ago`;
    const d = Math.round(h / 24);
    if (d < 7) return d === 1 ? "yesterday" : `${d} days ago`;
    // past a week the date ('3 oct'), past a year with its year ('3 oct 2025') (designer m_7475)
    const at = new Date(atMs), now = new Date(nowMs);
    const day = `${at.getDate()} ${MONTHS[at.getMonth()]}`;
    return at.getFullYear() === now.getFullYear() && d < 365 ? day : `${day} ${at.getFullYear()}`;
  }

  // ---- what changed: blocks of version n whose hash differs from version n-1 (new ones too) ----
  function versionOf(meta, n) {
    return ((meta && meta.versions) || []).find((v) => v.n === n) || null;
  }
  function diffBlocks(prev, next) {
    const before = new Map((prev || []).map((b) => [b.id, b.hash]));
    const after = new Set((next || []).map((b) => b.id));
    return {
      changed: (next || []).filter((b) => before.get(b.id) !== b.hash).map((b) => b.id),
      removed: (prev || []).filter((b) => !after.has(b.id)).map((b) => b.id),
    };
  }
  // ids changed in version n against n-1; [] for v1 or when meta lacks either
  function changedIn(meta, n) {
    const cur = versionOf(meta, n), prev = versionOf(meta, n - 1);
    if (!cur || !prev) return [];
    return diffBlocks(prev.blocks, cur.blocks).changed;
  }

  // ---- numbers in tables ----
  const NUM = /^[\s(]*[+\-−–~≈]?\s*[$€£¥]?\s*\d[\d\s.,' ]*(?:[.,]\d+)?\s*(?:%|‰|[a-zA-Zµ°$€£]{1,4}\.?)?\s*(?:→\s*[$€£]?\d[\d\s.,]*\s*(?:%|[a-zA-Zµ]{1,4})?)?[\s)]*$/;
  const isNumeric = (text) => NUM.test(String(text).trim());
  // the columns (0-based) whose non-empty body cells are all numbers; rows: arrays of cell texts
  function numericColumns(rows) {
    const cols = new Map();
    for (const row of rows) row.forEach((t, i) => {
      if (String(t).trim() === "") return;
      cols.set(i, (cols.get(i) ?? true) && isNumeric(t));
    });
    return [...cols].filter(([, ok]) => ok).map(([i]) => i).sort((a, b) => a - b);
  }

  // ---- DOM helpers that work on the browser's DOM and on the laws' fake one ----
  const kids = (el) => Array.from(el.children || []);
  const isBlock = (el) => !!el.getAttribute && el.getAttribute("data-kit") != null && el.getAttribute("data-id") != null;
  const idOf = (el) => el.getAttribute("data-id");
  const norm = (html) => String(html).replace(/\s+/g, " ").replace(/> </g, "><").trim();

  // The source each block came with, before notes.js decorated it: the fallback to compare
  // blocks when meta has no hashes.
  const sources = new WeakMap();
  function remember(el) { if (el && el.outerHTML != null) sources.set(el, norm(el.outerHTML)); }

  // Swap the blocks of `main` for `incoming` (the new version's block elements, in order), in
  // place: a block whose id is kept and that did not change stays the same node (notes.js keeps
  // its pins and edits there); a changed or new one comes in; a gone one goes; the order follows
  // the new version. `changed`: the ids that differ (from the hashes), or null to compare sources.
  // `view` (optional): {anchor(): {id, top} | null, topOf(id), scrollBy(dy)} keeps the reading
  // position: the first block on screen stays where it was. Returns {changed, removed}.
  function swap(main, incoming, changed, view) {
    const old = new Map(kids(main).filter(isBlock).map((el) => [idOf(el), el]));
    const anchor = view && view.anchor ? view.anchor() : null;
    const differs = changed
      ? (() => { const s = new Set(changed); return (el) => s.has(idOf(el)); })()
      : (el) => { const o = old.get(idOf(el)); return !o || sources.get(o) !== norm(el.outerHTML); };
    const out = [], diff = [];
    for (const nb of incoming) {
      const id = idOf(nb), ob = old.get(id);
      if (ob && !differs(nb)) { out.push(ob); continue; }
      diff.push(id);
      nb.setAttribute("data-arrived", "");
      out.push(nb);
    }
    const keep = new Set(out);
    const removed = [];
    for (const [id, el] of old) if (!keep.has(el)) { main.removeChild(el); if (!incoming.some((n) => idOf(n) === id)) removed.push(id); }
    // place in order, moving only what is out of place
    let ref = kids(main).find(isBlock) || null;
    for (const el of out) {
      if (el === ref) { ref = nextBlock(main, el); continue; }
      main.insertBefore(el, ref);
    }
    for (const el of out) if (!old.has(idOf(el)) || old.get(idOf(el)) !== el) remember(el);
    if (anchor && view.topOf) {
      // the anchor block, or the first kept block after it, back where it was on screen
      const ids = out.map(idOf);
      let at = ids.indexOf(anchor.id);
      if (at < 0) at = 0;
      const top = view.topOf(ids[at]);
      if (top != null && top !== anchor.top) view.scrollBy(top - anchor.top);
    }
    return { changed: diff, removed };
  }
  function nextBlock(main, el) {
    const list = kids(main);
    for (let i = list.indexOf(el) + 1; i < list.length; i++) if (isBlock(list[i])) return list[i];
    return null;
  }

  // ---- the components ----
  function cellTexts(tr) { return kids(tr).filter((c) => /^(td|th)$/i.test(c.tagName)).map((c) => c.textContent); }
  function decorateTable(block) {
    for (const table of block.querySelectorAll("table")) {
      const trs = Array.from(table.querySelectorAll("tr"));
      const body = trs.filter((tr) => kids(tr).some((c) => /^td$/i.test(c.tagName)));
      const cols = new Set(numericColumns(body.map(cellTexts)));
      for (const tr of trs) kids(tr).forEach((c, i) => {
        if (cols.has(i)) c.setAttribute("data-num", ""); else c.removeAttribute("data-num");
      });
    }
  }
  function decorateQuestion(block, agent) {
    if (agent) block.setAttribute("data-by", said(agent));
    const answer = block.getAttribute("data-answer");
    const opts = options(block);
    if (answer != null) {
      const n = parseInt(answer, 10);
      opts.forEach((li, i) => {
        const hit = n ? i + 1 === n : norm(li.textContent).toLowerCase() === norm(answer).toLowerCase();
        if (hit) li.setAttribute("data-picked", ""); else li.removeAttribute("data-picked");
      });
    }
  }
  const options = (block) => {
    const list = kids(block).find((c) => /^(ol|ul)$/i.test(c.tagName));
    return list ? kids(list).filter((c) => /^li$/i.test(c.tagName)) : [];
  };
  // pick (or take back) option n (1-based) of a question: marks it and says what was picked
  // ({block, option, text}; option null: taken back), or null when there is nothing to pick
  function pick(block, n) {
    if (block.getAttribute("data-answer") != null) return null;
    const opts = options(block);
    const li = opts[n - 1];
    if (!li) return null;
    const again = li.hasAttribute("data-picked");
    opts.forEach((o) => o.removeAttribute("data-picked"));
    if (!again) li.setAttribute("data-picked", "");
    return { block: idOf(block), option: again ? null : n, text: again ? null : norm(li.textContent) };
  }
  // The answer reached the page (SSE `answered`, from the page, the capsule or the TUI): the
  // question shows it, and notes.js drops any draft pick on that block.
  function answered(doc, block, reply) {
    block.setAttribute("data-answer", String(reply));
    decorateQuestion(block, block.getAttribute("data-by"));
    doc.dispatchEvent(new root.CustomEvent("bise:answered", { detail: { block: idOf(block), reply: String(reply) } }));
  }
  // ---- review and email (§4.3): approve / skip, as notes through notes.js ----
  // The kit's own controls inside a block carry data-kit-ui: not the agent's content (notes.js
  // leaves them out of quotes and anchors; the swap compares the agent's source, not them).
  const items = (block) => options(block).filter((li) => li.getAttribute("data-id") != null);
  const ui = (el) => el.getAttribute && el.getAttribute("data-kit-ui") != null;
  function button(doc, act, text) {
    const b = doc.createElement("button");
    b.setAttribute("type", "button");
    b.setAttribute("data-act", act);
    b.textContent = text;
    return b;
  }
  function acts(doc, kinds) {
    const bar = doc.createElement("span");
    bar.setAttribute("data-kit-ui", "");
    for (const k of kinds) bar.append(button(doc, k, W[k]));
    return bar;
  }
  // an item's words as one line: block elements apart, inline ones as written, the kit's controls out
  const BLOCKY = /^(p|li|ul|ol|h1|h2|h3|blockquote|pre|table|tr|td|th|caption)$/i;
  function textOf(el) {
    const walk = (e) => nodesOf(e).map((n) => {
      if (isText(n)) return n.textContent;
      if (ui(n)) return "";
      return BLOCKY.test(n.tagName) ? ` ${walk(n)} ` : walk(n);
    }).join("");
    return walk(el).replace(/\s+/g, " ").trim();
  }
  // ---- reply items (slice B, 'watch for me'): a comment somewhere and the drafted answer ----
  // bise never posts: approving opens the thread, with the reply prefilled where the site has an
  // intent URL for it (X), else the reply is copied first ('copy + open').
  const SITES = [[/(^|\.)news\.ycombinator\.com$/, "Hacker News"], [/(^|\.)reddit\.com$/, "Reddit"], [/(^|\.)(x|twitter)\.com$/, "X"], [/(^|\.)bsky\.app$/, "Bluesky"]];
  function replyTarget(thread, text) {
    const m = /^https:\/\/([^/?#]+)([^?#]*)/i.exec(String(thread || ""));
    if (!m) return null;
    const host = m[1].toLowerCase();
    const site = (SITES.find(([re]) => re.test(host)) || [null, host])[1];
    const tweet = site === "X" && /\/status\/(\d+)/.exec(m[2]);
    if (tweet) return { site, open: `https://x.com/intent/post?in_reply_to=${tweet[1]}&text=${encodeURIComponent(text)}`, copy: false };
    return { site, open: thread, copy: true };
  }
  // the drafted reply: the <p>s after the quoted comment (or the last <p>)
  function replyText(li) {
    const cs = kids(li).filter((c) => !ui(c));
    const q = cs.findIndex((c) => /^blockquote$/i.test(c.tagName));
    const ps = (q >= 0 ? cs.slice(q + 1) : cs.slice(-1)).filter((c) => /^p$/i.test(c.tagName));
    return ps.map((p) => textOf(p)).join("\n\n");
  }

  // ---- checklist who and when (slice D): data-who, data-due (YYYY-MM-DD) in words ----
  const DAYS = ["sunday", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday"];
  const MONTHS = ["jan", "feb", "march", "april", "may", "june", "july", "aug", "sept", "oct", "nov", "dec"];
  // {text, late}: "today", "tomorrow", "friday" (this week), "20 may", "late · 3 days"
  function dueWords(due, nowMs) {
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(String(due || ""));
    if (!m) return null;
    const d = new Date(+m[1], +m[2] - 1, +m[3]);
    const now = new Date(nowMs);
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const days = Math.round((d - today) / 86400000);
    if (days < 0) return { text: `late · ${-days === 1 ? "1 day" : `${-days} days`}`, late: true };
    if (days === 0) return { text: "today", late: false };
    if (days === 1) return { text: "tomorrow", late: false };
    if (days < 7) return { text: DAYS[d.getDay()], late: false };
    return { text: `${d.getDate()} ${MONTHS[d.getMonth()]}`, late: false };
  }
  function decorateWho(doc, li, nowMs) {
    const who = li.getAttribute("data-who"), due = dueWords(li.getAttribute("data-due"), nowMs);
    let tag = kids(li).find((c) => ui(c) && c.getAttribute("data-when") != null);
    // data-who="yours": a step only the user can do (roadmap D: the hub cards it when its turn
    // comes); shown 'you · friday'. A done or ticked step is never late: it keeps who did it,
    // not 'late · 3 days'
    const done = li.getAttribute("data-done") != null || li.getAttribute("data-verdict") === "tick";
    // what bise did on its own step (pm's 26): 'bise · drafted · in the page', the last part
    // taking you to the block that holds the draft (data-draft)
    const did = li.getAttribute("data-did"), draft = li.getAttribute("data-draft");
    // a step that is a decision (pm's 35): its question asks it; 'pick in the question' takes you
    // there until the row is done
    const asks = !done && li.getAttribute("data-question");
    const text = [who === "yours" ? W.yours : who, did, draft && W.inThePage, asks && W.inTheQuestion, due && !(done && due.late) && due.text].filter(Boolean).join(" · ");
    if (!text) { if (tag) li.removeChild(tag); li.removeAttribute("data-late"); return; }
    if (!tag) { tag = doc.createElement("span"); tag.setAttribute("data-kit-ui", ""); tag.setAttribute("data-when", ""); li.append(tag); }
    tag.textContent = text;
    const goto = draft || asks;
    if (goto) tag.setAttribute("data-goto", goto); else tag.removeAttribute("data-goto");
    if (due && due.late && li.getAttribute("data-done") == null && li.getAttribute("data-verdict") !== "tick") li.setAttribute("data-late", ""); else li.removeAttribute("data-late");
  }

  // ---- an item's agent (slice C): data-agent, live from SSE 'agent' {name, status, note?} ----
  // Before any event the item shows the name alone (the agent may not exist yet, or the stream is
  // not up): "∿ fix login"; then "∿ fix login · working · reading the Safari logs".
  // a glyph per status (ambient-lead m_5424, amb-web m_5425): '∿ fix login working · <note>',
  // '∿ fix login waiting', '✓ fix login done', '▲ fix login failed', '? fix login needs you · <note>';
  // the note only while it works or waits for the user
  const STATUS = { working: ["∿", "working"], idle: ["∿", "waiting"], done: ["✓", "done"], blocked: ["?", "needs you"], failed: ["▲", "failed"] };
  const agents = new Map();
  function agentLine(name, st) {
    const s = st && STATUS[st.status];
    if (!s) return `∿ ${said(name)}`;
    const line = `${s[0]} ${said(name)} ${s[1]}`;
    return st.note && (st.status === "working" || st.status === "blocked") ? `${line} · ${st.note}` : line;
  }
  function decorateAgent(doc, li) {
    const name = li.getAttribute("data-agent");
    let tag = kids(li).find((c) => ui(c) && c.getAttribute("data-agent-line") != null);
    if (!name) { if (tag) li.removeChild(tag); return; }
    if (!tag) { tag = doc.createElement("span"); tag.setAttribute("data-kit-ui", ""); tag.setAttribute("data-agent-line", ""); li.append(tag); }
    const st = agents.get(name);
    tag.textContent = agentLine(name, st);
    if (st && STATUS[st.status]) tag.setAttribute("data-status", st.status); else tag.removeAttribute("data-status");
  }

  // the items "approve N" approves: no verdict yet, and not edited by the user (those go with send)
  const pending = (block) => items(block).filter((li) => li.getAttribute("data-verdict") == null && li.getAttribute("data-edited") == null);
  // the buttons each item kind gets (review: approve / skip; compare: ♡ ✗ pick; checklist: a tick)
  const ITEM_ACTS = { review: ["approve", "skip"], compare: ["keep", "drop", "pick"], checklist: ["tick"] };
  function decorateItems(doc, block, kit) {
    for (const li of items(block)) {
      if (!kids(li).some((c) => ui(c) && c.getAttribute("data-agent-line") == null && c.getAttribute("data-went") == null)) {
        // feedback to act on (data-verb="start", roadmap C): start an agent / answer <who> / skip
        // a mention to answer (data-verb="reply", mentions): send the drafted answer / start an agent / skip
        const v = kit === "review" && block.getAttribute("data-verb");
        const bar = acts(doc, v === "start" ? ["start", "approve", "skip"] : v === "reply" ? ["approve", "start", "skip"] : ITEM_ACTS[kit]);
        // a checklist's tick goes first, like a box; an item the agent marked done is ticked
        if (kit === "checklist") li.insertBefore(bar, li.firstChild || (li.nodes && li.nodes[0]) || null);
        else li.append(bar);
      }
      if (kit === "checklist" && li.getAttribute("data-done") != null && li.getAttribute("data-verdict") == null) li.setAttribute("data-verdict", "tick");
      if (kit === "checklist") decorateWho(doc, li, Date.now());
      decorateAgent(doc, li);
      // a reply item: its approve says what it does ('reply on X', or 'copy + open' on Hacker News)
      // and a review of drafts that leave (data-verb="send") says 'send', never 'approve'
      const t = kit === "review" && replyTarget(li.getAttribute("data-reply"), "");
      const ok = kit === "review" && kids(li).filter(ui).flatMap((b) => kids(b)).find((b) => b.getAttribute("data-act") === "approve");
      // and what bise keeps about the user (data-verb="keep") says 'keep' / 'strike'
      // a read-only place (data-verb="open-reply", roadmap B): 'open with the reply', never 'send'
      const verb = block.getAttribute("data-verb");
      if (ok) ok.textContent = t ? W.openOn(t.site) : verb === "open-reply" ? W.openReply : verb === "start" ? W.answer(reporter(li)) : sends(block) ? W.send : keeps(block) ? W.keepRule : W.approve;
      // once an agent works on it (data-agent), 'start an agent' goes: its live line says the rest
      const go = (verb === "start" || verb === "reply") && kids(li).filter(ui).flatMap((b) => kids(b)).find((b) => b.getAttribute("data-act") === "start");
      if (go) {
        go.hidden = li.getAttribute("data-agent") != null;
        if (li.getAttribute("data-starting") != null) { go.textContent = W.starting; go.setAttribute("disabled", ""); }
        else { go.textContent = W.start; go.removeAttribute("disabled"); }
      }
      const no = keeps(block) && kids(li).filter(ui).flatMap((b) => kids(b)).find((b) => b.getAttribute("data-act") === "skip");
      if (no) no.textContent = W.strike;
    }
  }
  const keeps = (block) => block.getAttribute("data-verb") === "keep";
  // who reported it: the first words of the item's 'who · where · when' line ("Benjamin")
  function reporter(li) {
    const first = kids(li).find((c) => !ui(c));
    const who = first ? textOf(first).split("·")[0].trim() : "";
    return who.split(/\s+/)[0] || "";
  }
  // a review whose items are drafts bise sends once the user says so (mail replies, messages)
  // (and mentions to answer, data-verb="reply": the same 'send', plus 'start an agent')
  const sends = (block) => ["send", "reply"].includes(block.getAttribute("data-verb"));
  function decorateReview(doc, block) {
    decorateItems(doc, block, "review");
    let all = kids(block).find(ui);
    if (!all) { all = doc.createElement("p"); all.setAttribute("data-kit-ui", ""); all.append(button(doc, "approve-all", "")); block.append(all); }
    const n = pending(block).length;
    const b = kids(all)[0];
    b.textContent = sends(block) ? W.sendAll(n) : W.approveAll(n);
    if (n) b.removeAttribute("disabled"); else b.setAttribute("disabled", "");
  }
  // an email always leaves: 'send' / 'skip' (its note is still an approve)
  function decorateEmail(doc, block) {
    if (!kids(block).some((c) => ui(c) && c.getAttribute("data-went") == null)) {
      const bar = acts(doc, ["approve", "skip"]);
      block.append(bar);
    }
    // a draft bise never sends (data-verb="draft": a Kit newsletter, a mail he finishes in Gmail):
    // 'put it in my drafts'; otherwise the mail leaves: 'send'
    const ok = kids(block).filter((c) => ui(c) && c.getAttribute("data-went") == null).flatMap((b) => kids(b)).find((b) => b.getAttribute("data-act") === "approve");
    if (ok) ok.textContent = block.getAttribute("data-verb") === "draft" ? W.putInDrafts : W.send;
  }
  // a write his word triggers (lead m_6191: close OPS-12 in Linear): its own approve / skip, the
  // approve saying the write (data-do), never a side effect of another send
  function decorateAction(doc, block) {
    if (!kids(block).some((c) => ui(c) && c.getAttribute("data-went") == null)) block.append(acts(doc, ["approve", "skip"]));
    const ok = kids(block).filter((c) => ui(c) && c.getAttribute("data-went") == null).flatMap((b) => kids(b)).find((b) => b.getAttribute("data-act") === "approve");
    if (ok) ok.textContent = block.getAttribute("data-do") || W.approve;
  }
  // react to an item (review approve/skip, compare keep/drop/pick, checklist tick) or a whole
  // email (item null); the same again takes it back; a pick is one per block (the last wins).
  // Returns {block, item, kind (null: taken back), text}, or null.
  function react(block, item, kind) {
    const target = item == null ? block : items(block).find((li) => li.getAttribute("data-id") === item);
    if (!target) return null;
    const again = target.getAttribute("data-verdict") === kind;
    if (kind === "pick" && !again) for (const li of items(block)) if (li.getAttribute("data-verdict") === "pick") li.removeAttribute("data-verdict");
    if (again) target.removeAttribute("data-verdict"); else target.setAttribute("data-verdict", kind);
    const text = textOf(target);
    return { block: idOf(block), item: item == null ? null : item, kind: again ? null : kind, text };
  }
  // "approve N": every pending item, one reaction each
  function approveAll(block) {
    return pending(block).map((li) => react(block, li.getAttribute("data-id"), "approve"));
  }

  // ---- message (slice A): a draft that leaves through a channel; `copy` gives its markup ----
  const nodesOf = (e) => Array.from(e.childNodes || e.nodes || []);
  const isText = (n) => n.nodeType === 3 || (n.tagName == null && n.textContent != null);
  // The block as Slack mrkdwn, from what the page shows now (the user's edits included):
  // strong *x*, em _x_, del ~x~, code `x`, links <url|text>, list items "• ", paragraphs apart.
  function toSlack(block) {
    const inline = (e) => nodesOf(e).map((n) => {
      if (isText(n)) return n.textContent.replace(/\s+/g, " ");
      if (ui(n)) return "";
      const t = n.tagName.toLowerCase(), x = inline(n);
      if (t === "strong" || t === "b") return `*${x.trim()}*`;
      if (t === "em" || t === "i") return `_${x.trim()}_`;
      if (t === "del" || t === "s") return `~${x.trim()}~`;
      if (t === "code" || t === "kbd") return "`" + x + "`";
      if (t === "a") { const h = n.getAttribute("href") || ""; return /^(https?:|mailto:)/.test(h) ? `<${h}|${x.trim()}>` : x; }
      if (t === "br") return "\n";
      return x;
    }).join("");
    const out = [];
    for (const c of kids(block)) {
      if (ui(c)) continue;
      const t = c.tagName.toLowerCase();
      if (t === "ul" || t === "ol") out.push(kids(c).map((li, i) => `${t === "ol" ? `${i + 1}.` : "•"} ${inline(li).trim()}`).join("\n"));
      else if (t === "pre") out.push("```\n" + c.textContent.replace(/\n$/, "") + "\n```");
      else if (t === "blockquote") out.push(inline(c).trim().split("\n").map((l) => `> ${l}`).join("\n"));
      else out.push(inline(c).trim());
    }
    return out.filter(Boolean).join("\n\n");
  }
  // a message says what happens next (ambient m_5589): bise can post it (data-verb="send"):
  // 'send' / 'skip'; else 'copy' and, with its channel's link (data-open), 'open in Slack'
  function decorateMessage(doc, block) {
    if (kids(block).some((c) => ui(c) && c.getAttribute("data-went") == null)) return;
    if (block.getAttribute("data-verb") === "send") {
      const bar = acts(doc, ["approve", "skip"]);
      kids(bar)[0].textContent = W.send;
      block.append(bar);
      return;
    }
    const bar = acts(doc, ["copy"]);
    const href = safeLink(block.getAttribute("data-open"));
    if (href) {
      const a = doc.createElement("a");
      a.setAttribute("href", href);
      a.setAttribute("target", "_blank");
      a.setAttribute("rel", "noopener");
      a.setAttribute("data-act-link", "");
      a.textContent = W.openInPlace(String(block.getAttribute("data-to") || "").split("·")[0].trim() || "Slack");
      bar.append(a);
    }
    block.append(bar);
  }

  // ---- where a draft went (slice A): meta.went [{kind, ref, url, at, block?}], from
  // `sb page publish --went <kind>:<ref>[@<block>]=<url>`, and on SSE 'went' {went}. A draft put in
  // the user's tool shows "in your Gmail drafts · open"; a message copied "copied · open #launch".
  const DRAFTS = { "gmail-draft": "in your Gmail drafts", "outlook-draft": "in your Outlook drafts", "kit-draft": "in your Kit drafts" };
  const safeLink = (u) => (/^(https:|slack:|mailto:)/.test(String(u || "")) ? String(u) : null);
  // the channel a message goes to, from its data-to ("Slack · #launch" → "#launch")
  const channelOf = (block) => String(block.getAttribute("data-to") || "").split("·").pop().trim();
  function wentWords(entry, block) {
    const kind = String((entry && entry.kind) || "");
    const href = safeLink(entry && entry.url) || (block.getAttribute("data-kit") === "message" ? safeLink(block.getAttribute("data-open")) : null);
    const ch = block.getAttribute("data-kit") === "message" ? channelOf(block) : "";
    const text = DRAFTS[kind] || (kind === "copy" ? W.copied : kind === "slack" ? W.postedIn(ch || "Slack") : kind.replace(/[-_]+/g, " "));
    return { text, open: href ? (ch && (kind === "copy" || kind === "slack") ? W.openIn(ch) : W.open) : null, href };
  }
  // which block each entry belongs to: its own block, else a draft → the first email block and
  // slack/copy → the first message block (either, for another kind); the last entry wins
  function wentFor(went, blocks) {
    const out = new Map();
    const first = (kits) => blocks.find((b) => kits.includes(b.getAttribute("data-kit")));
    for (const e of went || []) {
      if (!e || !e.kind) continue;
      const k = String(e.kind);
      const b = e.block ? blocks.find((x) => idOf(x) === String(e.block)) : first(DRAFTS[k] ? ["email"] : k === "slack" || k === "copy" ? ["message"] : ["email", "message"]);
      if (!b) continue;
      const was = out.get(idOf(b));
      if (!was || (e.at || 0) >= (was.at || 0)) out.set(idOf(b), e);
    }
    return out;
  }
  function drawWent(doc, block, entry) {
    let line = kids(block).find((c) => ui(c) && c.getAttribute("data-went") != null);
    if (!entry) { if (line) block.removeChild(line); return; }
    if (!line) { line = doc.createElement("p"); line.setAttribute("data-kit-ui", ""); line.setAttribute("data-went", ""); block.append(line); }
    const w = wentWords(entry, block);
    line.setAttribute("data-went", String(entry.kind));
    const t = doc.createElement("span");
    t.textContent = w.text;
    const parts = [t];
    if (w.href) {
      const a = doc.createElement("a");
      a.setAttribute("href", w.href);
      a.setAttribute("target", "_blank");
      a.setAttribute("rel", "noopener");
      a.textContent = w.open;
      parts.push(a);
    }
    for (const c of kids(line)) line.removeChild(c);
    line.append(...parts);
  }
  function applyWent(doc, blocks, went) {
    const map = wentFor(went, blocks);
    for (const b of blocks) {
      const kit = b.getAttribute("data-kit");
      if (kit !== "email" && kit !== "message") continue;
      const e = map.get(idOf(b));
      // a local "copied" stays until the server says where the draft went
      const line = kids(b).find((c) => ui(c) && c.getAttribute("data-went") != null);
      if (!e && line && line.getAttribute("data-went") === "copy") continue;
      drawWent(doc, b, e || null);
    }
  }

  // the parts the user may edit in place (amb-web's notes.js edits only inside [data-editable]):
  // a review item's proposal (after its 'who · what · when' line, never the quoted comment), an
  // email's subject and body (not to/cc), a message's text. Set by the kit, never by the agent.
  function editableParts(block) {
    const kit = block.getAttribute("data-kit");
    const own = (e) => kids(e).filter((c) => !ui(c));
    if (kit === "message") return own(block);
    if (kit === "email") return own(block).filter((c) => { const f = c.getAttribute("data-field"); return f == null || f === "subject"; });
    if (kit === "review") return items(block).flatMap((li) => own(li).slice(1).filter((c) => c.tagName.toLowerCase() !== "blockquote"));
    return [];
  }
  function markEditable(block) { for (const e of editableParts(block)) e.setAttribute("data-editable", ""); }

  function decorate(blocks, agent, doc) {
    for (const b of blocks) {
      const kit = b.getAttribute("data-kit");
      markEditable(b);
      if (kit === "table") decorateTable(b);
      else if (kit === "question") decorateQuestion(b, agent);
      else if (kit === "review" && doc) decorateReview(doc, b);
      else if (kit === "email" && doc) decorateEmail(doc, b);
      else if (kit === "message" && doc) decorateMessage(doc, b);
      else if (kit === "action" && doc) decorateAction(doc, b);
      else if ((kit === "compare" || kit === "checklist") && doc) decorateItems(doc, b, kit);
      else if (kit === "ui" && doc) decorateUi(doc, b);
    }
  }

  // ---- ui blocks (pages-ui): the kit's behaviours, named by data-ui; a page carries no code ----
  // tabs: buttons in a .k-tabs bar (data-tab="x") show the pane .k-pane[data-tab="x"] of the same
  // data-ui="tabs" element (the n-th pane when they have no data-tab); without kit.js every pane shows
  const own = (root0, el, sel) => Array.from(el.querySelectorAll(sel)).filter((x) => x.closest("[data-ui=\"tabs\"]") === root0);
  function showTab(tabs, i) {
    const bs = own(tabs, tabs, ".k-tabs > button"), ps = own(tabs, tabs, ".k-pane");
    const key = bs[i] && bs[i].getAttribute("data-tab");
    bs.forEach((b, j) => b.setAttribute("aria-selected", String(j === i)));
    ps.forEach((p, j) => { if ((key ? p.getAttribute("data-tab") === key : j === i)) p.setAttribute("data-on", ""); else p.removeAttribute("data-on"); });
  }
  // term: the frame's own light/dark switch (a mock is read in both); system until one is picked
  const TERM = ["dark", "light"];
  // pick: rich options (.k-option or [data-pick]); a click picks it, again takes it back, one per
  // block: the same reaction and note as a compare's pick (bise:react {block, item, kind, text})
  function pickOption(block, opt) {
    const opts = Array.from(block.querySelectorAll("[data-pick]"));
    const again = opt.getAttribute("data-verdict") === "pick";
    opts.forEach((o) => { o.removeAttribute("data-verdict"); o.setAttribute("aria-pressed", "false"); });
    if (!again) { opt.setAttribute("data-verdict", "pick"); opt.setAttribute("aria-pressed", "true"); }
    const head = opt.querySelector("h3, b, strong");
    const text = norm((head || opt).textContent || "").replace(/<[^>]*>/g, "");
    return { block: idOf(block), item: opt.getAttribute("data-pick"), kind: again ? null : "pick", text };
  }
  function decorateUi(doc, block) {
    if (!block.querySelectorAll) return;
    for (const tabs of block.querySelectorAll("[data-ui=\"tabs\"]")) {
      if (tabs.hasAttribute("data-ready")) continue;
      tabs.setAttribute("data-ready", "");
      const bs = own(tabs, tabs, ".k-tabs > button");
      bs.forEach((b, i) => { b.setAttribute("type", "button"); b.setAttribute("role", "tab"); b.addEventListener("click", () => showTab(tabs, i)); });
      const on = bs.findIndex((b) => b.getAttribute("aria-selected") === "true");
      showTab(tabs, on < 0 ? 0 : on);
    }
    for (const term of block.querySelectorAll("[data-ui=\"term\"]")) {
      if (term.querySelector("[data-kit-ui] > [data-act^=\"term-\"]")) continue;
      const sw = doc.createElement("span");
      sw.setAttribute("data-kit-ui", "");
      // a two-state toggle: the system's scheme until one is picked
      const sys = root.matchMedia && root.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
      const now = () => term.getAttribute("data-term") || sys;
      for (const t of TERM) {
        const b = button(doc, "term-" + t, t);
        b.setAttribute("aria-pressed", String(now() === t));
        b.addEventListener("click", () => {
          term.setAttribute("data-term", t);
          for (const x of sw.children) x.setAttribute("aria-pressed", String(x.textContent === t));
        });
        sw.append(b);
      }
      // in the frame's bar or its first tab bar (made when the mock has none), right of its tabs
      let bar = term.querySelector(".k-bar, .k-tabs");
      if (!bar) { bar = doc.createElement("div"); bar.className = "k-bar"; bar.setAttribute("data-kit-ui", ""); term.insertBefore(bar, term.firstChild); }
      bar.append(sw);
    }
    // a static copy (the public export) takes no picks: no hub hears them, so they don't look clickable
    const still = block.closest && block.closest("[data-static]");
    for (const pick of block.querySelectorAll("[data-ui=\"pick\"]")) {
      if (still || pick.hasAttribute("data-ready")) continue;
      pick.setAttribute("data-ready", "");
      for (const opt of pick.querySelectorAll("[data-pick]")) {
        opt.setAttribute("role", "button");
        opt.setAttribute("tabindex", "0");
        if (!opt.hasAttribute("aria-pressed")) opt.setAttribute("aria-pressed", String(opt.getAttribute("data-verdict") === "pick"));
        const go = () => doc.dispatchEvent(new root.CustomEvent("bise:react", { detail: pickOption(block, opt) }));
        opt.addEventListener("click", (e) => { if (!(e.target.closest && e.target.closest("a"))) go(); });
        opt.addEventListener("keydown", (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); go(); } });
      }
    }
  }

  // A new version on screen: swap its blocks in (by the hashes in meta when it has both versions),
  // decorate them, mark what changed against the version before, and tell notes.js. Returns the
  // swap's {changed, removed}.
  function applyVersion(doc, main, incoming, meta, from, to, opts) {
    const o = opts || {};
    const cur = versionOf(meta, to), was = versionOf(meta, from);
    const res = swap(main, incoming, cur && was ? diffBlocks(was.blocks, cur.blocks).changed : null, o.view);
    main.setAttribute("data-version", String(to));
    main.setAttribute("data-latest", String(to));
    // the ui blocks' rules of the version on screen (the server serves them, scoped, as page.css)
    const css = doc && doc.querySelector && doc.querySelector("link[data-page-css]");
    const page = main.getAttribute("data-page");
    if (css && page) css.setAttribute("href", "/p/" + encodeURIComponent(page) + "/v/" + to + "/page.css");
    const blocks = kids(main).filter(isBlock);
    decorate(blocks, o.agent, doc);
    const marked = new Set(cur && versionOf(meta, to - 1) ? changedIn(meta, to) : res.changed);
    for (const b of blocks) if (marked.has(idOf(b))) b.setAttribute("data-changed", ""); else b.removeAttribute("data-changed");
    doc.dispatchEvent(new root.CustomEvent("bise:swapped", { detail: { version: to, changed: res.changed, removed: res.removed } }));
    return { ...res, marked: [...marked] };
  }

  // a step ticked elsewhere (roadmap D: 'done' on its card, by voice through `sb page tick`):
  // SSE 'ticked' {block, item}; the row shows ticked, nothing goes back as a note from here
  function ticked(doc, block, item) {
    const li = items(block).find((x) => x.getAttribute("data-id") === item);
    if (!li) return false;
    li.setAttribute("data-verdict", "tick");
    li.setAttribute("data-ticked", "");
    decorateWho(doc, li, Date.now());
    doc.dispatchEvent(new root.CustomEvent("bise:ticked", { detail: { block: idOf(block), item } }));
    return true;
  }

  // a reply opened from its item (roadmap B): the item says what happened, 'opened on X' (the
  // reply prefilled) or 'copied · open the thread' (the link, to open it again)
  function replyWent(doc, li, t) {
    let line = kids(li).find((c) => ui(c) && c.getAttribute("data-went") != null);
    if (!line) { line = doc.createElement("span"); line.setAttribute("data-kit-ui", ""); line.setAttribute("data-went", "reply"); li.append(line); }
    for (const c of kids(line)) line.removeChild(c);
    const s = doc.createElement("span");
    s.textContent = t.copy ? W.copiedOpen : W.openedOn(t.site);
    line.append(s);
    const href = safeLink(li.getAttribute("data-reply"));
    if (t.copy && href) {
      const a = doc.createElement("a");
      a.setAttribute("href", href);
      a.setAttribute("target", "_blank");
      a.setAttribute("rel", "noopener");
      a.textContent = W.openThread;
      line.append(a);
    }
    return line;
  }

  // the element a page link's #<id> names: an item (li[data-id]) first, else a block
  function targetOf(blocks, id) {
    if (!id) return null;
    for (const b of blocks) for (const li of items(b)) if (li.getAttribute("data-id") === id) return li;
    return blocks.find((b) => idOf(b) === id) || null;
  }

  // meta.taste {rules: N} (sb page publish --taste, amb-core m_5554): 'following your taste · 4 rules'
  function tasteWords(t) {
    const n = t && Number(t.rules);
    if (!t || !Number.isFinite(n) || n < 1) return null;
    return `${W.followingTaste} · ${n} ${n === 1 ? "rule" : "rules"}`;
  }

  // ---- a watching page (roadmap B: a standing order on a timer) ----
  // meta.watch / SSE 'watch' {timer, every, checked_ms, until_ms?}: the frame says
  // 'watching · checked 3 min ago · until tomorrow 18:00' and has 'stop'
  const hm = (d) => `${d.getHours()}:${String(d.getMinutes()).padStart(2, "0")}`;
  function untilWords(untilMs, nowMs) {
    if (!untilMs) return null;
    const d = new Date(untilMs), now = new Date(nowMs);
    const day = (x) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
    const days = Math.round((day(d) - day(now)) / 86400000);
    if (days <= 0) return `until ${hm(d)}`;
    if (days === 1) return `until tomorrow ${hm(d)}`;
    if (days < 7) return `until ${DAYS[d.getDay()]} ${hm(d)}`;
    return `until ${d.getDate()} ${MONTHS[d.getMonth()]}`;
  }
  function watchLine(w, nowMs) {
    if (!w) return null;
    const parts = [W.watching];
    if (w.checked_ms) parts.push(`checked ${ago(w.checked_ms, nowMs)}`);
    const u = untilWords(w.until_ms, nowMs);
    if (u) parts.push(u);
    return parts.join(" · ");
  }

  const BiseKit = { W, themeOf, ticked, untilWords, watchLine, replyWent, tasteWords, targetOf, said, intentLines, since, ago, versionOf, diffBlocks, changedIn, isNumeric, numericColumns, swap, applyVersion, remember, decorate, pick, answered, react, approveAll, toSlack, replyTarget, replyText, dueWords, agentLine, agents, wentWords, wentFor, applyWent, editableParts, options, norm };
  root.BiseKit = BiseKit;

  // ---- the browser: the frame, the stream, the keys ----
  const doc = root.document;
  if (!doc || !doc.addEventListener || !doc.createElement || !doc.body && doc.readyState !== "loading") return;

  function start() {
    const page = doc.getElementById("bise-page");
    const home = doc.getElementById("bise-home");
    // a static copy (the public export, pages-ui): no hub behind it, read-only
    if (page && page.hasAttribute("data-static")) startStatic(page);
    else if (page) startPage(page);
    else if (home && !home.hasAttribute("data-static")) startHome(home);
  }

  // The public export's page: the frame says whose and which version, the blocks get their kit
  // behaviours; no stream, no notes, no send (nothing here talks to a hub).
  function startStatic(main) {
    const d = main.dataset;
    const f = frame(d.title || d.page || "");
    f.setAttribute("data-static", "");
    const at = parseInt(d.at, 10);
    const words = [d.agent ? W.by(d.agent) : null, d.version ? "v" + d.version : null, at ? ago(at, Date.now()) : null].filter(Boolean);
    f.append(el("span", { class: "m" }, words.join(" · ")));
    decorate(Array.from(main.querySelectorAll(":scope > [data-kit][data-id]")), d.agent || "", doc);
  }

  function el(tag, attrs, text) {
    const e = doc.createElement(tag);
    for (const [k, v] of Object.entries(attrs || {})) if (v != null) e.setAttribute(k, v);
    if (text != null) e.textContent = text;
    return e;
  }

  function frame(title) {
    const f = el("header", { class: "bise-frame", role: "banner" });
    // the pearl at 36 px (SPEC §1, round 8): amb-web draws its shader into [data-bise-pearl]; the
    // ':*' stays as the fallback until it does
    f.append(el("span", { class: "k", "aria-hidden": "true", "data-bise-pearl": "36" }, ":*"), el("span", { class: "t" }, title));
    doc.body.prepend(f);
    return f;
  }

  function startPage(main) {
    const d = main.dataset;
    const id = d.page, agent = d.agent || "", title = d.title || id;
    // version 0: a page started with `sb page start`, nothing published yet (state writing)
    const num = (x, dflt) => { const v = parseInt(x, 10); return Number.isNaN(v) ? dflt : v; };
    let shown = num(d.version, 1);
    const latestAtLoad = num(d.latest, shown);
    const follow = shown === latestAtLoad; // an older version stays put
    let meta = null;

    const f = frame(title);
    f.setAttribute("data-state", d.state || "ready");
    const m = el("span", { class: "m" });
    // to the page of what bise keeps about him (about-you), where he edits the rules
    const tasteLink = el("a", { class: "taste", href: "/p/about-you", hidden: "" });
    const publicChip = el("span", { class: "pub", title: "published with --public: copied to your public mirror", hidden: main.hasAttribute("data-public") ? null : "" }, "public");
    const st = el("span", { class: "st" });
    const stWords = el("span", {}, W.updating(agent));
    const stProgress = el("span", { class: "p" });
    st.append(stWords, stProgress);
    // the page before its first version (ambient m_5095): its title, what you asked, then what the
    // agent is doing, live (SSE progress), newest at the bottom; a failure says so
    const placeholder = el("div", { "data-kit-ui": "", class: "bise-writing" });
    const phTitle = el("h1", {}, title);
    const phAsk = el("p", { class: "ask" }, d.ask ? W.asked(d.ask) : null);
    if (!d.ask) phAsk.hidden = true;
    const phMore = el("p", { class: "more", hidden: "" });
    const phList = el("ol", { class: "intents" });
    const phFail = el("p", { class: "fail", hidden: "" });
    placeholder.append(phTitle, phAsk, phMore, phList, phFail);
    let intents = [];
    let failure = null;
    function drawIntents() {
      const working = main.dataset.state !== "ready";
      const { earlier, lines } = intentLines(intents, working);
      phMore.hidden = earlier === 0;
      phMore.textContent = W.earlier(earlier);
      const items = lines.length ? lines : working ? [{ text: W.onIt, current: true }] : [];
      phList.replaceChildren(...items.map((x) => el("li", { "data-current": x.current ? "" : null }, x.text)));
      phFail.hidden = working;
      phFail.textContent = W.failed(failure);
    }
    const startedMs = num(d.started, Date.now());
    const sp = el("span", { class: "sp" });
    const wc = el("button", { type: "button", disabled: "" }, W.changed);
    // the version: "v3 ▾" opens the list of versions (ambient m_4918), older ones read-only
    const vbox = el("span", { class: "v" });
    const vb = el("button", { type: "button", "aria-haspopup": "menu", "aria-expanded": "false", "aria-controls": "bise-vmenu", "aria-label": W.versions }, `v${shown}`);
    const menu = el("span", { class: "vmenu", role: "menu", id: "bise-vmenu", "aria-label": W.versions, hidden: "" });
    vbox.append(vb, menu);
    const latest = el("a", { class: "b", href: `/p/${id}`, hidden: "" }, W.latest);
    const n = el("span", { class: "n", "data-bise-notes-count": "" });
    const send = el("button", { type: "button", class: "send", "data-bise-send": "", disabled: "" }, W.send);
    // a watching page: 'watching · checked 3 min ago · until tomorrow 18:00' and 'stop' (a stop
    // note to main through notes.js: document bise:stop {page, timer})
    const wbox = el("span", { class: "w", hidden: "" });
    const wWords = el("span", {});
    const wStop = el("button", { type: "button", class: "stop" }, W.stop);
    wbox.append(wWords, wStop);
    let watch = null;
    function drawWatch() {
      const line = main.dataset.state === "writing" ? null : watchLine(watch, Date.now());
      wbox.hidden = !line;
      wWords.textContent = line || "";
    }
    wStop.addEventListener("click", () => {
      if (!watch) return;
      wStop.disabled = true;
      doc.dispatchEvent(new root.CustomEvent("bise:stop", { detail: { page: id, timer: watch.timer == null ? null : watch.timer } }));
    });
    f.append(m, tasteLink, publicChip, st, wbox, sp, wc, vbox, latest, n, send);
    // the version menu (amb-web m_5684): ↓ ⏎ space on the button open it on the shown version;
    // ↑ ↓ Home End move, ⏎ space pick, esc closes back on the button, tab closes
    const menuItems = () => Array.from(menu.querySelectorAll('[role="menuitemradio"]'));
    const openMenu = (on, focusItem) => {
      menu.hidden = !on;
      vb.setAttribute("aria-expanded", String(on));
      if (on && focusItem) { const its = menuItems(); (its.find((a) => a.getAttribute("aria-checked") === "true") || its[0] || vb).focus(); }
    };
    vb.addEventListener("click", (e) => { e.stopPropagation(); if (!vb.disabled) openMenu(menu.hidden, e.detail === 0); });
    vb.addEventListener("keydown", (e) => {
      if (vb.disabled || !["ArrowDown", "ArrowUp"].includes(e.key)) return;
      e.preventDefault();
      openMenu(true, true);
    });
    menu.addEventListener("keydown", (e) => {
      const its = menuItems(), i = its.indexOf(doc.activeElement);
      const to = { ArrowDown: i + 1, ArrowUp: i - 1, Home: 0, End: its.length - 1 }[e.key];
      if (to != null) { e.preventDefault(); its[(to + its.length) % its.length].focus(); return; }
      if (e.key === " " && i >= 0) { e.preventDefault(); its[i].click(); return; }
      if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); openMenu(false); vb.focus(); return; }
      if (e.key === "Tab") openMenu(false);
    });
    doc.addEventListener("click", (e) => { if (!menu.hidden && !vbox.contains(e.target)) openMenu(false); });
    doc.addEventListener("keydown", (e) => { if (e.key === "Escape" && !menu.hidden) { openMenu(false); vb.focus(); } });

    const blocks = () => Array.from(main.querySelectorAll(":scope > [data-kit][data-id]"));
    blocks().forEach(remember);
    decorate(blocks(), agent, doc);
    // a link to an item (`/p/<id>#<item>`: the morning page, a card's page url) opens on it,
    // lit a moment; a block id works the same
    const target = targetOf(blocks(), decodeURIComponent(String((root.location && root.location.hash) || "").slice(1)));
    if (target) {
      target.scrollIntoView({ block: "center" });
      target.setAttribute("data-lit", "");
      setTimeout(() => target.removeAttribute("data-lit"), 2400);
    }

    function drawMeta() {
      const v = versionOf(meta, shown);
      const latestN = meta && meta.versions.length ? Math.max(...meta.versions.map((x) => x.n)) : latestAtLoad;
      const parts = [W.by(agent)];
      if (main.dataset.state === "writing") {
        // "by launch recap · writing · 12 s", counting up
        const t0 = meta && meta.created_ms ? meta.created_ms : startedMs;
        parts.push(W.writingFrame, since(t0, Date.now()));
      } else if (v && v.at_ms) parts.push(ago(v.at_ms, Date.now()));
      m.textContent = parts.join(" · ");
      // the agent read his taste file (sb page publish --taste): 'following your taste · 4 rules'
      const tw = tasteWords(meta && meta.taste);
      tasteLink.hidden = !tw || main.dataset.state === "writing";
      tasteLink.textContent = tw || "";
      // --public (pages-ui): a chip, so he sees it goes to his public mirror
      publicChip.hidden = !((meta && meta.public) || main.hasAttribute("data-public"));
      doc.title = main.dataset.state === "writing" ? W.tabWriting(title) : title;
      latest.hidden = shown >= latestN;
      // at v1 there is nothing to compare or go back to: no 'what changed', no 'v1' chip
      wc.hidden = latestN < 2;
      vbox.hidden = latestN < 2;
      vb.textContent = (shown < latestN ? W.older(shown, latestN) : `v${shown}`) + (latestN > 1 ? " ▾" : "");
      if (latestN > 1) vb.removeAttribute("disabled"); else vb.setAttribute("disabled", "");
      const vs = meta ? meta.versions.slice().sort((a, b) => b.n - a.n) : [];
      menu.replaceChildren(...vs.map((x) => {
        // a real menu (amb-web m_5684): radio items, the shown version checked, focus moved by keys
        const a = el("a", { role: "menuitemradio", tabindex: "-1", "aria-checked": String(x.n === shown), href: x.n === latestN ? `/p/${id}` : `/p/${id}/v/${x.n}` }, `v${x.n}` + (x.at_ms ? ` · ${ago(x.at_ms, Date.now())}` : ""));
        if (x.n === shown) a.setAttribute("aria-current", "page");
        return a;
      }));
    }
    function markChanged(ids) {
      const set = new Set(ids);
      for (const b of blocks()) if (set.has(b.dataset.id)) b.setAttribute("data-changed", ""); else b.removeAttribute("data-changed");
      if (ids.length) wc.removeAttribute("disabled"); else { wc.setAttribute("disabled", ""); wc.classList.remove("on"); doc.body.classList.remove("bise-changes"); }
    }
    wc.addEventListener("click", () => { wc.classList.toggle("on"); doc.body.classList.toggle("bise-changes"); });

    async function loadMeta() {
      try { const r = await fetch(`/p/${id}/meta`, { cache: "no-store" }); if (r.ok) meta = await r.json(); } catch (_) { /* offline: the page still reads */ }
      return meta;
    }
    // where the page's drafts went (meta.went, then SSE 'went'): the whole list each time
    let went = [];
    loadMeta().then(() => {
      drawMeta();
      markChanged(changedIn(meta, shown));
      if (meta && meta.went) { went = meta.went; applyWent(doc, blocks(), went); }
      if (meta && meta.watch) { watch = meta.watch; drawWatch(); }
    });
    // the meta line every 30 s; every second while writing (the time counts up)
    let tick = 0;
    setInterval(() => { tick++; if (main.dataset.state === "writing" || tick % 30 === 0) { drawMeta(); drawWatch(); } }, 1000);

    // the reading position: the first block whose bottom is under the frame
    const view = {
      anchor() {
        const fb = f.getBoundingClientRect().bottom;
        for (const b of blocks()) { const r = b.getBoundingClientRect(); if (r.bottom > fb) return { id: b.dataset.id, top: r.top }; }
        return null;
      },
      topOf(bid) { const b = main.querySelector(`:scope > [data-id="${CSS.escape(bid)}"]`); return b ? b.getBoundingClientRect().top : null; },
      scrollBy(dy) { root.scrollBy(0, dy); },
    };

    let swapping = Promise.resolve();
    async function goTo(v) {
      if (!follow || v <= shown) return;
      const r = await fetch(`/p/${id}/v/${v}/body`, { cache: "no-store" });
      if (!r.ok) return;
      const t = doc.createElement("template");
      t.innerHTML = await r.text();
      const incoming = Array.from(t.content.children).filter(isBlock);
      await loadMeta();
      const res = applyVersion(doc, main, incoming, meta, shown, v, { view, agent });
      if (placeholder.parentNode && blocks().length) placeholder.remove();
      shown = v;
      drawMeta();
      markChanged(res.marked);
      if (meta && meta.went) went = meta.went;
      applyWent(doc, blocks(), went);
    }

    // the frame and the placeholder follow the state: writing (no version yet), updating (notes
    // sent), ready; a page still at version 0 when it is ready again: its agent stopped
    function setState(s, reason) {
      f.setAttribute("data-state", s);
      main.dataset.state = s;
      stWords.textContent = W.updating(agent);
      if (s === "ready") stProgress.textContent = "";
      if (reason) failure = reason;
      const empty = blocks().length === 0;
      // nothing published yet: no send, no count, no versions in the frame, whatever the state
      if (empty) f.setAttribute("data-empty", ""); else f.removeAttribute("data-empty");
      if (empty && !placeholder.parentNode) main.append(placeholder);
      if (!empty && placeholder.parentNode) placeholder.remove();
      placeholder.setAttribute("data-state", s);
      drawIntents();
      drawMeta();
      doc.dispatchEvent(new root.CustomEvent("bise:state", { detail: { state: s } }));
    }
    function progress(text) {
      if (main.dataset.state === "ready" || !text) return;
      stProgress.textContent = ` · ${text}`;
      if (intents[intents.length - 1] !== text) intents.push(text);
      drawIntents();
    }
    if (blocks().length === 0) setState(d.state || "writing");

    if (typeof root.EventSource !== "undefined") {
      const es = new root.EventSource(`/p/${id}/events`);
      const data = (e) => { try { return JSON.parse(e.data); } catch (_) { return {}; } };
      es.addEventListener("version", (e) => { const v = data(e).n; swapping = swapping.then(() => goTo(v)).catch(() => {}); });
      es.addEventListener("state", (e) => { const x = data(e); setState(x.state || "ready", x.reason); });
      es.addEventListener("progress", (e) => progress(String(data(e).text || "")));
      const relay = (ev, name) => es.addEventListener(ev, (e) => doc.dispatchEvent(new root.CustomEvent(name, { detail: data(e) })));
      es.addEventListener("notes", (e) => doc.dispatchEvent(new root.CustomEvent("bise:notes", { detail: { notes: data(e).notes || [] } })));
      relay("voice", "bise:voice"); // slice 2 (§4.1): {phase, text}, notes.js pins it
      // the taste line (amb-home d75aefe4): {rules: N} or null after each publish and at connect
      es.addEventListener("taste", (e) => {
        let x = null;
        try { x = JSON.parse(e.data); } catch (_) { x = null; }
        if (!meta) meta = { versions: [] };
        meta.taste = x && typeof x === "object" ? x : null;
        drawMeta();
      });
      // a watching page (amb-home m_5458): the whole watch object on each change, null when it ends
      es.addEventListener("watch", (e) => {
        let x = null;
        try { x = JSON.parse(e.data); } catch (_) { x = null; }
        watch = x && typeof x === "object" ? x : null;
        wStop.disabled = false;
        drawWatch();
      });
      es.addEventListener("ticked", (e) => {
        const x = data(e);
        const b = blocks().find((y) => y.dataset.id === x.block && y.dataset.kit === "checklist");
        if (b && x.item) ticked(doc, b, String(x.item));
      });
      es.addEventListener("went", (e) => { const x = data(e); if (Array.isArray(x.went)) { went = x.went; applyWent(doc, blocks(), went); } });
      // slice C: an agent's live status {name, status, note?} redraws the items it works on
      es.addEventListener("agent", (e) => {
        const x = data(e);
        if (!x.name) return;
        agents.set(String(x.name), x);
        for (const b of blocks()) for (const li of items(b)) if (li.getAttribute("data-agent") === String(x.name)) decorateAgent(doc, li);
      });
      es.addEventListener("answered", (e) => {
        const d = data(e);
        const q = blocks().find((b) => b.dataset.id === d.block && b.dataset.kit === "question");
        if (q && d.reply != null) answered(doc, q, d.reply);
      });
    }

    // A pick answers the question's card (§4.2: POST /answer, one card, one answer). Until the
    // server has that route (404), or when it fails, the pick goes to notes.js as a pick note.
    let answerRoute = true;
    const token = (doc.querySelector('meta[name="bise-token"]') || { content: "" }).content;
    function onPick(q, n) {
      const d = pick(q, n);
      if (!d) return false;
      const asNote = () => doc.dispatchEvent(new root.CustomEvent("bise:pick", { detail: d }));
      if (d.option == null || !answerRoute) { asNote(); return true; }
      fetch(`/api/p/${id}/answer`, { method: "POST", headers: { "Content-Type": "application/json", "X-Bise-Token": token }, body: JSON.stringify({ block: d.block, option: d.option }) })
        .then((r) => {
          if (r.status === 404) answerRoute = false;
          // a 200: the hub answers the card, or with no open card saves the answer and sends the
          // agent a pick note itself (amb-core 03821a37); either way SSE 'answered' follows
          if (!r.ok) asNote();
        })
        .catch(asNote);
      return true;
    }

    // questions: click an option, or its digit when nothing is being typed
    main.addEventListener("click", (e) => {
      const act = e.target.closest && e.target.closest("[data-kit-ui] [data-act]");
      if (act) { onAct(act); return; }
      const li = e.target.closest && e.target.closest('[data-kit="question"] li');
      if (li) { const q = li.closest('[data-kit="question"]'); onPick(q, options(q).indexOf(li) + 1); return; }
      // a step's 'in the page': to the block that holds its draft, lit a moment
      const go = e.target.closest && e.target.closest("[data-goto]");
      if (go) {
        const t = targetOf(blocks(), go.getAttribute("data-goto"));
        if (t) { t.scrollIntoView({ behavior: "smooth", block: "center" }); t.setAttribute("data-lit", ""); setTimeout(() => t.removeAttribute("data-lit"), 2400); }
        return;
      }
      const sup = e.target.closest && e.target.closest("sup");
      if (sup) showSource(parseInt(sup.textContent, 10));
    });
    // an edit landed or was undone (notes.js sets data-edited): 'approve N' counts again
    doc.addEventListener("bise:edited", (e) => {
      const b = blocks().find((x) => x.dataset.id === (e.detail && e.detail.block));
      if (b) decorate([b], agent, doc);
    });
    doc.addEventListener("keydown", (e) => {
      if (e.metaKey || e.ctrlKey || e.altKey || !/^[1-9]$/.test(e.key)) return;
      const t = e.target;
      if (t && (t.isContentEditable || /^(input|textarea|select)$/i.test(t.tagName))) return;
      const q = blocks().find((b) => b.dataset.kit === "question" && !b.hasAttribute("data-answer"));
      if (q && onPick(q, parseInt(e.key, 10))) e.preventDefault();
    });
    // review and email: approve / skip (again: take it back), "approve N"; each one goes to
    // notes.js as bise:react {block, item, kind, text} (a note of kind approve or skip)
    function onAct(btn) {
      const block = btn.closest("[data-kit][data-id]");
      const kind = btn.getAttribute("data-act");
      // 'start an agent' on a feedback item: a start note to main (notes.js: bise:start), sent at
      // once; the item says 'starting…' until its agent shows up (data-agent on the next version)
      if (kind === "start") {
        const li = btn.closest("li[data-id]");
        if (!li || li.getAttribute("data-starting") != null) return;
        li.setAttribute("data-starting", "");
        doc.dispatchEvent(new root.CustomEvent("bise:start", { detail: { block: block.dataset.id, item: li.dataset.id, text: textOf(li) } }));
        decorate([block], agent, doc);
        return;
      }
      if (kind === "copy") {
        // copied: the button says so a moment, and the block keeps "copied · open #launch" (its
        // data-open link) until the server says where it went; a refused clipboard says so too
        const done = () => {
          btn.textContent = W.copied;
          setTimeout(() => { btn.textContent = W.copy; }, 1600);
          if (!wentFor(went, [block]).has(block.dataset.id)) drawWent(doc, block, { kind: "copy" });
        };
        const failed = () => { btn.textContent = W.copyFailed; setTimeout(() => { btn.textContent = W.copy; }, 3200); };
        if (root.navigator && root.navigator.clipboard) root.navigator.clipboard.writeText(toSlack(block)).then(done, failed);
        else failed();
        return;
      }
      const li = btn.closest("li[data-id]");
      const out = kind === "approve-all" ? approveAll(block) : [react(block, block.dataset.kit !== "email" && li ? li.dataset.id : null, kind)];
      for (const d of out) if (d) doc.dispatchEvent(new root.CustomEvent("bise:react", { detail: d }));
      // a reply item approved (not taken back): open its thread, the reply prefilled or copied
      const d0 = out[0];
      if (kind === "approve" && li && d0 && d0.kind === "approve" && li.dataset.reply) {
        const text = replyText(li);
        const t = replyTarget(li.dataset.reply, text);
        if (t) {
          if (t.copy && root.navigator && root.navigator.clipboard) root.navigator.clipboard.writeText(text).catch(() => {});
          root.open(t.open, "_blank", "noopener");
          replyWent(doc, li, t);
        }
      }
      decorate([block], agent, doc);
    }
    function showSource(k) {
      const src = blocks().find((b) => b.dataset.kit === "sources");
      const li = src && options(src)[k - 1];
      if (!li) return;
      li.scrollIntoView({ behavior: "smooth", block: "center" });
      li.setAttribute("data-lit", "");
      setTimeout(() => li.removeAttribute("data-lit"), 1600);
    }
  }

  // "for you": the server's list; the kit adds each page's line and follows the list live
  function startHome(main) {
    frame(W.forYou);
    const list = () => main.querySelector('[data-kit="pages"] > ul');
    function line(li) {
      li.querySelectorAll(".bise-meta, .bise-kicker").forEach((x) => x.remove());
      const s = li.dataset;
      // what the page is for: its heading's meta line (the server's data-kicker)
      if (s.kicker) li.append(el("span", { class: "bise-kicker" }, s.kicker));
      const parts = [said(s.agent), `v${s.version}`];
      if (s.state === "updating") parts.push(W.updating(s.agent).replace(" this page…", "…"));
      else if (s.at) parts.push(ago(parseInt(s.at, 10), Date.now()));
      li.append(el("span", { class: "bise-meta" }, parts.join(" · ")));
    }
    const draw = () => { const ul = list(); if (ul) Array.from(ul.children).forEach(line); };
    draw();
    setInterval(draw, 30000);
    if (typeof root.EventSource !== "undefined") {
      const es = new root.EventSource("/events");
      es.addEventListener("pages", (e) => {
        let pages = [];
        try { pages = JSON.parse(e.data).pages || []; } catch (_) { return; }
        const ul = list();
        if (!ul) return;
        ul.replaceChildren(...pages.map((p) => {
          const li = el("li", { "data-page": p.id, "data-agent": p.agent, "data-version": p.version, "data-state": p.state, "data-at": p.at_ms, "data-kicker": p.kicker || null, "data-asking": p.asking ? "" : null });
          li.append(el("a", { href: `/p/${p.id}` }, p.title || p.id));
          return li;
        }));
        draw();
      });
    }
  }

  if (doc.readyState === "loading") doc.addEventListener("DOMContentLoaded", start); else start();
})(typeof window !== "undefined" ? window : globalThis);
