import { describe, expect, it } from "vitest";
import { quoteOf } from "./quote";

const ctx = (context: Record<string, string>, excluded = false) =>
  ({ ev: "context", context, ...(excluded ? { excluded } : {}) }) as const;

describe("the ❝ chip, the capsule's side (architect m_53938)", () => {
  it("fn space's read with a selection: the chip, its text trimmed, its app", () => {
    expect(quoteOf(ctx({ app: "Safari", selection: "  p99 412 ms \n" }), true)).toEqual({ text: "p99 412 ms", app: "Safari" });
  });

  it("none when his switch is off or the spotlight is not there, for an excluded app, without a selection", () => {
    expect(quoteOf(ctx({ app: "Safari", selection: "x" }), false)).toBeNull();
    expect(quoteOf(ctx({ app: "Safari", selection: "x" }, true), true)).toBeNull();
    expect(quoteOf(ctx({ app: "Safari" }), true)).toBeNull();
    expect(quoteOf(ctx({ app: "Safari", selection: "  \n" }), true)).toBeNull();
  });
});
