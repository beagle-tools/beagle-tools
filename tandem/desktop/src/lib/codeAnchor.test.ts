import { describe, expect, it } from "vitest";

import { blobUrl, parseCodeAnchor, resolveLink } from "./codeAnchor";

describe("parseCodeAnchor", () => {
  it("parses tandem:// links with and without a line", () => {
    expect(parseCodeAnchor("tandem://src/x.rs#L42")).toEqual({ path: "src/x.rs", line: 42 });
    expect(parseCodeAnchor("tandem://src/x.rs")).toEqual({ path: "src/x.rs", line: null });
  });

  it("strips a leading ./ or / and ignores non-line hashes", () => {
    expect(parseCodeAnchor("tandem://./a.ts#l3")).toEqual({ path: "a.ts", line: 3 });
    expect(parseCodeAnchor("tandem:///a.ts#section")).toEqual({ path: "a.ts", line: null });
  });

  it("rejects other schemes and empty paths", () => {
    expect(parseCodeAnchor("https://example.com")).toBeNull();
    expect(parseCodeAnchor("tandem://")).toBeNull();
    expect(parseCodeAnchor(undefined)).toBeNull();
  });
});

describe("resolveLink", () => {
  it("treats missing hrefs and in-page fragments as nothing", () => {
    expect(resolveLink(undefined)).toEqual({ kind: "none" });
    expect(resolveLink("#top")).toEqual({ kind: "none" });
  });

  it("sends http(s) to the browser and drops other schemes", () => {
    expect(resolveLink("https://example.com/x")).toEqual({
      kind: "external",
      url: "https://example.com/x",
    });
    expect(resolveLink("mailto:a@b.c")).toEqual({ kind: "none" });
    expect(resolveLink("javascript:alert(1)")).toEqual({ kind: "none" });
  });

  it("treats tandem:// anchors and bare paths as code", () => {
    expect(resolveLink("tandem://a.rs#L1")).toEqual({
      kind: "path",
      anchor: { path: "a.rs", line: 1 },
    });
    expect(resolveLink("README.md")).toEqual({
      kind: "path",
      anchor: { path: "README.md", line: null },
    });
  });
});

describe("blobUrl", () => {
  const repo = { owner: "acme", name: "widgets" };
  it("links a file at a ref, with an optional line anchor", () => {
    expect(blobUrl(repo, "main", { path: "src/a.ts", line: 9 })).toBe(
      "https://github.com/acme/widgets/blob/main/src/a.ts#L9",
    );
    expect(blobUrl(repo, "main", { path: "src/a.ts", line: null })).toBe(
      "https://github.com/acme/widgets/blob/main/src/a.ts",
    );
  });
});
