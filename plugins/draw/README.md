# draw

A hand-drawn canvas the agent and the user share, as an MCP Apps view.
The agent draws (boxes, ellipses, diamonds, sticky notes, arrows, groups,
laid out for it); the user edits by hand (select, move, resize, draw,
write, recolor, group, undo); the agent reads his edits back; SVG and PNG
export. Off by default: turn it on from the plugins page.

## Ask

- "sketch how the hub, the agents and the TUI talk to each other"
- "draw the signup flow as boxes and arrows, I'll fix it by hand"
- "look at what I changed in the drawing and update the plan"
- "export the drawing as an SVG into docs/"

## Parts

| part | what |
| --- | --- |
| `server.mjs` | the MCP server (Node 18+, stdlib only), the tools below, drawings in `$PLUGIN_DATA/drawings/<id>.json` |
| `lib/core.js` | the scene model, the layout, the hand-drawn renderer and the words of `read_drawing`: one file, loaded by the server and inlined into the view |
| `views/canvas.html` | the canvas (`ui://draw/canvas`), inline under the tool row or in the panel |
| `skills/draw/SKILL.md` | the agent's guide (the SDK: nodes, edges, groups) |
| `test/draw.test.mjs` | node:test units, run by `tests/gate.sh quick` |

Model tools: `draw`, `show_drawing` (both show the view), `add_shapes`,
`update_shapes`, `remove_shapes`, `arrange`, `read_drawing`,
`export_drawing`, `list_drawings`. App-only tools (the view's, never shown
to the model): `get_drawing` (polled every 4 s), `save_edits`,
`save_snapshot` (the PNG `export_drawing` gives).

## How edits flow

The view keeps the server's last scene and his local one; it sends the
difference (`save_edits`: upserts, deletes, order) 0.7 s after he stops,
and replays any edit made meanwhile on the answer. An agent change seen
by the poll is merged the same way. The server keeps what the agent last
saw; `read_drawing` and every model tool say the difference first. The
view also sends his changes as model context and, on "Send to agent", as
a message.

## Keys

V select, H pan (or hold space), R box, O ellipse, D diamond, A arrow,
L line, P pencil, T text; Enter or double-click writes in a shape;
⌘Z / ⇧⌘Z, ⌘D duplicate, ⌘G group, ⇧⌘G ungroup, ⌫ delete, arrows nudge
(shift: 10), ⌘0 fit, ⌘-wheel or pinch zooms.

## Credits

Our own code. The sketchy stroke (two jittered passes, hachure fills) is
the look [rough.js](https://roughjs.com) (MIT) made known; no rough.js
code is copied. The colors are the gallery kit's series
(`plugins/.view-kit/kit.css`).
