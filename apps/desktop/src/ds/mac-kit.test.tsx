// @vitest-environment happy-dom
// src/ds · bise fait mac's shared components (page native-mac §6, nm-ds): what each draws for its
// props, what it says to VoiceOver and in its tip, what a click sends; and noClip's walk. The sizes
// are tokens.css's (lint.test.ts reads their values); a box's pixels are ui-review's shots.
import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ds } from "@/proto/words_ds.gen";
import { entityKey } from "@/shared/entity";
import { APP_ICONS, AppIcon, BannerWait, Filters, GroupLabel, ListBar, ListRow, ListSearch, MoreButton, PageBanner, roomOf, RowAction, sentenceCase, type EntityMenu } from "./index";
import { clippers, noClip } from "./noclip";

afterEach(cleanup);

const menu = (): EntityMenu & { opened: number; right: number } => {
  const m = { opened: 0, right: 0, open: () => { m.opened += 1; }, onContextMenu: () => { m.right += 1; } };
  return m;
};

describe("PageBanner", () => {
  it("the title, the sentence, then the facts with the accent only on what waits; no button in it", () => {
    const { getByTestId } = render(<PageBanner title="Inbox" sentence="what your agents wait on you for." facts={<><BannerWait>3 waiting</BannerWait> · the oldest 21 d ago</>} icon="inbox" testId="b" />);
    const b = getByTestId("b");
    expect(b.querySelector("h1.title")?.textContent).toBe("Inbox");
    expect(b.querySelector(".say")?.textContent).toBe("what your agents wait on you for.");
    expect(b.querySelector(".facts")?.textContent).toBe("3 waiting · the oldest 21 d ago");
    expect([...b.querySelectorAll(".wait")].map((n) => n.textContent)).toEqual(["3 waiting"]);
    expect(b.querySelector("button")).toBeNull();
  });

  it("no facts line when the place has no live number", () => {
    const { getByTestId } = render(<PageBanner title="t" sentence="s" icon="inbox" testId="b" />);
    expect(getByTestId("b").querySelector(".facts")).toBeNull();
  });

  it("never an empty box: the page's line icon with no vignette, the vignette when it has one", () => {
    const { getByTestId, unmount } = render(<PageBanner title="t" sentence="s" icon="scheduled" testId="b" />);
    expect(getByTestId("b.visual").getAttribute("data-fallback")).toBe("true");
    expect(getByTestId("b.visual").querySelector('[data-icon="scheduled"]')).not.toBeNull();
    unmount();
    const r = render(<PageBanner title="t" sentence="s" icon="scheduled" visual={<img className="light" alt="" />} testId="b" />);
    expect(r.getByTestId("b.visual").hasAttribute("data-fallback")).toBe(false);
    expect(r.getByTestId("b.visual").querySelector("img.light")).not.toBeNull();
    expect(r.getByTestId("b.visual").getAttribute("aria-hidden")).toBe("true");
  });

  it("its edge (a column page's or 32 in) and the right side's gutter as --banner-gutter", () => {
    const { getByTestId, unmount } = render(<PageBanner title="t" sentence="s" icon="inbox" gutter={172} testId="b" />);
    expect(getByTestId("b").getAttribute("data-edge")).toBe("column");
    expect(getByTestId("b").style.getPropertyValue("--banner-gutter")).toBe("172px");
    unmount();
    const r = render(<PageBanner title="t" sentence="s" icon="inbox" edge="page" testId="b" />);
    expect(r.getByTestId("b").getAttribute("data-edge")).toBe("page");
    expect(r.getByTestId("b").style.getPropertyValue("--banner-gutter")).toBe("");
  });

  it("its right margin as a band, so its sizes key on the content area (ui-review on 40829c85f)", () => {
    expect([0, 24, 92, 172].map(roomOf)).toEqual(["24", "24", "92", "172"]);
    const { getByTestId } = render(<PageBanner title="t" sentence="s" icon="inbox" gutter={172} testId="b" />);
    expect(getByTestId("b").getAttribute("data-room")).toBe("172");
  });
});

describe("sentenceCase", () => {
  it("a chrome label's first letter up, the rest as written", () => {
    expect(["run now", "show in Finder", "", "Open"].map(sentenceCase)).toEqual(["Run now", "Show in Finder", "", "Open"]);
  });
});

describe("Filters", () => {
  const items = [{ id: "waiting", label: "Waiting" }, { id: "answered", label: "Answered", tip: "12 answered" }];

  it("a tablist, the chosen tab selected and the only one in the tab order, each with its test id", () => {
    const { getByTestId } = render(<Filters items={items} on="waiting" label="show" testId="f" />);
    expect(getByTestId("f").getAttribute("role")).toBe("tablist");
    expect(getByTestId("f").getAttribute("aria-label")).toBe("show");
    const on = getByTestId("f.waiting");
    const off = getByTestId("f.answered");
    expect([on.getAttribute("aria-selected"), on.tabIndex, on.className]).toEqual(["true", 0, "on"]);
    expect([off.getAttribute("aria-selected"), off.tabIndex, off.className]).toEqual(["false", -1, ""]);
    expect(off.dataset["tip"]).toBe("12 answered");
  });

  it("a click or enter picks a segment by its id", () => {
    const pick = vi.fn();
    const { getByTestId } = render(<Filters items={items} on="waiting" onPick={pick} testId="f" />);
    fireEvent.click(getByTestId("f.answered"));
    fireEvent.keyDown(getByTestId("f.waiting"), { key: "Enter" });
    expect(pick.mock.calls).toEqual([["answered"], ["waiting"]]);
  });
});

describe("ListSearch", () => {
  it("the ds's words: its placeholder, its VoiceOver name, its tip with the key; the key shown while empty", () => {
    const { getByTestId, rerender } = render(<ListSearch value="" testId="s" />);
    const input = getByTestId("s.input") as HTMLInputElement;
    expect([input.placeholder, input.getAttribute("aria-label")]).toEqual([ds.search, ds.search]);
    expect(getByTestId("s").dataset["tip"]).toBe(ds.search_tip({ key: "/" }));
    expect(getByTestId("s").querySelector(".b-kbd")?.textContent).toBe("/");
    expect(getByTestId("s").querySelector("svg[data-icon=search]")).not.toBeNull();
    rerender(<ListSearch value="perf" testId="s" />);
    expect(getByTestId("s").querySelector(".b-kbd")).toBeNull();
  });
});

describe("ListBar", () => {
  it("the list's row: its filters left, its search right; stuck only once its scroller scrolled it to the top", () => {
    const { getByTestId } = render(
      <div data-testid="sc" style={{ overflowY: "auto" }}>
        <ListBar inset end={<ListSearch value="" testId="s" />} testId="b"><Filters items={[{ id: "a", label: "all" }]} on="a" testId="f" /></ListBar>
      </div>,
    );
    const bar = getByTestId("b");
    expect(bar.className).toBe("b-listbar inset");
    expect(bar.querySelector(".b-listbar-start [data-testid=f]")).not.toBeNull();
    expect(bar.querySelector(".b-listbar-end [data-testid=s]")).not.toBeNull();
    expect(bar.dataset["stuck"]).toBeUndefined();
    const sc = getByTestId("sc");
    sc.scrollTop = 40;
    act(() => { sc.dispatchEvent(new Event("scroll")); });
    expect(bar.dataset["stuck"]).toBe("true");
  });
});

describe("GroupLabel", () => {
  it("its words and a count, the count in the accent only when it waits", () => {
    const { getByTestId, rerender } = render(<GroupLabel count={3} waits tip="3 waiting for you" testId="g">waiting for you</GroupLabel>);
    const n = getByTestId("g").querySelector(".n") as HTMLElement;
    expect([getByTestId("g").textContent, n.className, n.dataset["tip"]]).toEqual(["waiting for you3", "n wait", "3 waiting for you"]);
    rerender(<GroupLabel count={12} testId="g">earlier</GroupLabel>);
    expect(getByTestId("g").querySelector(".n")?.className).toBe("n");
  });
});

describe("ListRow", () => {
  it("one button holds the state and the two lines; its end beside it, never inside it", () => {
    const m = menu();
    const { getByTestId } = render(<ListRow title="run rm -rf target?" sub="acme · 21 d" state="?" end={<MoreButton menu={m} testId="r.more" />} testId="r" />);
    const body = getByTestId("r.open");
    expect(body.tagName).toBe("BUTTON");
    expect([body.querySelector(".l1")?.textContent, body.querySelector(".l2")?.textContent, body.querySelector(".state")?.textContent]).toEqual(["run rm -rf target?", "acme · 21 d", "?"]);
    expect(body.contains(getByTestId("r.more"))).toBe(false);
    expect(getByTestId("r").querySelectorAll("button button")).toHaveLength(0);
  });

  it("the chosen row says so (aria-current); a click opens, a right click anywhere is the menu's", () => {
    const m = menu();
    const open = vi.fn();
    const { getByTestId } = render(<ListRow title="t" on onClick={open} onContextMenu={m.onContextMenu} testId="r" />);
    expect(getByTestId("r").className).toBe("b-lrow b-chrome on");
    expect(getByTestId("r.open").getAttribute("aria-current")).toBe("true");
    fireEvent.click(getByTestId("r.open"));
    fireEvent.contextMenu(getByTestId("r"));
    expect([open.mock.calls.length, m.right]).toEqual([1, 1]);
  });
});

describe("RowAction", () => {
  it("a real button with its words, its tip and its click", () => {
    const go = vi.fn();
    const { getByTestId } = render(<RowAction onClick={go} tip="answer perf" testId="a">answer</RowAction>);
    const b = getByTestId("a");
    expect([b.tagName, b.className, b.textContent, b.dataset["tip"]]).toEqual(["BUTTON", "b-ract", "answer", "answer perf"]);
    fireEvent.click(b);
    expect(go).toHaveBeenCalledTimes(1);
  });
});

describe("entityKey (shared/entity.ts)", () => {
  it("one key per thing, the same for the same ids, the kind first", () => {
    expect([
      entityKey({ kind: "agent", project: "acme", agent: "perf" }),
      entityKey({ kind: "office", project: "acme" }),
      entityKey({ kind: "pr", project: "acme", url: "https://github.com/acme/web/pull/12" }),
      entityKey({ kind: "diff", project: "acme", agent: "perf" }),
      entityKey({ kind: "diff", project: "acme", agent: "perf", path: "src/a.rs" }),
      entityKey({ kind: "scheduled", project: "acme", id: 7 }),
    ]).toEqual(["agent:acme/perf", "office:acme", "pr:acme/https://github.com/acme/web/pull/12", "diff:acme/perf", "diff:acme/perf/src/a.rs", "scheduled:acme/7"]);
  });
});

describe("MoreButton", () => {
  it("its VoiceOver name and tip are the ds's words; a click opens the menu without the row's click", () => {
    const m = menu();
    const row = vi.fn();
    const { getByTestId } = render(<ListRow title="t" onClick={row} end={<MoreButton menu={m} testId="more" />} testId="r" />);
    const b = getByTestId("more");
    expect([b.getAttribute("aria-label"), b.dataset["tip"], b.getAttribute("aria-haspopup")]).toEqual([ds.more_actions, ds.more_actions, "menu"]);
    expect(b.querySelector("svg[data-icon=more]")).not.toBeNull();
    fireEvent.click(b);
    expect([m.opened, row.mock.calls.length]).toEqual([1, 0]);
  });
});

describe("AppIcon", () => {
  it("every icon draws, decorative, a 1.5 px stroke at every size, in currentColor", () => {
    const { container } = render(<div>{APP_ICONS.map((n) => <AppIcon key={n} name={n} size={18} />)}</div>);
    const svgs = [...container.querySelectorAll("svg")];
    expect(svgs).toHaveLength(APP_ICONS.length);
    for (const s of svgs) {
      expect([s.getAttribute("aria-hidden"), s.getAttribute("stroke"), s.getAttribute("stroke-width"), s.getAttribute("width")]).toEqual(["true", "currentColor", "1.5", "18"]);
      expect(s.childElementCount, s.dataset["icon"]).toBeGreaterThan(0);
      for (const p of s.querySelectorAll("path")) expect(p.getAttribute("vector-effect")).toBe("non-scaling-stroke");
    }
  });
});

describe("noClip (plan native-mac-plan §2)", () => {
  it("names each ancestor that clips: an overflow other than visible, a paint containment", () => {
    const { getByTestId } = render(
      <div style={{ overflow: "hidden" }} className="app-main">
        <div style={{ overflowY: "auto" }}>
          <div style={{ contain: "paint" }}>
            <i data-testid="obj" />
          </div>
        </div>
      </div>,
    );
    const obj = getByTestId("obj");
    expect(clippers(obj)).toEqual(["div contain: paint", "div overflow-y: auto", "div.app-main overflow-x: hidden", "div.app-main overflow-y: hidden"]);
    expect(() => { noClip(obj); }).toThrow(/clipped by 4 ancestor/);
  });

  it("is quiet for an element whose ancestors all let it overflow, and stops at its root", () => {
    const { getByTestId } = render(
      <div style={{ overflow: "hidden" }} data-testid="root">
        <div>
          <i data-testid="obj" />
        </div>
      </div>,
    );
    expect(() => { noClip(getByTestId("obj"), getByTestId("root")); }).not.toThrow();
    expect(clippers(getByTestId("obj"))).toEqual(["div overflow-x: hidden", "div overflow-y: hidden"]);
  });
});
