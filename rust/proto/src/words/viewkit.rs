//! @bise/view's UI words (kit/view/, docs/plugin-apps.md A5): what the
//! library itself draws inside a plugin's view (the pill by a selection),
//! and the places a first-party view names in a note's `label` (architect
//! m_60509 (3): from this catalog, never literals in each plugin, so
//! designer signs them once). Generated into `kit/view/words.gen.ts` (and
//! its .js) by rust/proto/tests/ts.rs. The anchor labels' words
//! (bise_proto::anchor::label) stay the same as these: `page 2 · an area`.
//! A view reads them as `viewkit.<name>` (tests/words_read_check.py).

crate::words! { viewkit:
    // ---- the pill by a selection (kit/view/pill.ts; select.js's in the viewer)
    /// the ink pill under what he selected in a view: typing or a click asks about it in his message
    ask = "type to ask about it";
    /// the pill, a moment after he asked (then it goes)
    added = "added to your message";
    // ---- a document view's own state (Preview, Sites: one pair, apps-sites m_62274)
    /// a view before its document is drawn
    opening = "opening…";
    /// a view whose document failed to load (its reason follows on its own line)
    didnt_open = "this didn't open";
    // ---- places a view names (a note's label: the chip reads `page 2 · "net revenue…"`): each
    // view adds its own here in the land that reads them (architect m_61995 (3)). Designer
    // signed these (m_61894): `an area` (never 'a box': he drew a region), `page {n}`,
    // `page {n} · an area`, `slide {n}`, `slide {n} · an area`, `{t} · an area` (t as a
    // player says it), `line {n}`, `lines {from}-{to}`, `cell {r}`, `{sheet} · {range}` (one
    // sheet: the range alone); lowercase but proper names and cell refs.
    /// a place: the slide a selection is on (plugins/slides' view, a clicked shape's label)
    slide(n) = "slide {n}";
    // Preview (plugins/media): a box drawn on an image, a pdf's page, a video's moment
    /// a box he drew on an image
    area = "an area";
    /// a passage of a pdf, on its page (its words follow in the chip)
    page(n) = "page {n}";
    /// a box he drew on a pdf's page
    page_area(n) = "page {n} · an area";
    /// a box he drew on a video's frame at that moment (t as the player says it, `0:12`)
    moment_area(t) = "{t} · an area";
}
