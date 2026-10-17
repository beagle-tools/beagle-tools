// Render test: the pill is absent when up to date, and reflects progress
// and failures while installing.
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { UpdateCheck } from "../hooks/useUpdateCheck";
import { UpdatePill } from "./UpdatePill";

function check(overrides: Partial<UpdateCheck>): UpdateCheck {
  return { available: null, installing: false, error: null, install: vi.fn(), ...overrides };
}

describe("UpdatePill", () => {
  afterEach(cleanup);

  it("renders nothing when no update is available", () => {
    const { container } = render(<UpdatePill update={check({})} />);
    expect(container.childElementCount).toBe(0);
  });

  it("offers the newer version and installs on click", () => {
    const install = vi.fn();
    render(<UpdatePill update={check({ available: "0.30.0", install })} />);
    fireEvent.click(screen.getByRole("button", { name: "update v0.30.0" }));
    expect(install).toHaveBeenCalledOnce();
  });

  it("is disabled while installing and says why after a failure", () => {
    const { rerender } = render(
      <UpdatePill update={check({ available: "0.30.0", installing: true })} />,
    );
    expect(screen.getByRole<HTMLButtonElement>("button").disabled).toBe(true);
    rerender(
      <UpdatePill update={check({ available: "0.30.0", error: "update failed: offline" })} />,
    );
    const button = screen.getByRole("button", { name: "update failed — retry" });
    expect(button.getAttribute("title")).toBe("update failed: offline");
  });
});
