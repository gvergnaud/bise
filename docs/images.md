# Images: screenshots in the composer, image content for the model

Status: implemented (task `screenshots`): crate `rust/images`
(`bend-images`), `rust/tui/src/attach.rs`, `rust/jsrt/src/main.rs`,
`core/image.bend`, `core/api.bend`, `runtime/provider.bend`,
`tool-desc-run-typescript.txt`. Tested live on Claude (Anthropic) and
mistral-medium (OpenAI style), single agent and Switchboard task.

## Goal

- The user attaches images in the composer: `@shot.png` in the `@`
  popup, a file dragged from Finder into the terminal (the terminal
  pastes its path), a clipboard image (Ctrl+V).
- The model receives them as image content blocks (Anthropic and the
  OpenAI-style Mistral API), in a single-agent session and in a
  Switchboard task.
- A `run_typescript` program can return images: they reach the model as
  image blocks inside the tool result.

## What Codex does (`~/lab/codex/codex-rs`, pulled 2026-09-28)

- Composer (`tui/src/bottom_pane/chat_composer.rs`): an attached image is
  an atomic placeholder `[Image #N]` in the text; the path lives beside
  the text (`attachments`). On submit, only the attachments whose
  placeholder is still in the text are sent; placeholders renumber.
- Paths: `clipboard_paste.rs::normalize_pasted_path` turns a paste into a
  path (`file://` URL, quoted, shell-escaped `a\ b.png`, Windows/WSL). A
  paste that is one path to a decodable image is attached
  (`handle_paste_image_path`), else it is text.
- `@` popup: a picked file with an image extension (png jpg jpeg gif
  webp) that decodes is attached, not inserted as a path.
- Clipboard: Ctrl+V / Alt+V read an image with `arboard` (file list
  first, else raw pixels), encode PNG into a temp file, attach it.
- Encoding (`utils/image`): resize to fit 2048x2048, re-encode, data URL.
  In the request (`protocol/src/models.rs`) each image is three items:
  text `<image name=[Image #1] path="...">`, the image, text `</image>`.

## What Vibe does (`~/mistral/dashboard/vibe_sdk/harness`, the Unified Harness)

- Content is MCP `ContentBlock`s everywhere (`core/src/core/wire/content.rs`):
  `{type:"text",text}`, `{type:"image",data,mimeType}`, audio, resources.
  User images are `ImageAttachment{source: file|inline, alias, mimeType}`;
  a file source is read and base64-encoded when the request is built
  (`_session.py`, 10 MiB cap per image).
- `run_typescript` (`features/programmatic_tool_calling/execution.rs`):
  when the program returns a NON-EMPTY ARRAY that decodes as content
  blocks, those blocks REPLACE the tool result (`returned_content_blocks`,
  camelCase MCP fields: `[{type:'image', data, mimeType:'image/png'}]`).
  Otherwise the result is the JSON text, plus the non-text blocks that the
  nested tool calls returned ("Additional content from `tools.x.y` (call
  N):" then the blocks).
- Provider serialization of tool results with images:
  - Mistral/OpenAI chat (`adapters/mistral.py::_content`): a tool message
    with an image gets a parts array `[{type:text}, {type:image_url,
    image_url:{url:"data:<mime>;base64,<data>"}}]`; text-only stays a
    string. User messages: same parts.
  - Anthropic: user `{type:image, source:{type:base64, media_type,
    data}}`; tool results carry the same blocks inside `tool_result`.

## Our design

Everything between the TUI and the provider is text (composer -> REPL
socket or hub -> journal -> task REPL -> session history -> wire
request). So an image travels as a **marker in the text**, and becomes a
content block only at the last step, the provider request.

### The marker (wire format)

```
<image name="[Image #1]" path="shots/login.png" mime="image/png" b64="/Users/me/.bend-harness/images/3f9c…e1.b64">
```

- `name`: the label the user sees in the composer.
- `path`: where the image came from (shown to the model, Codex style).
- `mime`: `image/png`, `image/jpeg`, `image/gif` or `image/webp`.
- `b64`: the image, base64, in the **image store**
  (`$BEND_IMAGE_DIR`, else `~/.bend-harness/images/`), named by a hash
  of the bytes. The store also keeps the decoded image beside it
  (`<hash>.<ext>`). The attributes are always in this order; no value
  holds `"`, `>` or a newline (the writer refuses such a path).
- One line, plain ASCII quotes: it survives the socket protocol, the
  hub JSON, the journal, the session file, compaction and `--resume`.
  A copy in the store means a deleted or edited original (a macOS
  screenshot thumbnail lives in a temp folder) never breaks the history.

### Writers of markers

- **TUI** (`rust/tui/src/attach.rs`, shared code in the `bend-images`
  crate `rust/images`): the composer holds `[Image #N]` and the app keeps
  the attachments. Enter/Tab expands each placeholder still in the text
  into its marker; the removed ones are dropped. Sources:
  - `@` popup: a picked file with an image extension is attached.
  - Paste (drag-and-drop): a paste that is ONE path (quotes, `\ `
    escapes and `file://` accepted) to an existing image file is
    attached; any other paste stays text.
  - Ctrl+V: the clipboard image (macOS `osascript` «class PNGf»; Linux
    `wl-paste`/`xclip`), stored as PNG.
- **bend-jsrt** (`run_typescript`): a program that returns a non-empty
  array of content blocks gets the Vibe behavior: text blocks become
  text, image blocks are stored and become markers. Blocks:
  `{type:'image', data, mimeType}` (MCP) and, our extension,
  `{type:'image', path:'/abs/shot.png'}` (a screenshot taken by `bash`
  would not fit through the bash output cap as base64).

### Preparation and limits (`bend-images`)

- Kind by magic bytes (PNG, JPEG, GIF, WebP), not by extension; size
  from the header.
- Larger than 2048 px on a side, or more than 3.75 MB: downscaled with
  macOS `sips` to fit 2048 px (JPEG if still too big). Without `sips`,
  such an image is refused with a message. Anthropic takes 5 MB of
  base64 per image, 8000 px; Mistral 10 MB.
- At most 20 images per request (the oldest ones become text
  `[image omitted: <path>]`).
- And within a byte limit (task `big-request`, 2026-10-04: ambient's 20
  most recent screenshots were 32.9 MB of base64, so every turn got
  Anthropic's 400 "Request content length exceeded 32 MB limit" through
  the foundry proxy, and the agent never took a turn again): when the
  body without its images plus the kept ones would pass
  `$BISE_REQUEST_MAX_BYTES` (default 24 MiB), the count cap halves (20,
  10, 5, 2, 1, 0; stepped like the count cap, for the prompt cache) and
  the oldest become `[image unavailable: <name> (removed to keep the
  request under N MB)]` (`Im.res_fit`). A provider that still refuses the
  size (a 413, or a 400 that says so: `W.size_refused`) gets the request
  again once, at once, with half its weight (`model_call.sized`, a
  `provider_retry` line); a second refusal for size makes the Core compact
  instead of failing the turn, unless the history holds no assistant
  message (just compacted). Test: `tests/request_size_e2e.py` (the fake
  provider refuses a body over `$FAKE_MAX_BODY` with that 400).

### The REPL: marker -> content blocks (`core/api.bend`, `runtime/provider.bend`)

- Pure (`core/api.bend`, pinned by laws): a user or tool message whose
  content holds a marker is split into parts. Each marker becomes three
  parts, the Codex shape: text `<image name=[Image #1] path="...">`, the
  image, text `</image>`. The image data is a **placeholder**
  `@@BENDIMG:<b64 path>@@` (the JSON never carries megabytes through
  the pure JSON printer).
  - OpenAI style: `content: [{type:text,…}, {type:image_url,
    image_url:{url:"data:<mime>;base64,@@BENDIMG:…@@"}}]` for user and
    tool messages (Mistral accepts parts on tool messages).
  - Anthropic: user `{type:image, source:{type:base64, media_type,
    data:"@@BENDIMG:…@@"}}`; a `tool_result` gets a blocks array.
  - A message without marker is byte-identical to before.
- IO (`runtime/provider.bend`, `img_req`), once per model call, before
  the body is built: the `.b64` file of each marker is measured (its
  size, never its data), only inside
  the image store (`$BEND_IMAGE_DIR` or `~/.bend-harness/images/`, name
  ending in `.b64`, no `/../`). A marker whose file is not there, or
  beyond the 20 most recent images, is turned into plain text
  (`<image-unavailable name=…`, `Im.img_defang`): so a file that merely
  CONTAINS a marker (a doc, a log read with bash) never breaks a request.
  After the body is printed, `Im.img_splice` puts a **file part** at each
  placeholder (`Http.file_part(path, size)`: NUL `bend-file:<size>:<path>`
  NUL). The HTTP client (`vendor/http/http.bend`) counts the file's size in
  Content-Length and sends its bytes from a C buffer at write time
  (`Wire.send.file`, `Wire.tls.send.file`): the base64 never enters the
  Bend heap. Before (2026-10-01): the data was read into Bend Strings
  (~47 bytes of heap per char) and spliced in, so 20 screenshots of 545 KB
  made a REPL of 802 MB, and the Bend heap never shrinks (an agent's
  repl-live sat at 1.8 GB). Now 120 MB, and the turn 0.5 s instead of 1.3 s
  (`tests/images_mem_e2e.py`). A request without marker costs one scan.
- Since 2026-10-02 (task `image-tag`; three agents stuck on Anthropic's
  400 "tool_result.content.2.image.source.base64: invalid base64 data"),
  the wire-level `img_req` above is gone. The cause: an agent printed the
  user's marker in bash output, wrapped by a note so that its path held a
  newline; the wire scan saw the escaped newline and skipped it, the
  message parse saw a real newline and made an image part, whose data went
  out as the `@@BENDIMG:…@@` placeholder itself. Now:
  - scope (`core/wire.bend` `img_scope`): markers become images only in
    user and context messages and in `run_typescript` results; in any
    other tool result (bash, file reads, `sb inspect`) a marker is text;
  - a marker value may not hold a newline, a CR or a backslash (the
    characters the wire escapes), so every scan agrees;
  - each image is printed as ONE part placeholder, the JSON string
    `"@@BENDPART:<style>|<mime>|<b64 path>|<name>@@"`; before every send,
    `runtime/provider.bend` `img_body` checks each file (in the store,
    there, non-empty, a multiple of 4 bytes, base64 in its first 64 bytes)
    and puts in its place the image object with its file part, or a text
    part `[image unavailable: <name> (<reason>)]`, for every family. The 20
    most recent good images go (`Im.res_cap`, stepped as before); older
    ones become such a text. Test: `tests/image_tag_e2e.py` (the fake
    provider answers Anthropic's 400 for bad base64, like the real API).

### Display

The feed shows a marker as `[Image #1 shots/login.png]` (user lines and
tool results), not the raw tag.

## Keys and flows

- `@name.png` in the popup, Tab or Enter: `[Image #N]` (flash "attached").
- Drag a file from Finder into the terminal (the terminal pastes
  `/path/Screen\ Shot.png `): `[Image #N]`; several files at once work.
- Ctrl+V: the clipboard image (a screenshot copied with Cmd+Ctrl+Shift+4).
  Its chip goes at the cursor and replaces the selection; one undo takes
  chip and attachment away.
- Cmd+V on an image-only clipboard, per terminal (the terminal owns
  Cmd+V; a text paste never reads the clipboard):
  - an empty or blank bracketed paste: the clipboard image, like Ctrl+V;
  - Ghostty 1.3: sends nothing by default. With
    `keybind = performable:super+v=paste_from_clipboard` in its config it
    pastes text as before and passes Cmd+V through when the clipboard has
    no text: it arrives as SUPER+V (kitty keyboard protocol) and attaches
    the image;
    (the same kind of line passes cmd+f to find in the history:
    `keybind = super+f=unbind`, book §16 "cmd+f");
  - iTerm2 3.7: asks "Paste Image"; "Save to Temp File and Paste Path"
    pastes the temp file's path, which attaches like a dropped file;
  - kitty and Ghostty tip: nothing, unless the app enables the paste
    events mode 5522 (OSC 5522, not done: crossterm cannot parse an OSC
    in input); Ctrl+V works everywhere.
- Deleting `[Image #N]` from the text drops the attachment; the next
  image reuses the free number.
- `run_typescript`: `return [{type:'image', path:'/tmp/s.png'}]` or
  `[{type:'image', data, mimeType}]`, text blocks allowed around.
  `self.readImages(paths)` (alias `self.readImage`, jsrt's prelude) builds
  the path blocks: one path or an array, relative to BEND_WORKDIR, each
  file checked (exists, PNG/JPEG/GIF/WebP by its bytes) or the program
  throws the reason; local, no tool call.

## Tests

- Rust unit tests: path normalization (quotes, `\ `, `file://`),
  magic-byte sniffing, PNG/JPEG size parsing, marker build/parse, the
  placeholder expansion on submit (deleted placeholder dropped), no
  panic on any input (empty, multibyte, truncated headers).
- jsrt: a program returning `[{type:'image',…}]` prints markers.
- Laws: the marker split, the OpenAI and Anthropic JSON of a user/tool
  message with a marker, and the unchanged JSON without marker.
- Live: single-agent and a Switchboard task on a throwaway hub
  (`SB_DEV_ROOT=/tmp/shot-*`): attach a PNG via `@` and via a dropped
  path, the model describes it; a `run_typescript` returning an image,
  the model describes it.
- Gates: cargo build, cargo test --workspace, clippy, `bend PROOF.bend`,
  `tests/run_all.sh`.

Done: `rust/images/src/tests.rs` (6), `attach.rs` test,
`fuzz_tests` (panic-audit) exercise the paste/Ctrl+V paths, LAWS.bend
`image_*` and `api_body_image_*` (7 laws), `tests/tui_images_tmux.py`
(in run_all.sh: @ pick, dropped path, plain paste, Ctrl+V, 3 image_url
parts with the PNG data at the fake provider, the feed shows
`[Image #1 shots/red-blue.png]`).

## Not done / later

- bend-jsrt is a 110 MB debug binary: an agent shell (50 MB file cap)
  cannot link it; rebuild it from a terminal (`cd rust/jsrt && cargo
  build`) after a change in `rust/jsrt` or `rust/images`.
- Downscaling needs macOS `sips`; elsewhere an image over 2048 px or
  3.75 MB is refused with a message.

- Images inside MCP tool results (an MCP server returning image content)
  and the Vibe "Additional content from `tools.x.y`" retention.
- A model without vision gets the provider's error as is.
- The context estimate counts a marker as its text, not the ~1.5k
  tokens an image costs.
