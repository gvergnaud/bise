// fake-opener's static opener (docs/plugin-apps.md, Fixtures), on @bise/view (kit/view): it
// reads its file through the handle, saves it back with the etag of its last read (the write
// grant), shows each line as a part his notes anchor on (`line:<n>`), and says how many notes it
// marked and how many it could not place. A click on a line selects it (the kit's pill asks).
import { connect } from "./~kit/view/view.js";

const $ = (id) => document.getElementById(id);
const say = (t) => { $("state").textContent = t; };

const view = await connect({ name: "fake-opener fake", version: "0.2.0", typeToAsk: true });
const doc = view.document();
let etag = null;

function lines(text) {
  const ol = $("lines");
  ol.replaceChildren(...text.split("\n").map((line, i) => {
    const li = document.createElement("li");
    li.dataset.anchor = `line:${i + 1}`;
    li.textContent = line;
    li.addEventListener("click", () => view.onSelection(li.dataset.anchor, line, { at: li }));
    return li;
  }));
  layer.redraw();
}

const layer = view.notes.mount({ place: (anchor) => $("lines").querySelector(`li[data-anchor="${CSS.escape(anchor)}"]`) });
view.notes.on((notes) => {
  const placed = notes.filter((n) => $("lines").querySelector(`li[data-anchor="${CSS.escape(n.anchor)}"]`)).length;
  $("notes").textContent = `${notes.length} notes · ${notes.length - placed} detached`;
});

if (!doc) say("no file");
else {
  $("name").textContent = doc.name;
  try {
    const r = await view.read();
    etag = r.etag;
    $("text").value = r.text ?? "";
    lines($("text").value);
    $("save").disabled = !r.writable;
    say(r.writable ? "saved" : "read only");
  } catch (err) {
    say("error " + (err && err.code));
  }
}

$("save").onclick = async () => {
  say("saving…");
  try {
    const r = await view.save($("text").value, { ifMatch: etag });
    if (r.outcome === "saved") etag = r.etag;
    lines($("text").value);
    say(r.outcome);
  } catch (err) {
    say("error " + (err && err.code));
  }
};
