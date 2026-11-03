// The pure parts, without a browser: node --test computer-use/extension/test/
import { test } from "node:test";
import assert from "node:assert/strict";
import { walk, render, pick, match, diff, parseRe, ariaRole } from "../lib/ax.js";
import { parseKeys, chordEvents, keyLabel } from "../lib/keys.js";
import { summary, failure, withPlace, hostOf, label, refusal } from "../lib/text.js";
import { jpegSize } from "../lib/jpeg.js";
import { remember, nextChooser, sawPicker, filesOf, namesLine, uploadSummary, uploadCandidates, KEEP } from "../lib/upload.js";
import type { Chooser } from "../lib/upload.js";

// A small CDP AX tree: root → heading, button (with its own text), a
// password textbox, a generic wrapper, a paragraph of text, an ignored node.
let id = 0;
/** A CDP AX node as Accessibility.getFullAXTree gives it (the fields walk reads). */
type AXNode = {
  nodeId: string;
  parentId?: string;
  role: { value: string };
  name?: { value: string };
  value?: { value: string };
  properties?: { name: string; value: { value: unknown } }[];
  ignored?: boolean;
  backendDOMNodeId: number;
  childIds: string[];
};
type Tree = AXNode & { _kids: Tree[] };
const N = (role: string, name?: string, extra: Partial<AXNode> = {}, kids: Tree[] = []): Tree => ({ nodeId: String(++id), role: { value: role }, name: name === undefined ? undefined : { value: name }, backendDOMNodeId: 100 + id, childIds: kids.map((k) => k.nodeId), _kids: kids, ...extra });
function flat(root: Tree) {
  const out: AXNode[] = [];
  const go = (n: Tree, parent: Tree | null) => { const { _kids, ...node } = n; if (parent) node.parentId = parent.nodeId; out.push(node); _kids.forEach((k) => go(k, n)); };
  go(root, null);
  return out;
}
const tree = () => {
  id = 0;
  const btnText = N("StaticText", "Add to cart", {}, [N("InlineTextBox", "Add to cart")]);
  return flat(N("RootWebArea", "Shop", {}, [
    N("heading", "Shop"),
    N("generic", "", {}, [
      N("button", "Add to cart", {}, [btnText]),
      N("button", "Pay", { properties: [{ name: "disabled", value: { value: true } }] }),
    ]),
    N("textbox", "Password", { value: { value: "hunter2" }, properties: [{ name: "focused", value: { value: true } }] }),
    N("checkbox", "Remember me", { properties: [{ name: "checked", value: { value: "true" } }] }),
    N("paragraph", "", {}, [N("StaticText", "Price: 12,50 €")]),
    N("button", "Hidden", { ignored: true }),
    N("link", "Next \"page\""),
  ]));
};

const entriesOf = (nodes: AXNode[], passwords = new Set<number>()) => {
  let n = 0;
  const refs = new Map<number, number>();
  return walk(nodes, { refFor: (b: number) => refs.get(b) || (refs.set(b, ++n), n), passwords });
};
/** A thrown error with code bad_args (the lib's refusals). */
const badArgs = (e: unknown) => e instanceof Error && "code" in e && e.code === "bad_args";
/** One dispatchKeyEvent's params, the fields these tests read. */
type KeyEvent = { type: string; key: string; modifiers: number; text?: string; commands?: string[] };
const events = (chord: ReturnType<typeof parseKeys>[number]): KeyEvent[] => chordEvents(chord);

test("walk + render: the C2 text", () => {
  const nodes = tree();
  const pwNode = nodes.find((n) => n.name?.value === "Password");
  assert.ok(pwNode, "the tree has a Password box");
  const pw = pwNode.backendDOMNodeId;
  const r = render(entriesOf(nodes, new Set([pw])), { title: "Shop", host: "shop.test" });
  assert.equal(r.text, [
    "# Shop · shop.test",
    '- heading "Shop" [e1]',
    '- button "Add to cart" [e2]',
    '- button "Pay" [e3] (disabled)',
    '- textbox "Password" [e4] value="•••" (focused)',
    '- checkbox "Remember me" [e5] (checked)',
    '- text "Price: 12,50 €" [e6]',
    '- link "Next \\"page\\"" [e7]',
  ].join("\n"));
  assert.equal(r.refs, 7);
  assert.equal(r.truncated, false);
  assert.ok(!r.text.includes("hunter2"));
});

test("a paragraph split in one span per character is one text line (launch #3)", () => {
  id = 0;
  const chars = [..."This is"].map((c) => N("generic", "", {}, [N("StaticText", c, {}, [N("InlineTextBox", c)])]));
  const nodes = flat(N("RootWebArea", "Example", {}, [N("paragraph", "", {}, chars), N("link", "Learn more")]));
  const lines = entriesOf(nodes).map((e) => e.line);
  assert.deepEqual(lines.map((l) => l!.replace(/ \[e\d+\]/, "")), ['- paragraph', '- text "This is"', '- link "Learn more"']);
});

test("render cuts at max_nodes and says so", () => {
  const r = render(entriesOf(tree()), { title: "Shop", host: "x", maxNodes: 2 });
  assert.equal(r.truncated, true);
  assert.equal(r.refs, 2);
  assert.match(r.text.split("\n").at(-1)!, /^# … 5 more nodes/);
});

test("nesting indents 2 spaces per depth; long values cut at 80", () => {
  id = 0;
  const nodes = flat(N("RootWebArea", "", {}, [N("list", "", {}, [N("listitem", "", {}, [N("textbox", "Note", { value: { value: "x".repeat(200) } })])])]));
  const lines = render(entriesOf(nodes)).lines;
  assert.equal(lines[2].indexOf("- textbox"), 4);
  assert.match(lines[2], /value="x{79}…"/);
});

test("ariaRole maps Chrome's internal roles", () => {
  assert.equal(ariaRole("StaticText"), "text");
  assert.equal(ariaRole("image"), "img");
  assert.equal(ariaRole("LabelText"), "label");
  assert.equal(ariaRole("button"), "button");
});

test("locators: role, name (substring, exact, regex), text, label, nth", () => {
  const e = entriesOf(tree());
  assert.equal(pick(e, { role: "button", name: "add to" }).entry!.name, "Add to cart");
  assert.equal(pick(e, { role: "button", name: "Add", exact: true }).code, "not_found");
  assert.equal(pick(e, { name_re: "^pay$/i" }).entry!.name, "Pay");
  assert.equal(pick(e, { text: "12,50" }).entry!.role, "text");
  assert.equal(pick(e, { text_re: "Price: \\d+" }).entry!.name, "Price: 12,50 €");
  assert.equal(pick(e, { label: "Password" }).entry!.role, "textbox");
  assert.equal(pick(e, { text: "Password" }).code, "not_found", "text never matches a form field");
  const amb = pick(e, { role: "button" });
  assert.equal(amb.code, "ambiguous");
  assert.equal(amb.candidates!.length, 2);
  assert.equal(pick(e, { role: "button", nth: 1 }).entry!.name, "Pay");
  assert.equal(pick(e, { role: "button", nth: -1 }).entry!.name, "Pay");
  assert.equal(pick(e, { role: "button", nth: 5 }).code, "not_found");
  const nf = pick(e, { role: "button", name: "Checkout" });
  assert.deepEqual(nf.candidates, ['- button "Add to cart" [e2]', '- button "Pay" [e3] (disabled)']);
});

test("text locators keep the innermost match", () => {
  id = 0;
  const nodes = flat(N("RootWebArea", "", {}, [N("listitem", "Price list", {}, [N("StaticText", "Price: 3 €")])]));
  const m = match(entriesOf(nodes), { text: "price" });
  assert.equal(m.length, 1);
  assert.equal(m[0].role, "text");
});

test("parseRe: flags after the last slash; bad source is bad_args", () => {
  assert.ok(parseRe("Anker.*2 m/i").test("anker usb 2 M"));
  assert.ok(parseRe("a/b").test("a/b"));
  assert.throws(() => parseRe("(/"), badArgs);
});

test("diff: + added, - removed, at most 20 lines", () => {
  assert.equal(diff(["- a", "- b"], ["- a", "- b"]), "");
  assert.equal(diff(['- text "x" [e1]', "- button [e2]"], ['- text "y" [e3]', "- button [e2]"]), '- text "x" [e1]\n+ text "y" [e3]');
  const many = diff([], Array.from({ length: 30 }, (_, i) => `- t${i}`));
  const lines = many.split("\n");
  assert.equal(lines.length, 20);
  assert.equal(lines.at(-1), "… 11 more changes");
});

test("keys: Playwright names, chords, macOS edit commands", () => {
  const [enter] = parseKeys("Enter");
  assert.deepEqual(events(enter).map((e) => [e.type, e.key, e.text]), [["keyDown", "\r", "\r"], ["keyUp", "\r", undefined]]);
  const chords = parseKeys("Meta+A Backspace");
  assert.equal(chords.length, 2);
  const ev = events(chords[0]);
  assert.equal(ev[0].key, "Meta");
  assert.deepEqual(ev[1].commands, ["selectAll"]);
  assert.equal(ev[1].modifiers, 4);
  assert.equal(events(parseKeys("Shift+a")[0])[1].text, "A");
  assert.throws(() => parseKeys("Hyper+Q"), badArgs);
  assert.throws(() => parseKeys("Florp"), badArgs);
  assert.equal(keyLabel("Escape Meta+A Enter"), "esc cmd+a enter");
});

test("summary lines (designer m_3616)", () => {
  const h = "amazon.fr";
  assert.equal(summary("click", { role: "button", name: "Add to cart" }, {}, h), 'clicked "Add to cart" · amazon.fr');
  assert.equal(summary("fill", { role: "searchbox", name: "" }, {}, h), "typed in a search box · amazon.fr");
  assert.equal(summary("type", { role: "textbox", name: "" }, {}, h), "typed in a text box · amazon.fr");
  assert.equal(summary("press", null, {}, h, keyLabel("Enter")), "pressed enter · amazon.fr");
  assert.equal(summary("select", { role: "combobox", name: "Size" }, { value: "Large" }, h), 'picked "Large" in "Size" · amazon.fr');
  assert.equal(summary("check", { role: "checkbox", name: "Remember me" }, { value: false }, h), 'unchecked "Remember me" · amazon.fr');
  assert.equal(summary("scroll", null, {}, h), "scrolled down · amazon.fr");
  assert.equal(summary("scroll", { role: "heading", name: "Reviews" }, {}, h), 'scrolled to "Reviews" · amazon.fr');
  assert.equal(summary("goto", null, {}, h), "went to amazon.fr");
  assert.equal(summary("open", null, {}, h), "opened a tab · amazon.fr");
  assert.equal(summary("close", null, {}, h), "closed the tab · amazon.fr");
  assert.equal(label("a\nb"), "a b");
  assert.equal(label("x".repeat(40)), "x".repeat(31) + "…");
  assert.equal(failure("click", '"Add to cart"', "timeout", "something covers it"), 'couldn\'t click "Add to cart": something covers it');
  assert.equal(failure("fill", '"Email"', "not_found"), 'couldn\'t find "Email"');
  assert.equal(withPlace('couldn\'t click "Add to cart": something covers it', "amazon.fr"), 'couldn\'t click "Add to cart": something covers it · amazon.fr');
  assert.equal(withPlace("couldn't go to amazon.fr: it took too long", "amazon.fr"), "couldn't go to amazon.fr: it took too long");
  assert.equal(withPlace("couldn't find \"Email\"", ""), "couldn't find \"Email\"");
});

test("hosts and refusals", () => {
  assert.equal(hostOf("https://www.amazon.fr/dp/1"), "amazon.fr");
  assert.equal(hostOf("http://127.0.0.1:8080/x"), "127.0.0.1:8080");
  for (const u of ["chrome://settings", "chrome-extension://abc/x.html", "edge://flags", "https://chromewebstore.google.com/detail/x", "https://chrome.google.com/webstore/x", "https://microsoftedge.microsoft.com/addons/x", "https://accounts.google.com/v3/signin/challenge/pwd", "about:config"]) {
    assert.ok(refusal(u), u);
  }
  for (const u of ["https://amazon.fr", "about:blank", "https://accounts.google.com/", "https://chrome.google.com/", "http://localhost:3000"]) {
    assert.equal(refusal(u), null, u);
  }
});

test("jpegSize reads the SOF marker", () => {
  // SOI, APP0 (len 4), SOF0 with height 0x0102 and width 0x0304.
  const bytes = [0xff, 0xd8, 0xff, 0xe0, 0x00, 0x04, 0x00, 0x00, 0xff, 0xc0, 0x00, 0x11, 0x08, 0x01, 0x02, 0x03, 0x04, 0x03, 0, 0, 0, 0, 0, 0, 0, 0];
  assert.deepEqual(jpegSize(Buffer.from(bytes).toString("base64")), { width: 0x0304, height: 0x0102 });
  assert.deepEqual(jpegSize(Buffer.from("not a jpeg").toString("base64")), { width: 0, height: 0 });
});

test("uploads: the choosers a tab opened, the files, the summary, the candidates", () => {
  let list: Chooser[] = [];
  for (let i = 1; i <= KEEP + 2; i++) list = remember(list, { id: i, kind: i === 3 ? "picker" : "input", backendNodeId: i, multiple: false });
  assert.equal(list.length, KEEP);
  assert.equal(list[0].id, 3);
  // after 2: 3 is a picker bise can't feed, 4 is next
  assert.equal(nextChooser(list, 2)?.id, 4);
  assert.ok(sawPicker(list, 2));
  list[1].used = true;
  assert.equal(nextChooser(list, 2)?.id, 5);
  assert.equal(nextChooser(list, KEEP + 2), null);
  assert.ok(!sawPicker(list, 3));
  assert.deepEqual(filesOf("/a/b.pdf"), ["/a/b.pdf"]);
  assert.deepEqual(filesOf(["/a", "/b"]), ["/a", "/b"]);
  for (const bad of [[], ["rel.pdf"], [1], null, {}]) assert.equal(typeof filesOf(bad), "string", JSON.stringify(bad));
  assert.equal(namesLine(["a.pdf", "b.pdf", "c.pdf"]), "a.pdf, b.pdf, c.pdf");
  assert.equal(namesLine(["a", "b", "c", "d", "e"]), "a, b and 3 more");
  assert.equal(uploadSummary(["01.pdf"], '"Upload payslip"', "cafpi.fr"), 'attached 01.pdf to "Upload payslip" · cafpi.fr');
  assert.equal(summary("set_input_files", { role: "button", name: "Upload payslip" }, { files: ["/x/01.pdf", "/x/02.pdf"] }, "cafpi.fr"), 'attached 01.pdf, 02.pdf to "Upload payslip" · cafpi.fr');
  assert.equal(summary("set_input_files", null, { files: "/x/01.pdf" }, "cafpi.fr"), "attached 01.pdf · cafpi.fr");
  assert.equal(failure("set_input_files", '"Next"', "no_file_input", "it takes no file"), 'couldn\'t attach files to "Next": it takes no file');
  const c = uploadCandidates([{ name: "Identity card", hidden: false }, { name: "payslip", hidden: true }], [
    { role: "button", name: "Upload payslip", line: '- button "Upload payslip" [e4]' },
    { role: "button", name: "Next", line: '- button "Next" [e5]' },
    { role: "link", name: "Joindre un justificatif", line: '- link "Joindre un justificatif" [e6]' },
  ]);
  assert.deepEqual(c.map((l) => l.slice(0, 30)), ['- file input "Identity card"', '- file input "payslip" (hidden', '- button "Upload payslip" [e4]', '- link "Joindre un justificati']);
});
