import { describe, expect, it } from "vitest";

import { errorMessage } from "./ipc";

describe("errorMessage", () => {
  it("unwraps the AppError { message } shape", () => {
    expect(errorMessage({ message: "run not found" })).toBe("run not found");
    expect(errorMessage(new Error("boom"))).toBe("boom");
  });

  it("stringifies anything else", () => {
    expect(errorMessage("plain")).toBe("plain");
    expect(errorMessage(42)).toBe("42");
    expect(errorMessage(null)).toBe("null");
  });
});
