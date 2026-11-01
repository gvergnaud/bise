---
name: draw
description: Draw hand-drawn diagrams for the user on a canvas he can edit by hand (boxes, ellipses, diamonds, sticky notes, arrows, groups, free text), then read his edits back and export to SVG or PNG. Use when a sketch says it better than words - an architecture, a flow, a sequence of steps, a plan, a mind map, a before/after - or when the user asks to draw, sketch, whiteboard or diagram something.
---

# Draw

A drawing is a list of elements with ids. You give nodes and edges; the
server lays them out, sizes each shape to its text and binds the arrows
to their shapes (they follow when a shape moves). The user sees an
editable canvas under your tool call and can open it in the panel.

## Draw

```ts
await tools.draw.draw({
  title: "Signup flow",
  direction: "right",            // right (default) | down | grid | row | column
  nodes: [
    { id: "form", text: "Signup form" },
    { id: "api", text: "API\nPOST /users", color: "blue", fill: "blue" },
    { id: "ok", text: "Valid?", shape: "diamond", color: "amber" },
    { id: "db", text: "Postgres", shape: "ellipse", group: "backend" },
    { id: "mail", text: "Welcome mail", group: "backend" },
    { id: "todo", text: "rate limit?", shape: "note" },
  ],
  edges: [
    { from: "form", to: "api", label: "submit" },
    { from: "api", to: "ok" },
    { from: "ok", to: "db", label: "yes" },
    { from: "ok", to: "form", label: "no", style: "dashed" },
    { from: "db", to: "mail" },
  ],
  groups: [{ id: "backend", label: "backend" }],   // or a node's `group`
});
```

- shapes: `box` (default), `ellipse`, `circle`, `diamond`, `note` (a sticky
  note), `text` (words with no shape).
- colors: `ink` (default), `blue`, `amber`, `green`, `violet`, `teal`,
  `rust`, `olive`, `gray`, `red` (a `#hex` works too, for a brand color). `color` is the stroke and text, `fill`
  the inside (`fillStyle`: `hachure` default, `cross`, `solid`).
- edges: `label`, `style` (`solid`, `dashed`, `dotted`), `head` (`end`
  default, `both`, `start`, `none`).
- give `x`/`y` (and `w`/`h`) only to pin a node; the rest is laid out.
- `\n` breaks a line. Keep texts short: 1 to 4 words a line reads best.
- `elements` takes raw ones for what nodes can't say: a free line or arrow
  (`{type: "arrow", points: [[0,0],[120,40]]}`), a freehand stroke
  (`type: "draw"`), a text at a place.

Every answer is the drawing as text, one line per element with its id:
use those ids in later calls, never guess them.

## Change it

- `add_shapes({id, nodes, edges, groups})`: edges may point at elements
  already there; new nodes are placed next to what they connect to.
- `update_shapes({id, changes: [{id: "api", text: "API v2", color: "green"}]})`:
  text, label, x, y, dx, dy, w, h, color, fill, fillStyle, style, shape,
  fontSize, from, to, members. A shape grows to fit new text.
- `remove_shapes({id, ids: ["mail"]})`: its arrows go too.
- `arrange({id, direction: "down"})`: lay everything out again (it moves
  what he placed by hand too: ask before).
- `id` is optional everywhere: the last drawing changed.

The canvas he sees updates by itself within a few seconds: no need to
show it again. `show_drawing({id})` shows it again later in the thread.

## His edits

He can move, resize, draw, write, recolor, group and delete by hand.
`read_drawing({id})` starts with what he changed since you last looked
("box api: moved to (240,4)", "added text t-u1 'rate limit?'", ...),
then the whole drawing. Any tool you call on that drawing says his
pending changes first, so none is lost. When he presses "Send to agent"
you get a message listing them; his edits also reach your context with
his next message. Treat them as his intent: a shape he moved stays where
he put it, a text he added is a note to you.

## Export

`export_drawing({id, format: "svg"})` writes an SVG (light by default,
`theme: "dark"`; `to: "/abs/path.svg"` to put it somewhere). `png` gives
the view's last render (made while the drawing is shown): if it says
there is none yet, call `show_drawing`, then export again.
`list_drawings()` names them all.
