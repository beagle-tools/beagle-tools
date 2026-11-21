import { describe, expect, it } from "vitest";
import { jobsKeyAction } from "./JobsControls";

describe("jobsKeyAction", () => {
  it("uses agenttop's keys for the Running / All toggle", () => {
    expect(jobsKeyAction("f")).toBe("running");
    expect(jobsKeyAction("a")).toBe("all");
  });

  it("leaves r unbound (agenttop's reload, not a filter)", () => {
    expect(jobsKeyAction("r")).toBeNull();
  });

  it("keeps / for search and H for the hidden projects", () => {
    expect(jobsKeyAction("/")).toBe("search");
    expect(jobsKeyAction("H")).toBe("hidden");
    expect(jobsKeyAction("h")).toBeNull();
  });
});
