#!/usr/bin/env python3
"""third-party-notices.py - write THIRD_PARTY_NOTICES at the repo root.

    packaging/third-party-notices.py [--check]

The inventory of what the bise bundle ships or the repo vendors:
  - the Rust crates linked into `bise` (rust/, package bend-harness) and
    into `bend-jsrt` (rust/jsrt), from `cargo tree` (normal dependencies,
    no build or proc-macro crates, macOS and Linux targets) and the
    license files of each crate in ~/.cargo/registry (cargo-about style:
    for "A OR B" the first license of PREFER is used);
  - the parts that cargo does not see (V8 and the C/C++ libraries built
    into it, the Bend runtime and packages, gemoji, fonts): MANUAL below,
    their license texts in packaging/notices/.

It prints a summary (crates per license) and every FLAG: a license that
is not permissive (MPL: allowed unmodified, LGPL/GPL/AGPL: a decision)
or no license at all. --check: exit 1 when THIRD_PARTY_NOTICES is not
what this script writes (after a Cargo.lock change: run it again).
Offline: needs the crates in ~/.cargo/registry (any cargo build did it).
"""

import hashlib
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = subprocess.check_output(["git", "-C", HERE, "rev-parse", "--show-toplevel"], text=True).strip()
NOTICES = os.path.join(HERE, "notices")
OUT = os.path.join(REPO, "THIRD_PARTY_NOTICES")
TARGETS = ["aarch64-apple-darwin", "x86_64-apple-darwin", "x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]

# the licenses that need nothing but their text and notice kept
PERMISSIVE = {
    "MIT", "MIT-0", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "BSD-2-Clause", "BSD-3-Clause",
    "ISC", "Zlib", "Unlicense", "BSL-1.0", "Unicode-3.0", "Unicode-DFS-2016", "CDLA-Permissive-2.0",
    "CC0-1.0", "0BSD",
}
# file-level copyleft: fine unmodified, the source stays available (crates.io)
WEAK_COPYLEFT = {"MPL-2.0", "MPL-2.0+"}
# for "A OR B": the one we take
PREFER = ["MIT", "Apache-2.0", "BSD-3-Clause", "BSD-2-Clause", "ISC", "Zlib", "BSL-1.0", "Unlicense",
          "Apache-2.0 WITH LLVM-exception", "CC0-1.0", "MIT-0", "0BSD"]

# what cargo does not see. files: in packaging/notices/ (fetched once from
# the upstream repos at the version shipped; see notices/README.md)
MANUAL = [
    ("bend-jsrt: V8 and the libraries built into it", [
        dict(name="V8 15.0.245.2 (the JavaScript engine, in the rusty_v8 static library)",
             license="BSD-3-Clause", url="https://chromium.googlesource.com/v8/v8",
             files=["v8.LICENSE", "v8.LICENSE.fdlibm"]),
        dict(name="rusty_v8 150.4.0 (crate v8, the Rust binding)", license="MIT",
             url="https://github.com/denoland/rusty_v8", files=["rusty_v8.LICENSE"]),
        dict(name="glibc IBM Accurate Mathematical Library (sin/cos: V8's third_party/glibc, v8_use_libm_trig_functions)",
             license="LGPL-2.1-or-later", url="https://sourceware.org/git/?p=glibc.git",
             files=["v8-glibc.LICENSE"],
             notice="Used unmodified; its source: https://chromium.googlesource.com/v8/v8/+/15.0.245.2/third_party/glibc"),
        dict(name="ICU (V8's Intl; data built in)", license="Unicode-3.0",
             url="https://github.com/unicode-org/icu", files=["icu.LICENSE"]),
        dict(name="Abseil (abseil-cpp)", license="Apache-2.0",
             url="https://github.com/abseil/abseil-cpp", files=["abseil-cpp.LICENSE"]),
        dict(name="LLVM libc++, libc++abi, llvm-libc (V8's C++ runtime, use_custom_libcxx)",
             license="Apache-2.0 WITH LLVM-exception", url="https://github.com/llvm/llvm-project",
             files=["llvm.LICENSE"]),
        dict(name="simdutf", license="Apache-2.0 OR MIT", url="https://github.com/simdutf/simdutf",
             files=["simdutf.LICENSE-MIT"]),
        dict(name="FP16", license="MIT", url="https://github.com/Maratyszcza/FP16", files=["fp16.LICENSE"]),
        dict(name="fast_float", license="Apache-2.0 OR MIT OR BSL-1.0",
             url="https://github.com/fastfloat/fast_float", files=["fast_float.LICENSE-MIT"]),
        dict(name="Dragonbox", license="Apache-2.0 WITH LLVM-exception OR BSL-1.0",
             url="https://github.com/jk-jeon/dragonbox", files=["dragonbox.LICENSE-Boost"]),
        dict(name="UTF-8 decoder (Bjoern Hoehrmann, V8's third_party/utf8-decoder)", license="MIT",
             url="https://bjoern.hoehrmann.de/utf-8/decoder/dfa/", files=["v8-utf8-decoder.LICENSE"]),
        dict(name="SipHash (V8's third_party/siphash)", license="CC0-1.0",
             url="https://github.com/veorq/SipHash", files=["v8-siphash.LICENSE"]),
    ]),
    ("repl-live, repl-scripted, sb-core: the Bend runtime and packages", [
        dict(name="Bend (the compiler's C runtime, compiled into the three binaries)",
             license="Apache-2.0", url="https://github.com/bendlang/bend",
             notice="Copyright 2026 HigherOrderCO", files=[]),
        dict(name="BendHub package 0xbf477e663cf4acb1369a68e0f0fa713b (http, dns, url, encoding, zlib, json, wire; vendored and patched as bend/vendor/http/)",
             license="MIT-0", url="https://bend-lang.com (no LICENSE file: MIT-0 under the BendHub terms)", files=[]),
        dict(name="BendHub package 0x16458a2db4f36577294543ec2a6d420c (json)",
             license="MIT-0", url="https://bend-lang.com (no LICENSE file: MIT-0 under the BendHub terms)", files=[]),
        dict(name="BendHub package 0x1f4d6c03caf955232d0b0dc6e6f36cf4 (json, vendored and patched as bend/vendor/json.bend)",
             license="MIT-0", url="https://bend-lang.com (no LICENSE file: MIT-0 under the BendHub terms)", files=[]),
    ]),
    ("office/dist/office.js: the office SDK's bundle (office/sdk, docs/office.md)", [
        dict(name="fflate 0.8.3 (zip)", license="MIT", url="https://github.com/101arrowz/fflate",
             files=["fflate.LICENSE"]),
        dict(name="@xmldom/xmldom 0.9.12 (XML DOM)", license="MIT", url="https://github.com/xmldom/xmldom",
             files=["xmldom.LICENSE"]),
        dict(name="exceljs 4.4.0 (xlsx model and parts, src/sheets)", license="MIT", url="https://github.com/exceljs/exceljs",
             files=["exceljs.LICENSE"]),
        dict(name="saxes 5.0.1 (XML parser, used by exceljs's parts)", license="ISC",
             url="https://github.com/lddubeau/saxes (the npm package has no LICENSE file: ISC as its package.json says)",
             files=["saxes.LICENSE"]),
        dict(name="xmlchars 2.2.0 (XML character classes, used by saxes)", license="MIT", url="https://github.com/lddubeau/xmlchars",
             files=["xmlchars.LICENSE"]),
    ]),
    ("office/dist/office-view.js: the office viewer's docx renderer (office/sdk/src/view, docs/office.md)", [
        dict(name="docx-preview 0.4.1 (docx to HTML)", license="Apache-2.0", url="https://github.com/VolodymyrBaydalka/docxjs",
             files=["docx-preview.LICENSE"]),
        dict(name="jszip 3.10.2 (zip; dual MIT or GPL-3.0, taken under MIT)", license="MIT", url="https://github.com/Stuk/jszip",
             files=["jszip.LICENSE"]),
    ]),
    ("office/dist/office-pdf.js and pdf.worker.js: the office viewer's pdf renderer and its worker (office/sdk/src/view, pdf-notes)", [
        dict(name="pdfjs-dist 6.4.299 (pdf.js: pdf to canvas and text)", license="Apache-2.0", url="https://github.com/mozilla/pdf.js",
             files=["pdfjs-dist.LICENSE"]),
    ]),
    ("office/dist/office-md.js: the office viewer's markdown renderer, micromark and GFM (office/sdk/src/view/markdown.ts, md-artifact)", [
        dict(name="character-entities 2.0.2", license="MIT", url="https://github.com/wooorm/character-entities",
             files=['office-md/character-entities.LICENSE']),
        dict(name="decode-named-character-reference 1.3.0", license="MIT", url="https://github.com/wooorm/decode-named-character-reference",
             files=['office-md/decode-named-character-reference.LICENSE']),
        dict(name="micromark 4.0.3", license="MIT", url="https://github.com/micromark/micromark",
             files=['office-md/micromark.LICENSE']),
        dict(name="micromark-core-commonmark 2.0.4", license="MIT", url="https://github.com/micromark/micromark",
             files=['office-md/micromark-core-commonmark.LICENSE']),
        dict(name="micromark-extension-gfm 3.0.0", license="MIT", url="https://github.com/micromark/micromark-extension-gfm",
             files=['office-md/micromark-extension-gfm.LICENSE']),
        dict(name="micromark-extension-gfm-autolink-literal 2.1.0", license="MIT", url="https://github.com/micromark/micromark-extension-gfm-autolink-literal",
             files=['office-md/micromark-extension-gfm-autolink-literal.LICENSE']),
        dict(name="micromark-extension-gfm-footnote 2.1.0", license="MIT", url="https://github.com/micromark/micromark-extension-gfm-footnote",
             files=['office-md/micromark-extension-gfm-footnote.LICENSE']),
        dict(name="micromark-extension-gfm-strikethrough 2.1.0", license="MIT", url="https://github.com/micromark/micromark-extension-gfm-strikethrough",
             files=['office-md/micromark-extension-gfm-strikethrough.LICENSE']),
        dict(name="micromark-extension-gfm-table 2.1.2", license="MIT", url="https://github.com/micromark/micromark-extension-gfm-table",
             files=['office-md/micromark-extension-gfm-table.LICENSE']),
        dict(name="micromark-extension-gfm-tagfilter 2.0.0", license="MIT", url="https://github.com/micromark/micromark-extension-gfm-tagfilter",
             files=['office-md/micromark-extension-gfm-tagfilter.LICENSE']),
        dict(name="micromark-extension-gfm-task-list-item 2.1.0", license="MIT", url="https://github.com/micromark/micromark-extension-gfm-task-list-item",
             files=['office-md/micromark-extension-gfm-task-list-item.LICENSE']),
        dict(name="micromark-factory-destination 2.0.1", license="MIT", url="https://github.com/micromark/micromark",
             files=['office-md/micromark-factory-destination.LICENSE']),
        dict(name="micromark-factory-label 2.0.1", license="MIT", url="https://github.com/micromark/micromark",
             files=['office-md/micromark-factory-label.LICENSE']),
        dict(name="micromark-factory-space 2.1.0", license="MIT", url="https://github.com/micromark/micromark",
             files=['office-md/micromark-factory-space.LICENSE']),
        dict(name="micromark-factory-title 2.0.1", license="MIT", url="https://github.com/micromark/micromark",
             files=['office-md/micromark-factory-title.LICENSE']),
        dict(name="micromark-factory-whitespace 2.0.1", license="MIT", url="https://github.com/micromark/micromark",
             files=['office-md/micromark-factory-whitespace.LICENSE']),
        dict(name="micromark-util-character 2.1.1", license="MIT", url="https://github.com/micromark/micromark",
             files=['office-md/micromark-util-character.LICENSE']),
        dict(name="micromark-util-chunked 2.0.1", license="MIT", url="https://github.com/micromark/micromark",
             files=['office-md/micromark-util-chunked.LICENSE']),
        dict(name="micromark-util-classify-character 2.0.1", license="MIT", url="https://github.com/micromark/micromark",
             files=['office-md/micromark-util-classify-character.LICENSE']),
        dict(name="micromark-util-combine-extensions 2.0.1", license="MIT", url="https://github.com/micromark/micromark",
             files=['office-md/micromark-util-combine-extensions.LICENSE']),
        dict(name="micromark-util-decode-numeric-character-reference 2.0.2", license="MIT", url="https://github.com/micromark/micromark",
             files=['office-md/micromark-util-decode-numeric-character-reference.LICENSE']),
        dict(name="micromark-util-edit-map 1.0.0", license="MIT", url="https://github.com/micromark/micromark",
             files=['office-md/micromark-util-edit-map.LICENSE']),
        dict(name="micromark-util-encode 2.0.1", license="MIT", url="https://github.com/micromark/micromark",
             files=['office-md/micromark-util-encode.LICENSE']),
        dict(name="micromark-util-html-tag-name 2.0.1", license="MIT", url="https://github.com/micromark/micromark",
             files=['office-md/micromark-util-html-tag-name.LICENSE']),
        dict(name="micromark-util-normalize-identifier 2.0.1", license="MIT", url="https://github.com/micromark/micromark",
             files=['office-md/micromark-util-normalize-identifier.LICENSE']),
        dict(name="micromark-util-resolve-all 2.0.1", license="MIT", url="https://github.com/micromark/micromark",
             files=['office-md/micromark-util-resolve-all.LICENSE']),
        dict(name="micromark-util-sanitize-uri 2.0.1", license="MIT", url="https://github.com/micromark/micromark",
             files=['office-md/micromark-util-sanitize-uri.LICENSE']),
        dict(name="micromark-util-subtokenize 2.1.0", license="MIT", url="https://github.com/micromark/micromark",
             files=['office-md/micromark-util-subtokenize.LICENSE']),
    ]),
    ("office/dist/office-sheet.js: the office viewer's xlsx grid, Univer and what it bundles (office/sdk/src/view/sheet.ts, xlsx-univer; built, never committed)", [
        dict(name="@flatten-js/interval-tree 1.1.3", license="MIT", url="https://github.com/alexbol99/flatten-interval-tree",
             files=['office-sheet/flatten-js__interval-tree.LICENSE']),
        dict(name="@floating-ui/core 1.8.0", license="MIT", url="https://github.com/floating-ui/floating-ui",
             files=['office-sheet/floating-ui__core.LICENSE']),
        dict(name="@floating-ui/dom 1.8.0", license="MIT", url="https://github.com/floating-ui/floating-ui",
             files=['office-sheet/floating-ui__dom.LICENSE']),
        dict(name="@floating-ui/react-dom 2.1.9", license="MIT", url="https://github.com/floating-ui/floating-ui",
             files=['office-sheet/floating-ui__react-dom.LICENSE']),
        dict(name="@floating-ui/utils 0.2.12", license="MIT", url="https://github.com/floating-ui/floating-ui",
             files=['office-sheet/floating-ui__utils.LICENSE']),
        dict(name="@radix-ui/primitive 1.2.0", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__primitive.LICENSE']),
        dict(name="@radix-ui/react-arrow 1.1.17", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-arrow.LICENSE']),
        dict(name="@radix-ui/react-collection 1.2.0", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-collection.LICENSE']),
        dict(name="@radix-ui/react-compose-refs 1.1.5", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-compose-refs.LICENSE']),
        dict(name="@radix-ui/react-context 1.3.0", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-context.LICENSE']),
        dict(name="@radix-ui/react-dialog 1.3.1", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-dialog.LICENSE']),
        dict(name="@radix-ui/react-direction 1.2.0", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-direction.LICENSE']),
        dict(name="@radix-ui/react-dismissable-layer 1.2.1", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-dismissable-layer.LICENSE']),
        dict(name="@radix-ui/react-dropdown-menu 2.2.1", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-dropdown-menu.LICENSE']),
        dict(name="@radix-ui/react-focus-guards 1.2.0", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-focus-guards.LICENSE']),
        dict(name="@radix-ui/react-focus-scope 1.3.1", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-focus-scope.LICENSE']),
        dict(name="@radix-ui/react-hover-card 1.2.1", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-hover-card.LICENSE']),
        dict(name="@radix-ui/react-id 1.1.4", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-id.LICENSE']),
        dict(name="@radix-ui/react-menu 2.2.1", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-menu.LICENSE']),
        dict(name="@radix-ui/react-popover 1.3.1", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-popover.LICENSE']),
        dict(name="@radix-ui/react-popper 1.4.0", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-popper.LICENSE']),
        dict(name="@radix-ui/react-portal 1.2.0", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-portal.LICENSE']),
        dict(name="@radix-ui/react-presence 1.1.11", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-presence.LICENSE']),
        dict(name="@radix-ui/react-primitive 2.2.0", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-primitive.LICENSE']),
        dict(name="@radix-ui/react-roving-focus 1.2.1", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-roving-focus.LICENSE']),
        dict(name="@radix-ui/react-separator 1.2.0", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-separator.LICENSE']),
        dict(name="@radix-ui/react-slot 1.5.0", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-slot.LICENSE']),
        dict(name="@radix-ui/react-use-callback-ref 1.1.5", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-use-callback-ref.LICENSE']),
        dict(name="@radix-ui/react-use-controllable-state 1.3.0", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-use-controllable-state.LICENSE']),
        dict(name="@radix-ui/react-use-effect-event 0.0.5", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-use-effect-event.LICENSE']),
        dict(name="@radix-ui/react-use-is-hydrated 0.1.3", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-use-is-hydrated.LICENSE']),
        dict(name="@radix-ui/react-use-layout-effect 1.1.4", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-use-layout-effect.LICENSE']),
        dict(name="@radix-ui/react-use-size 1.1.5", license="MIT", url="https://github.com/radix-ui/primitives",
             files=['office-sheet/radix-ui__react-use-size.LICENSE']),
        dict(name="@univerjs/core 1.1.0", license="Apache-2.0", url="https://github.com/dream-num/univer",
             files=['office-sheet/univerjs__core.LICENSE']),
        dict(name="@univerjs/design 1.1.0", license="Apache-2.0", url="https://github.com/dream-num/univer",
             files=['office-sheet/univerjs__design.LICENSE']),
        dict(name="@univerjs/docs 1.1.0", license="Apache-2.0", url="https://github.com/dream-num/univer",
             files=['office-sheet/univerjs__docs.LICENSE']),
        dict(name="@univerjs/docs-ui 1.1.0", license="Apache-2.0", url="https://github.com/dream-num/univer",
             files=['office-sheet/univerjs__docs-ui.LICENSE']),
        dict(name="@univerjs/drawing 1.1.0", license="Apache-2.0", url="https://github.com/dream-num/univer",
             files=['office-sheet/univerjs__drawing.LICENSE']),
        dict(name="@univerjs/engine-formula 1.1.0", license="Apache-2.0", url="https://github.com/dream-num/univer",
             files=['office-sheet/univerjs__engine-formula.LICENSE']),
        dict(name="@univerjs/engine-render 1.1.0", license="Apache-2.0", url="https://github.com/dream-num/univer",
             files=['office-sheet/univerjs__engine-render.LICENSE']),
        dict(name="@univerjs/icons 1.46.0", license="MIT", url="https://github.com/dream-num/univer-icons",
             files=['office-sheet/univerjs__icons.LICENSE']),
        dict(name="@univerjs/network 1.1.0", license="Apache-2.0", url="https://github.com/dream-num/univer",
             files=['office-sheet/univerjs__network.LICENSE']),
        dict(name="@univerjs/preset-sheets-core 1.1.0", license="Apache-2.0", url="https://github.com/dream-num/univer",
             files=['office-sheet/univerjs__preset-sheets-core.LICENSE']),
        dict(name="@univerjs/protocol 1.1.0", license="Apache-2.0", url="https://github.com/dream-num/univer (the npm package has no LICENSE file: Apache-2.0 as its package.json says)",
             files=['office-sheet/univerjs__core.LICENSE']),
        dict(name="@univerjs/rpc 1.1.0", license="Apache-2.0", url="https://github.com/dream-num/univer",
             files=['office-sheet/univerjs__rpc.LICENSE']),
        dict(name="@univerjs/sheets 1.1.0", license="Apache-2.0", url="https://github.com/dream-num/univer",
             files=['office-sheet/univerjs__sheets.LICENSE']),
        dict(name="@univerjs/sheets-formula 1.1.0", license="Apache-2.0", url="https://github.com/dream-num/univer",
             files=['office-sheet/univerjs__sheets-formula.LICENSE']),
        dict(name="@univerjs/sheets-formula-ui 1.1.0", license="Apache-2.0", url="https://github.com/dream-num/univer",
             files=['office-sheet/univerjs__sheets-formula-ui.LICENSE']),
        dict(name="@univerjs/sheets-numfmt 1.1.0", license="Apache-2.0", url="https://github.com/dream-num/univer",
             files=['office-sheet/univerjs__sheets-numfmt.LICENSE']),
        dict(name="@univerjs/sheets-numfmt-ui 1.1.0", license="Apache-2.0", url="https://github.com/dream-num/univer",
             files=['office-sheet/univerjs__sheets-numfmt-ui.LICENSE']),
        dict(name="@univerjs/sheets-ui 1.1.0", license="Apache-2.0", url="https://github.com/dream-num/univer",
             files=['office-sheet/univerjs__sheets-ui.LICENSE']),
        dict(name="@univerjs/telemetry 1.1.0", license="Apache-2.0", url="https://github.com/dream-num/univer",
             files=['office-sheet/univerjs__telemetry.LICENSE']),
        dict(name="@univerjs/themes 1.1.0", license="Apache-2.0", url="https://github.com/dream-num/univer",
             files=['office-sheet/univerjs__themes.LICENSE']),
        dict(name="@univerjs/ui 1.1.0", license="Apache-2.0", url="https://github.com/dream-num/univer",
             files=['office-sheet/univerjs__ui.LICENSE']),
        dict(name="@wendellhu/redi 1.1.3", license="MIT", url="https://github.com/wzhudev/redi",
             files=['office-sheet/wendellhu__redi.LICENSE']),
        dict(name="aria-hidden 1.2.6", license="MIT", url="https://github.com/theKashey/aria-hidden",
             files=['office-sheet/aria-hidden.LICENSE']),
        dict(name="async-lock 1.4.1", license="MIT", url="https://github.com/rogierschouten/async-lock",
             files=['office-sheet/async-lock.LICENSE']),
        dict(name="class-variance-authority 0.7.1", license="Apache-2.0", url="https://github.com/joe-bell/cva",
             files=['office-sheet/class-variance-authority.LICENSE']),
        dict(name="clsx 2.1.1", license="MIT", url="lukeed/clsx",
             files=['office-sheet/clsx.LICENSE']),
        dict(name="cmdk 1.1.1", license="MIT", url="https://github.com/pacocoursey/cmdk",
             files=['office-sheet/cmdk.LICENSE']),
        dict(name="collapse-white-space 2.1.0", license="MIT", url="wooorm/collapse-white-space",
             files=['office-sheet/collapse-white-space.LICENSE']),
        dict(name="decimal.js 10.6.0", license="MIT", url="https://github.com/MikeMcl/decimal.js (the npm package has no LICENSE file: MIT as its package.json says)",
             files=['MIT.spdx.txt']),
        dict(name="detect-node-es 1.1.0", license="MIT", url="https://github.com/thekashey/detect-node",
             files=['office-sheet/detect-node-es.LICENSE']),
        dict(name="fast-diff 1.3.0", license="Apache-2.0", url="https://github.com/jhchen/fast-diff",
             files=['office-sheet/fast-diff.LICENSE']),
        dict(name="franc-min 6.2.0", license="MIT", url="https://github.com/wooorm/franc/tree/main/packages/franc-min (the npm package has no LICENSE file: MIT as its package.json says)",
             files=['MIT.spdx.txt']),
        dict(name="get-nonce 1.0.1", license="MIT", url="git@github.com:theKashey/get-nonce",
             files=['office-sheet/get-nonce.LICENSE']),
        dict(name="kdbush 4.1.0", license="ISC", url="https://github.com/mourner/kdbush",
             files=['office-sheet/kdbush.LICENSE']),
        dict(name="lodash-es 4.18.1", license="MIT", url="lodash/lodash",
             files=['office-sheet/lodash-es.LICENSE']),
        dict(name="n-gram 2.0.2", license="MIT", url="words/n-gram",
             files=['office-sheet/n-gram.LICENSE']),
        dict(name="ot-json1 1.0.2", license="ISC", url="https://github.com/josephg/json1 (the npm package has no LICENSE file: ISC as its package.json says)",
             files=['office-sheet/ISC.spdx.txt']),
        dict(name="ot-text-unicode 4.0.0", license="ISC", url="https://github.com/ottypes/text (the npm package has no LICENSE file: ISC as its package.json says)",
             files=['office-sheet/ISC.spdx.txt']),
        dict(name="quickselect 3.0.0", license="ISC", url="github:mourner/quickselect",
             files=['office-sheet/quickselect.LICENSE']),
        dict(name="rbush 4.0.1", license="MIT", url="https://github.com/mourner/rbush",
             files=['office-sheet/rbush.LICENSE']),
        dict(name="react 18.3.1", license="MIT", url="https://github.com/facebook/react",
             files=['office-sheet/react.LICENSE']),
        dict(name="react-dom 18.3.1", license="MIT", url="https://github.com/facebook/react",
             files=['office-sheet/react-dom.LICENSE']),
        dict(name="react-remove-scroll 2.7.2", license="MIT", url="https://github.com/theKashey/react-remove-scroll",
             files=['office-sheet/react-remove-scroll.LICENSE']),
        dict(name="react-remove-scroll-bar 2.3.8", license="MIT", url="https://github.com/theKashey/react-remove-scroll-bar (the npm package has no LICENSE file: MIT as its package.json says)",
             files=['MIT.spdx.txt']),
        dict(name="react-style-singleton 2.2.3", license="MIT", url="https://github.com/theKashey/react-style-singleton",
             files=['office-sheet/react-style-singleton.LICENSE']),
        dict(name="rxjs 7.8.2", license="Apache-2.0", url="https://github.com/reactivex/rxjs",
             files=['office-sheet/rxjs.LICENSE']),
        dict(name="scheduler 0.23.2", license="MIT", url="https://github.com/facebook/react",
             files=['office-sheet/scheduler.LICENSE']),
        dict(name="sonner 2.0.8", license="MIT", url="https://github.com/emilkowalski/sonner",
             files=['office-sheet/sonner.LICENSE']),
        dict(name="trigram-utils 2.0.1", license="MIT", url="wooorm/trigram-utils",
             files=['office-sheet/trigram-utils.LICENSE']),
        dict(name="tslib 2.8.1", license="0BSD", url="https://github.com/Microsoft/tslib",
             files=['office-sheet/tslib.LICENSE']),
        dict(name="unicount 1.1.0", license="ISC", url="https://github.com/josephg/unicount (the npm package has no LICENSE file: ISC as its package.json says)",
             files=['office-sheet/ISC.spdx.txt']),
        dict(name="use-callback-ref 1.3.3", license="MIT", url="https://github.com/theKashey/use-callback-ref",
             files=['office-sheet/use-callback-ref.LICENSE']),
        dict(name="use-sidecar 1.1.3", license="MIT", url="https://github.com/theKashey/use-sidecar",
             files=['office-sheet/use-sidecar.LICENSE']),
    ]),
    ("apps/desktop: the bims office's renderer chunk (src/bims/render, loaded on demand)", [
        dict(name="three 0.186.1 (three.js: the WebGL renderer, scene and geometry)", license="MIT",
             url="https://github.com/mrdoob/three.js", files=["three.LICENSE"]),
    ]),
    ("bise: data built in", [
        dict(name="gemoji (the emoji shortcode table, rust/tui/src/emoji.tsv)", license="MIT",
             url="https://github.com/github/gemoji", files=["gemoji.LICENSE"]),
    ]),
]

# crates whose package has no license file: the upstream repo's, fetched
# once into notices/ (sys_traits has none upstream either: the SPDX MIT
# text, and a FLAG)
CRATE_FILES = {
    "v8": ["rusty_v8.LICENSE"],
    "deno_core": ["deno.LICENSE.md"],
    "serde_v8": ["deno.LICENSE.md"],
    "base64-simd": ["nugine-simd.LICENSE"],
    "vsimd": ["nugine-simd.LICENSE"],
    "dasp_sample": ["dasp.LICENSE-MIT"],
    "dprint-swc-ext": ["dprint-swc-ext.LICENSE"],
    "sys_traits": ["MIT.spdx.txt"],
}
NO_UPSTREAM_TEXT = {"sys_traits"}

NOT_SHIPPED = """\
Not shipped, listed for completeness:
  - OpenSSL 3: on Linux, the Bend http package loads the system's
    libssl/libcrypto at run time (Apache-2.0); bise does not ship it. On
    macOS it uses the system's TLS (Security.framework).
  - JetBrains Mono (SIL Open Font License 1.1): the site
    (site) loads it from Google Fonts; no
    font file is in the repo or the bundle.
  - The Rust standard library (MIT OR Apache-2.0) is linked into every Rust
    binary; its license is the Apache-2.0 text below.
"""


def sh(args, cwd):
    return subprocess.check_output(args, cwd=cwd, text=True)


def crates(ws, pkg):
    """{(name, version)} linked into the binary of the workspace at ws"""
    args = ["cargo", "tree", "--offline", "-e", "normal,no-proc-macro", "--prefix", "none", "-f", "{p}"]
    for t in TARGETS:
        args += ["--target", t]
    if pkg:
        args += ["-p", pkg]
    out = set()
    for line in sh(args, ws).splitlines():
        m = re.match(r"(\S+) v(\S+)( \((.*)\))?", line)
        if not m:
            continue
        path = m.group(4) or ""
        # our own crates (a path in the repo, not rust/vendor): not third party
        if path.startswith("/") and "/rust/vendor/" not in path:
            continue
        out.add((m.group(1), m.group(2)))
    return out


def metadata(ws):
    md = json.loads(sh(["cargo", "metadata", "--offline", "--format-version", "1"], ws))
    return {(p["name"], p["version"]): p for p in md["packages"]}


def norm_spdx(expr):
    expr = (expr or "").strip()
    # old style "MIT/Apache-2.0"
    if "/" in expr and " OR " not in expr and "(" not in expr:
        expr = " OR ".join(x.strip() for x in expr.split("/"))
    return expr


def choose(expr):
    """the license we use for this crate, or None (all of expr applies)"""
    if not expr or "(" in expr or " AND " in expr:
        return None
    opts = [o.strip() for o in expr.split(" OR ")]
    if len(opts) == 1:
        return opts[0]
    for p in PREFER:
        if p in opts:
            return p
    return None


def ids(expr):
    return {t for t in re.split(r"\s+(?:AND|OR)\s+|[()]", expr.replace(" WITH ", "_WITH_")) if t.strip()}


def classify(expr):
    """ok | weak | FLAG"""
    if not expr:
        return "FLAG"
    c = choose(expr)
    need = {c} if c else {i.replace("_WITH_", " WITH ") for i in ids(expr)}
    if need <= PERMISSIVE:
        return "ok"
    if need <= PERMISSIVE | WEAK_COPYLEFT:
        return "weak"
    return "FLAG"


LICENSE_FILE = re.compile(r"^(LICEN[CS]E|COPYING|NOTICE|UNLICENSE|COPYRIGHT)", re.I)


def license_files(root):
    out = []
    for f in sorted(os.listdir(root)):
        p = os.path.join(root, f)
        if LICENSE_FILE.match(f) and os.path.isfile(p):
            out.append(p)
        elif f.lower() in ("license", "licenses") and os.path.isdir(p):
            out += [os.path.join(p, g) for g in sorted(os.listdir(p)) if os.path.isfile(os.path.join(p, g))]
    return out


def kind(text):
    t = text[:3000]
    if "Apache License" in t and "Version 2.0" in t:
        return "Apache-2.0"
    if "Permission is hereby granted, free of charge" in t:
        return "MIT"
    if "This is free and unencumbered software" in t:
        return "Unlicense"
    return None


def pick_files(files, chosen):
    """the files of the chosen license (all of them when unsure)"""
    if not chosen or len(files) <= 1:
        return files
    keep = []
    for f in files:
        name = os.path.basename(f).upper()
        k = kind(read(f))
        if chosen == "MIT" and ("MIT" in name or k == "MIT"):
            keep.append(f)
        elif chosen == "Apache-2.0" and ("APACHE" in name or k == "Apache-2.0"):
            keep.append(f)
        elif name.startswith("NOTICE") or name.startswith("COPYRIGHT"):
            keep.append(f)
    return keep if [f for f in keep if not os.path.basename(f).upper().startswith(("NOTICE", "COPYRIGHT"))] else files


def read(p):
    with open(p, encoding="utf-8", errors="replace") as fh:
        return fh.read()


def clean(text):
    lines = [ln.rstrip() for ln in text.replace("\r\n", "\n").replace("\t", "    ").split("\n")]
    while lines and not lines[0]:
        lines.pop(0)
    while lines and not lines[-1]:
        lines.pop()
    return "\n".join(lines)


def main():
    check = "--check" in sys.argv[1:]
    groups = [("bise", os.path.join(REPO, "rust"), "bend-harness"),
              ("bend-jsrt", os.path.join(REPO, "rust", "jsrt"), None)]
    entries = {}  # (name, version) -> dict
    for binary, ws, pkg in groups:
        md = metadata(ws)
        for key in crates(ws, pkg):
            p = md[key]
            e = entries.setdefault(key, dict(name=key[0], version=key[1], license=norm_spdx(p.get("license")),
                                              url=p.get("repository") or p.get("homepage") or "https://crates.io/crates/" + key[0],
                                              root=os.path.dirname(p["manifest_path"]), bins=set()))
            e["bins"].add(binary)

    texts = {}  # sha -> [text, [users]]
    order = []

    def add_text(text, user):
        text = clean(text)
        h = hashlib.sha256(re.sub(r"\s+", " ", text).encode()).hexdigest()
        if h not in texts:
            texts[h] = [text, []]
            order.append(h)
        if user not in texts[h][1]:
            texts[h][1].append(user)

    apache = clean(read(os.path.join(REPO, "LICENSE")))
    flags, counts, missing = [], {}, []
    for key in sorted(entries, key=lambda k: (k[0].lower(), k[1])):
        e = entries[key]
        user = f"{e['name']} {e['version']}"
        chosen = choose(e["license"])
        cls = classify(e["license"])
        counts[e["license"] or "(none)"] = counts.get(e["license"] or "(none)", 0) + 1
        if cls != "ok":
            flags.append((cls, user, e["license"] or "(no license field)", "/".join(sorted(e["bins"]))))
        files = pick_files(license_files(e["root"]), chosen)
        if not files and e["name"] in CRATE_FILES:
            files = [os.path.join(NOTICES, f) for f in CRATE_FILES[e["name"]]]
        if e["name"] in NO_UPSTREAM_TEXT:
            flags.append(("no-text", user, e["license"] + ": no license file in the crate or its repo; the SPDX text is used",
                          "/".join(sorted(e["bins"]))))
        e["chosen"] = chosen
        if not files:
            if chosen == "Apache-2.0":
                add_text(apache, user)  # the standard text, as in LICENSE
            else:
                missing.append(user)
        for f in files:
            add_text(read(f), user)

    manual_counts = {}
    for _, items in MANUAL:
        for it in items:
            manual_counts[it["license"]] = manual_counts.get(it["license"], 0) + 1
            cls = classify(it["license"])
            if cls != "ok":
                flags.append((cls, it["name"], it["license"], "manual"))

    w = []
    w.append("bise - third-party notices")
    w.append("==========================")
    w.append("")
    w.append("bise is licensed under the Apache License 2.0 (see LICENSE). The bise")
    w.append("distribution contains, or is built from, the third-party software listed")
    w.append("here, each under its own license. The license texts follow the lists.")
    w.append("")
    w.append("Generated by packaging/third-party-notices.py from")
    w.append("rust/Cargo.lock and rust/jsrt/Cargo.lock: do not edit by hand.")
    w.append("")
    w.append("1. Summary")
    w.append("")
    w.append(f"Rust crates ({len(entries)}), by license expression:")
    for lic, n in sorted(counts.items(), key=lambda x: (-x[1], x[0])):
        w.append(f"  {n:4d}  {lic}")
    w.append("")
    w.append("Other components (section 3), by license:")
    for lic, n in sorted(manual_counts.items(), key=lambda x: (-x[1], x[0])):
        w.append(f"  {n:4d}  {lic}")
    w.append("")
    w.append("2. Rust crates")
    w.append("")
    w.append("   Binary: bise = the `bise` command (rust/), jsrt = `bend-jsrt` (rust/jsrt).")
    w.append("   For a choice of licenses (\"A OR B\"), bise uses the one after \"->\".")
    w.append("")
    for key in sorted(entries, key=lambda k: (k[0].lower(), k[1])):
        e = entries[key]
        lic = e["license"] or "(no license field)"
        if e["chosen"] and e["chosen"] != e["license"]:
            lic += f" -> {e['chosen']}"
        bins = ",".join("jsrt" if b == "bend-jsrt" else b for b in sorted(e["bins"]))
        w.append(f"  {e['name']} {e['version']}  [{bins}]  {lic}")
        w.append(f"      {e['url']}")
    w.append("")
    mpl = [entries[k] for k in sorted(entries) if classify(entries[k]["license"]) == "weak"]
    if mpl:
        w.append("   The MPL-2.0 crates are used unmodified; their source code:")
        for e in mpl:
            w.append(f"      https://crates.io/crates/{e['name']}/{e['version']}")
        w.append("")
    w.append("   rust/vendor/crossterm is crossterm 0.28.1 with a patch (see its")
    w.append("   README-bise.md), under its MIT license (rust/vendor/crossterm/LICENSE).")
    w.append("")
    w.append("3. Other components")
    for title, items in MANUAL:
        w.append("")
        w.append(f"   {title}")
        w.append("")
        for it in items:
            w.append(f"  {it['name']}  {it['license']}")
            w.append(f"      {it['url']}")
            if it.get("notice"):
                w.append(f"      {it['notice']}")
            for f in it["files"]:
                add_text(read(os.path.join(NOTICES, f)), it["name"].split(" (")[0])
            if it["license"].startswith("Apache-2.0") and not it["files"]:
                add_text(apache, it["name"].split(" (")[0])
    w.append("")
    w.append(NOT_SHIPPED.rstrip())
    w.append("")
    w.append("4. License texts")
    w.append("")
    w.append("   Each text once, with the components it applies to.")
    for h in order:
        text, users = texts[h]
        w.append("")
        w.append("-" * 78)
        w.append("Used by: " + ", ".join(users))
        w.append("-" * 78)
        w.append("")
        w.append(text)
    w.append("")
    out = "\n".join(w)

    print(f"{len(entries)} crates, {len(texts)} distinct license texts", file=sys.stderr)
    for lic, n in sorted(counts.items(), key=lambda x: (-x[1], x[0])):
        print(f"  {n:4d}  {lic}", file=sys.stderr)
    for cls, who, lic, where in flags:
        print(f"FLAG {cls}: {who}: {lic} ({where})", file=sys.stderr)
    for m in missing:
        print(f"MISSING license file: {m}", file=sys.stderr)
    if check:
        cur = read(OUT) if os.path.exists(OUT) else ""
        if cur != out:
            print("THIRD_PARTY_NOTICES is out of date: run " + os.path.relpath(__file__, REPO), file=sys.stderr)
            return 1
        return 0
    with open(OUT, "w", encoding="utf-8") as fh:
        fh.write(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
