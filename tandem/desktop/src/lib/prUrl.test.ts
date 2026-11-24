import { describe, expect, it } from "vitest";

import { parsePrUrl } from "./prUrl";

describe("parsePrUrl", () => {
  it("extracts the slug and number from a PR URL", () => {
    expect(parsePrUrl("https://github.com/acme/widgets/pull/42")).toEqual({
      slug: "acme/widgets",
      number: 42,
    });
  });

  it("accepts trailing paths, surrounding whitespace, http and any case", () => {
    expect(parsePrUrl("  https://github.com/acme/my.repo-x/pull/7/files  ")).toEqual({
      slug: "acme/my.repo-x",
      number: 7,
    });
    expect(parsePrUrl("HTTP://GitHub.com/a/b/PULL/3")).toEqual({ slug: "a/b", number: 3 });
  });

  it("rejects anything that isn't a github.com pull URL", () => {
    expect(parsePrUrl("https://github.com/acme/widgets/issues/42")).toBeNull();
    expect(parsePrUrl("https://gitlab.com/acme/widgets/pull/42")).toBeNull();
    expect(parsePrUrl("acme/widgets#42")).toBeNull();
    expect(parsePrUrl("")).toBeNull();
  });
});
