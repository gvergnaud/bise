// generated from video.ts by scripts/ts_emit.ts: never hand-edited (edit the .ts, then run it)
// Preview · a video (video-view, his ask main m_32004): played by the browser's own <video> from
// the document's session url (resources/url, architect m_60354: the hub serves the file's byte
// ranges on this view's origin, so a long file seeks without being read whole). Our controls, no
// browser chrome: play, the track (a hover shows that moment's frame, drawn by a second muted
// player), the time, the speed, loop, sound, fullscreen. Keys: space plays or pauses, ← → 5 s,
// , . one frame (paused). His selection: a frame he clicked (a `time` place, keys.ts) or a box
// drawn on the frame, which pauses it (the place with its rect, in % of the frame). The screen
// takes the video's own ratio, the bar right under it (designer m_32678). His notes: a tick on
// the track at each one's moment, its box on the frame near it. Where he is (the moment, speed,
// loop, sound) is the view's state.
//
// Pure helpers (exported for plugins/media/test/video.test.mjs): stateOf, keyAction, stepped,
// seekAt, nextRate, near, screenHeight, timeWords.
import { capture } from "./dom.js";
import { clock } from "./keys.js";
/** The speeds the speed button goes through, in order. */
export const RATES = [1, 1.5, 2, 0.5];
/** ← → move this many seconds. */
export const JUMP_S = 5;
/** One frame when the file has not said its rate yet (requestVideoFrameCallback measures it). */
export const FRAME_S = 1 / 30;
/** A note's box shows on the frame this close to its moment, in seconds. */
export const NEAR_S = 0.5;
/** The player's one state, from the element's facts: `loading` (no frame yet), `playing`,
 * `paused`, `ended`, `failed`. The root says it (data-state) and the play button follows it. */
export function stateOf(m) {
    if (m.error)
        return "failed";
    if (m.readyState < 1)
        return "loading";
    if (m.ended)
        return "ended";
    return m.paused ? "paused" : "playing";
}
/** What a key does to the player, or null (his, or another layer's): space plays or pauses,
 * ← → jump JUMP_S, , . step one frame. Never with a modifier, never in a field he types in. */
export function keyAction(e) {
    if (e.metaKey || e.ctrlKey || e.altKey || e.isComposing || e.typing)
        return null;
    if (e.key === " " || e.code === "Space")
        return { do: "toggle" };
    if (e.key === "ArrowLeft")
        return { do: "jump", by: -JUMP_S };
    if (e.key === "ArrowRight")
        return { do: "jump", by: JUMP_S };
    if (e.key === ",")
        return { do: "step", by: -1 };
    if (e.key === ".")
        return { do: "step", by: 1 };
    return null;
}
/** `t` moved by `by` seconds (or frames of `frame` s), held in 0..dur. */
export function stepped(t, by, dur, frame = 0) {
    const to = t + (frame > 0 ? by * frame : by);
    const end = Number.isFinite(dur) && dur > 0 ? dur : Math.max(0, to);
    return Math.min(end, Math.max(0, Math.round(to * 1000) / 1000));
}
/** The moment under `x` px of a track `w` px wide, for a file of `dur` s. */
export function seekAt(x, w, dur) {
    if (!(w > 0) || !Number.isFinite(dur) || dur <= 0)
        return 0;
    return Math.min(dur, Math.max(0, (x / w) * dur));
}
/** The speed after `r` (RATES' order; one not in it goes back to 1). */
export function nextRate(r) {
    const i = RATES.indexOf(r);
    return i < 0 ? 1 : RATES[(i + 1) % RATES.length];
}
/** A note at `t` is shown on the frame at `now` (its box, if it has one). */
export function near(now, t) {
    return Math.abs(now - t) <= NEAR_S;
}
/** The screen's height: the video's own ratio at the view's width (the caller holds it to the
 * room there is). */
export function screenHeight(w, vw, vh) {
    return vw > 0 && vh > 0 && w > 0 ? Math.round((w * vh) / vw) : 0;
}
/** The time line: `0:12 / 1:30`. */
export function timeWords(t, dur) {
    return Number.isFinite(dur) && dur > 0 ? `${clock(t)} / ${clock(dur)}` : clock(t);
}
// ---- the DOM
const SVG = "http://www.w3.org/2000/svg";
/** The controls' glyphs, 16 px, drawn in currentColor. */
const ICON = {
    play: "M5 3.5v9l7.5-4.5z",
    pause: "M4.5 3.5h2.5v9H4.5zM9 3.5h2.5v9H9z",
    loop: "M3 7a3.5 3.5 0 0 1 3.5-3.5h5l-1.5-1.5M13 9a3.5 3.5 0 0 1-3.5 3.5h-5l1.5 1.5",
    sound: "M2.5 6h2.5l3.5-3v10l-3.5-3H2.5zM11 5.5a3.5 3.5 0 0 1 0 5",
    muted: "M2.5 6h2.5l3.5-3v10l-3.5-3H2.5zM11 6l3.5 4M14.5 6L11 10",
    full: "M2.5 6V2.5H6M10 2.5h3.5V6M13.5 10v3.5H10M6 13.5H2.5V10",
    exit: "M6 2.5V6H2.5M13.5 6H10V2.5M10 13.5V10h3.5M2.5 10H6v3.5",
};
const FILLED = new Set(["play", "pause"]);
function icon(name) {
    const svg = document.createElementNS(SVG, "svg");
    svg.setAttribute("viewBox", "0 0 16 16");
    svg.setAttribute("aria-hidden", "true");
    const p = document.createElementNS(SVG, "path");
    p.setAttribute("d", ICON[name]);
    if (FILLED.has(name))
        p.setAttribute("fill", "currentColor");
    else
        Object.entries({ fill: "none", stroke: "currentColor", "stroke-width": "1.4", "stroke-linecap": "round", "stroke-linejoin": "round" }).forEach(([k, v]) => p.setAttribute(k, v));
    svg.append(p);
    return svg;
}
/** A field he types in: its keys are his. */
const typing = (t) => !!t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName || ""));
const boxAt = (el, r) => Object.assign(el.style, { left: `${r.x}%`, top: `${r.y}%`, width: `${r.w}%`, height: `${r.h}%` });
export async function draw(root, src, ctx) {
    root.classList.add("pv-video");
    root.tabIndex = -1;
    const el = (tag, cls, text) => {
        const e = document.createElement(tag);
        if (cls)
            e.className = cls;
        if (text != null)
            e.textContent = text;
        return e;
    };
    const button = (cls, label, glyph) => {
        const b = el("button", cls);
        b.type = "button";
        b.title = label;
        b.setAttribute("aria-label", label);
        if (glyph)
            b.append(icon(glyph));
        return b;
    };
    const setIcon = (b, glyph) => b.replaceChildren(icon(glyph));
    const url = src.url;
    // the screen: the frame fit in it (the video and every box, all in % of the frame)
    const screen = el("div", "screen");
    const frame = el("div", "frame");
    const video = el("video");
    video.preload = "auto";
    video.playsInline = true;
    video.disablePictureInPicture = true;
    frame.append(video);
    screen.append(frame);
    // the bar: play, the time, the track, speed, loop, sound, fullscreen
    const bar = el("div", "bar");
    const play = button("play", "play · space", "play");
    const time = el("span", "time");
    const timeText = document.createTextNode("");
    time.append(timeText);
    const track = el("div", "track");
    track.setAttribute("role", "slider");
    track.setAttribute("aria-label", "time");
    const buffered = el("div", "buffered");
    const played = el("div", "played");
    const head = el("div", "head");
    const ticks = el("div", "ticks");
    const peek = el("div", "peek");
    const peekCanvas = el("canvas");
    const peekTime = el("span", "at");
    const peekText = document.createTextNode("");
    peekTime.append(peekText);
    peek.append(peekCanvas, peekTime);
    peek.hidden = true;
    track.append(buffered, played, ticks, head, peek);
    const rate = button("rate", "speed", null);
    const rateText = document.createTextNode("1×");
    rate.append(rateText);
    const loop = button("loop", "loop", "loop");
    loop.setAttribute("aria-pressed", "false");
    const sound = button("sound", "sound", "sound");
    const full = button("full", "full screen", "full");
    bar.append(play, time, track, rate, loop, sound, full);
    root.append(screen, bar);
    // the file, its first frame, or what failed in words
    await new Promise((ok, no) => {
        video.addEventListener("loadedmetadata", () => ok(), { once: true });
        video.addEventListener("error", () => no(Object.assign(new Error("not a video this player plays"), { code: "unreadable" })), { once: true });
        video.src = url;
    });
    const vw = video.videoWidth || 16;
    const vh = video.videoHeight || 9;
    const dur = () => (Number.isFinite(video.duration) ? video.duration : 0);
    // ---- the frame fit in the screen (up or down), centred: boxes in % of what is drawn
    const fit = () => {
        // the screen takes the video's own ratio, the bar right under it, the rest the view's
        // surface; in full screen it takes the whole screen (designer m_32678)
        const isFull = document.fullscreenElement === root;
        const room = root.clientHeight - bar.offsetHeight;
        const want = screenHeight(root.clientWidth, vw, vh);
        screen.style.height = isFull ? "" : `${Math.max(0, Math.min(room, want))}px`;
        const bw = screen.clientWidth;
        const bh = screen.clientHeight;
        if (bw <= 0 || bh <= 0)
            return;
        const k = Math.min(bw / vw, bh / vh);
        const w = Math.round(vw * k);
        const h = Math.round(vh * k);
        Object.assign(frame.style, { width: `${w}px`, height: `${h}px`, left: `${Math.round((bw - w) / 2)}px`, top: `${Math.round((bh - h) / 2)}px` });
        root.dataset.fit = "true";
    };
    new ResizeObserver(() => root.isConnected && fit()).observe(root);
    // ---- what the bar says, from the player's facts
    let frameS = FRAME_S;
    let scrubbing = null;
    const paint = () => {
        const st = stateOf(video);
        root.dataset.state = st;
        const playing = st === "playing";
        setIcon(play, playing ? "pause" : "play");
        play.title = playing ? "pause · space" : "play · space";
        play.setAttribute("aria-label", playing ? "pause" : "play");
        timeText.data = timeWords(video.currentTime, dur());
        const pct = dur() > 0 ? (video.currentTime / dur()) * 100 : 0;
        played.style.width = `${pct}%`;
        head.style.left = `${pct}%`;
        track.setAttribute("aria-valuetext", timeWords(video.currentTime, dur()));
        const b = video.buffered;
        buffered.style.width = b.length && dur() > 0 ? `${(b.end(b.length - 1) / dur()) * 100}%` : "0%";
        for (const m of frame.querySelectorAll(".pv-mark[data-t]"))
            m.classList.toggle("off", !near(video.currentTime, Number(m.dataset.t)));
    };
    for (const ev of ["play", "pause", "ended", "seeked", "timeupdate", "progress", "durationchange", "loadeddata", "ratechange"])
        video.addEventListener(ev, paint);
    // where he is, kept as the view's state: on a pause, a seek, a speed, loop or sound change
    for (const ev of ["pause", "seeked", "ratechange", "volumechange"])
        video.addEventListener(ev, () => ctx.moved());
    // a smooth head while it plays (timeupdate comes 4 times a second)
    const tick = () => {
        if (!root.isConnected) {
            for (const v of [video, peeker]) {
                v.pause();
                v.removeAttribute("src");
                v.load();
            }
            return;
        }
        if (!video.paused)
            paint();
        requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
    // the file's frame rate, measured while it plays: one frame is what a , or . steps
    if (video.requestVideoFrameCallback) {
        let last = null;
        const onFrame = (_now, meta) => {
            if (last != null && meta.presentedFrames === last.n + 1) {
                const d = meta.mediaTime - last.t;
                if (d > 0.004 && d < 0.2)
                    frameS = d;
            }
            last = { n: meta.presentedFrames, t: meta.mediaTime };
            if (root.isConnected)
                video.requestVideoFrameCallback(onFrame);
        };
        video.requestVideoFrameCallback(onFrame);
    }
    // ---- the selection (designer m_32678): only once there is something to ask about, a frame
    // he clicked (a click while it plays pauses there; paused, a click picks that frame, the
    // next plays) or a box drawn on it. Playing or moving to another moment drops it.
    let picked = false;
    let rect = null;
    let box = null;
    /** The frame he picked, paused: its moment, with the box; else null. */
    const selection = () => {
        if (!picked || !video.paused || scrubbing)
            return null;
        const t = Math.round(video.currentTime * 100) / 100;
        return rect ? { kind: "time", t, rect } : { kind: "time", t };
    };
    /** Where the kit's pill goes: by the box, or at the frame's foot. */
    const pillAt = () => {
        if (box && rect)
            return box;
        const r = frame.getBoundingClientRect();
        const y = Math.min(r.bottom, screen.getBoundingClientRect().bottom) - 40;
        return new DOMRect(r.left + 12, y, 0, 0);
    };
    const said = () => {
        const s = selection();
        ctx.select(s, null, s ? pillAt() : null);
    };
    const clearBox = () => {
        rect = null;
        box?.remove();
        box = null;
    };
    const toggle = () => {
        picked = false;
        if (video.paused || video.ended) {
            clearBox();
            video.play().catch(() => { });
        }
        else
            video.pause();
    };
    const seek = (t) => {
        if (Math.abs(t - video.currentTime) > 1e-3) {
            clearBox();
            picked = false;
        }
        video.currentTime = t;
    };
    // a click on the frame: playing, it pauses on that frame and picks it; paused, the first
    // click picks the frame it shows, the next plays
    const clickFrame = () => {
        if (!video.paused && !video.ended) {
            picked = true;
            video.pause();
        }
        else if (!picked) {
            picked = true;
            said();
        }
        else
            toggle();
    };
    video.addEventListener("play", said);
    video.addEventListener("pause", said);
    video.addEventListener("seeked", () => video.paused && !scrubbing && said());
    play.addEventListener("click", toggle);
    rate.addEventListener("click", () => {
        video.playbackRate = nextRate(video.playbackRate);
        rateText.data = `${video.playbackRate}×`;
    });
    loop.addEventListener("click", () => {
        video.loop = !video.loop;
        loop.setAttribute("aria-pressed", String(video.loop));
        ctx.moved();
    });
    sound.addEventListener("click", () => {
        video.muted = !video.muted;
        setIcon(sound, video.muted ? "muted" : "sound");
        sound.setAttribute("aria-pressed", String(video.muted));
    });
    const fullNow = () => document.fullscreenElement === root;
    full.addEventListener("click", () => (fullNow() ? document.exitFullscreen() : root.requestFullscreen()).catch(() => { }));
    document.addEventListener("fullscreenchange", () => {
        setIcon(full, fullNow() ? "exit" : "full");
        full.title = fullNow() ? "leave full screen · esc" : "full screen";
        root.classList.toggle("is-full", fullNow());
        fit();
    });
    // ---- the track: a press seeks, a drag scrubs (a playing video pauses while he scrubs)
    const x = (e) => e.clientX - track.getBoundingClientRect().left;
    track.addEventListener("pointerdown", (e) => {
        if (e.button !== 0 || e.target.closest(".tick"))
            return;
        scrubbing = { was: !video.paused };
        video.pause();
        capture(track, e.pointerId);
        seek(seekAt(x(e), track.clientWidth, dur()));
    });
    track.addEventListener("pointermove", (e) => {
        showPeek(e);
        if (scrubbing)
            seek(seekAt(x(e), track.clientWidth, dur()));
    });
    const release = () => {
        if (!scrubbing)
            return;
        const was = scrubbing.was;
        scrubbing = null;
        if (was)
            video.play().catch(() => { });
        else
            said();
    };
    track.addEventListener("pointerup", release);
    track.addEventListener("pointercancel", release);
    track.addEventListener("pointerleave", () => !scrubbing && (peek.hidden = true));
    // a note's tick: to its moment, paused
    ticks.addEventListener("click", (e) => {
        const t = e.target.closest(".tick");
        if (!t)
            return;
        video.pause();
        seek(Number(t.dataset.t));
    });
    // ---- the hover's frame: a second muted player seeks where the pointer is, drawn small
    const peeker = el("video");
    peeker.muted = true;
    peeker.preload = "metadata";
    peeker.src = url;
    const pw = 160;
    peekCanvas.width = pw * (window.devicePixelRatio || 1);
    peekCanvas.height = Math.round((pw * vh) / vw) * (window.devicePixelRatio || 1);
    peekCanvas.style.width = `${pw}px`;
    peekCanvas.style.height = `${Math.round((pw * vh) / vw)}px`;
    let peekBusy = false;
    let peekNext = null;
    const peekSeek = (t) => {
        if (peekBusy)
            return void (peekNext = t);
        peekBusy = true;
        peeker.currentTime = t;
    };
    peeker.addEventListener("seeked", () => {
        try {
            peekCanvas.getContext("2d").drawImage(peeker, 0, 0, peekCanvas.width, peekCanvas.height);
        }
        catch { }
        peekBusy = false;
        if (peekNext != null) {
            const t = peekNext;
            peekNext = null;
            peekSeek(t);
        }
    });
    function showPeek(e) {
        if (dur() <= 0)
            return;
        const w = track.clientWidth;
        const t = seekAt(x(e), w, dur());
        peekText.data = clock(t);
        const half = pw / 2;
        peek.style.left = `${Math.min(w - half, Math.max(half, x(e)))}px`;
        peek.hidden = false;
        peekSeek(t);
    }
    let start = null;
    const at = (e) => {
        const r = video.getBoundingClientRect();
        const pct = (v, size) => (size > 0 ? Math.max(0, Math.min(100, (v / size) * 100)) : 0);
        return { x: pct(e.clientX - r.left, r.width), y: pct(e.clientY - r.top, r.height) };
    };
    const pctBox = (a, b) => {
        const r1 = (v) => Math.round(v * 10) / 10;
        const x0 = Math.min(a.x, b.x), y0 = Math.min(a.y, b.y);
        return { x: r1(x0), y: r1(y0), w: r1(Math.max(a.x, b.x) - x0), h: r1(Math.max(a.y, b.y) - y0) };
    };
    const showBox = (r) => {
        box ??= el("div", "pv-box");
        boxAt(box, r);
        if (!box.isConnected)
            frame.append(box);
    };
    screen.addEventListener("pointerdown", (e) => {
        if (e.button !== 0 || !frame.contains(e.target))
            return;
        start = at(e);
        capture(screen, e.pointerId);
    });
    screen.addEventListener("selectstart", (e) => start && e.preventDefault());
    screen.addEventListener("pointermove", (e) => {
        if (!start)
            return;
        const r = pctBox(start, at(e));
        if (r.w < 1 && r.h < 1)
            return;
        if (!video.paused)
            video.pause();
        showBox(r);
    });
    const up = (e) => {
        if (!start)
            return;
        const r = pctBox(start, at(e));
        start = null;
        // a click, not a box: the frame (clickFrame)
        if (r.w < 1 || r.h < 1) {
            if (box && !rect)
                clearBox();
            clickFrame();
            return;
        }
        picked = true;
        rect = r;
        showBox(r);
        said();
    };
    screen.addEventListener("pointerup", up);
    screen.addEventListener("pointercancel", () => (start = null));
    // ---- the keys: capture, before the kit reads a typed key as his first word
    document.addEventListener("keydown", (e) => {
        if (!root.isConnected)
            return;
        const a = keyAction({ key: e.key, code: e.code, metaKey: e.metaKey, ctrlKey: e.ctrlKey, altKey: e.altKey, isComposing: e.isComposing, typing: typing(e.target) });
        if (!a)
            return;
        // space plays even on a control he clicked (loop, speed): never presses it again
        e.preventDefault();
        e.stopImmediatePropagation();
        if (a.do === "toggle")
            toggle();
        else if (a.do === "jump")
            seek(stepped(video.currentTime, a.by, dur()));
        else {
            video.pause();
            seek(stepped(video.currentTime, a.by, dur(), frameS));
        }
    }, true);
    paint();
    fit();
    // ---- his notes' marks: a tick on the track at each one's moment, its box on the frame near it
    const marks = new Map();
    const placeOf = (p) => {
        if (p.kind !== "time")
            return null;
        const d = dur();
        // a moment past this version's end: not here (detached)
        if (d > 0 && p.t > d + 0.05)
            return null;
        const t = el("button", "tick pv-mark");
        t.type = "button";
        t.dataset.t = String(p.t);
        t.title = ctx.label(p);
        t.style.left = `${d > 0 ? Math.min(100, (p.t / d) * 100) : 0}%`;
        ticks.append(t);
        if (!p.rect)
            return [t];
        const m = el("div", "pv-mark");
        m.dataset.t = String(p.t);
        boxAt(m, p.rect);
        m.classList.toggle("off", !near(video.currentTime, p.t));
        frame.append(m);
        return [t, m];
    };
    return {
        foot: [],
        place(p) {
            const k = JSON.stringify(p);
            if (!marks.has(k))
                marks.set(k, placeOf(p));
            return marks.get(k);
        },
        state: () => ({ t: video.currentTime, paused: video.paused, rate: video.playbackRate, loop: video.loop, muted: video.muted }),
        restore(s) {
            const p = s;
            if (!p || typeof p.t !== "number")
                return;
            video.currentTime = Math.min(p.t, dur() || p.t);
            video.playbackRate = typeof p.rate === "number" ? p.rate : 1;
            rateText.data = `${video.playbackRate}×`;
            video.loop = !!p.loop;
            loop.setAttribute("aria-pressed", String(video.loop));
            video.muted = !!p.muted;
            setIcon(sound, video.muted ? "muted" : "sound");
            sound.setAttribute("aria-pressed", String(video.muted));
            if (p.paused === false)
                video.play().catch(() => { });
        },
    };
}
