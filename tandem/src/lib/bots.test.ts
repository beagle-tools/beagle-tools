import { describe, expect, it } from "vitest";

import { isBotLogin } from "./bots";

describe("isBotLogin", () => {
  it("recognizes GitHub apps and named service accounts", () => {
    for (const login of ["dependabot[bot]", "renovate-bot", "ci-bot", "dependabot", "bot"]) {
      expect(isBotLogin(login)).toBe(true);
    }
  });

  it("does not flag people whose names merely contain 'bot'", () => {
    for (const login of ["botros", "bottle", "abbott"]) {
      expect(isBotLogin(login)).toBe(false);
    }
  });

  it("is false for blank logins", () => {
    expect(isBotLogin("")).toBe(false);
    expect(isBotLogin("   ")).toBe(false);
  });
});
