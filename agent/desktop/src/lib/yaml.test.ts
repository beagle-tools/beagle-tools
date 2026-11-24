import { describe, expect, it } from "vitest";

import { highlightYaml, type Token } from "./yaml";

const kinds = (line: Token[] | undefined): [string, string][] =>
  (line ?? []).filter((t) => t.kind !== "plain").map((t) => [t.kind, t.text]);

describe("highlightYaml", () => {
  it("colours keys, values and comments", () => {
    const [line] = highlightYaml("timeout_minutes: 30 # half an hour");
    expect(kinds(line)).toContainEqual(["key", "timeout_minutes"]);
    expect(kinds(line)).toContainEqual(["number", "30"]);
    expect(kinds(line)).toContainEqual(["comment", "# half an hour"]);
  });

  it("treats list items and literals", () => {
    const lines = highlightYaml("triggers:\n  - review\nenabled: true\n");
    expect(kinds(lines[1])).toContainEqual(["punct", "- "]);
    expect(kinds(lines[2])).toContainEqual(["literal", "true"]);
  });

  it("keeps one token list per input line", () => {
    expect(highlightYaml("a: 1\nb: 2")).toHaveLength(2);
  });
});
