# Testing

How every app in this monorepo is tested, what a change must ship with, and
how CI picks which tests to run. It is written for the agents (and humans)
who write tests here: follow it literally. It sits beside
[`CODING_STANDARDS.md`](CODING_STANDARDS.md); where a rule here is stricter,
this file wins for tests.

**The contract:** the CI `gate` is the merge decision. A change whose tests
don't prove its behavior is not done, even when the gate is green.

---

## 1. The layers

| Layer | Tool | Lives in | Runs in |
|---|---|---|---|
| Rust unit | `cargo test` | `#[cfg(test)] mod tests` beside the code, or `src/**/tests/<module>.rs` | `scripts/ci/rust.sh` |
| Rust integration | `cargo test` | `<crate>/tests/*.rs` (public API only) | `scripts/ci/rust.sh` |
| Rust doc-tests | `cargo test` | `///` examples on public `utils/` APIs | `scripts/ci/rust.sh` |
| Rust property | `proptest` | beside the unit tests | `scripts/ci/rust.sh` |
| TS unit / component | vitest (+ `@testing-library/react`) | `src/**/*.test.ts(x)` | `pnpm test` in `scripts/ci/frontend.sh` |
| Browser e2e | Playwright, Chromium only | `e2e/*.spec.ts`, `playwright.config.ts` at the package root | `pnpm e2e` in `scripts/ci/frontend.sh` |
| CI scripts | `node:test` | `scripts/ci/*.test.mjs` | `make standards`, CI `standards` job |

Every layer is part of the gate. There are no optional suites, no
`#[ignore]`d tests without an issue link in the comment, and no `test.skip` /
`test.only` / `describe.only` in committed code (Playwright's `forbidOnly`
is on in CI).

## 2. What each kind of change MUST ship with

| Change | Required tests |
|---|---|
| New logic (parsing, caching, persistence, scheduling, process handling, a pure TS helper) | Unit tests for the happy path, every error branch you wrote, and the edges: empty, one, many, max size, invalid UTF-8 where input is bytes. |
| Bug fix | A regression test that **fails without the fix**. Name it after the bug (`does_not_drop_last_line_without_newline`). Run it once against the unfixed code to prove it. |
| IPC contract change (a Tauri command's args, result or error type; an event payload) | Rust: a test of the command's core function with the new shape, plus a serde test pinning the JSON (field names, `kind` tag). TS: update the hand-written type and its type guard, with a unit test that the guard accepts the new shape and rejects the old/invalid one. e2e: update the app's mockIPC fixture to the new shape, and a spec that exercises the UI path that reads it. All three in the same PR. |
| Data format change (on-disk file, manifest, DB schema, cache) | Serde round-trip (`value -> bytes -> value == value`), a test that a file in the **previous** format still loads (or is migrated), and tests that invalid input is rejected with a useful error (unknown field under `deny_unknown_fields`, wrong type, truncated file). Schema migrations get a test that applies them to a DB at the previous version. |
| New UI screen or flow | Component tests for its behavior (state changes, error banner, empty state). An e2e spec that reaches the screen from app start and completes the main flow once. |
| New Tauri command | A unit test of the function the command wraps (keep the `#[tauri::command]` a thin shell over a testable function), its mock in the e2e fixture, and coverage in at least one spec. |
| Refactor with no behavior change | No new tests required; every existing test still passes **unchanged**. If you have to edit an assertion, it wasn't a pure refactor. |
| Performance work | A test that pins the bound you rely on (capacity, truncation length, number of calls) — not a wall-clock benchmark. |

## 3. Rules for every test

1. **No network.** No real HTTP, DNS, or sockets to other hosts. Inject a
   client/trait and fake it, or feed recorded fixture bytes to the parser.
   Loopback servers created by the test itself (e.g. an IPC socket in a
   tempdir) are allowed.
2. **No sleeping, no wall-clock waits.** No `thread::sleep`,
   `tokio::time::sleep`, `setTimeout`-based waits, or `page.waitForTimeout`.
   Use channels, `tokio::time::pause()` + `advance()`, an injected `Clock`,
   vitest fake timers, or Playwright's auto-waiting `expect(...)` assertions.
3. **No real filesystem outside a temp dir.** Use `tempfile::TempDir` (Rust)
   or `fs.mkdtemp(os.tmpdir())` (TS). Never read or write `$HOME`,
   `~/.config`, `~/.local/state`, the app's real data dir, or the repo
   checkout. Code that resolves those paths takes the base dir as a
   parameter or env override, so tests can point it at the tempdir.
4. **No TTY, no GUI, no real child agents.** TUI rendering is tested against
   a buffer backend (`ratatui::backend::TestBackend`). Subprocess code runs
   a tiny fixture command (`sh -c 'echo …'`) or a fake behind a trait, never
   `claude`, `gh`, `git push` or tmux on the developer's machine.
5. **Deterministic.** No dependence on test order, the current time, the
   timezone, locale, random seeds (proptest seeds are fine; it records
   failures), or HashMap iteration order in assertions.
6. **Independent.** Each test builds its own state. No shared mutable
   statics, no reliance on another test's files.
7. **Fast.** A unit test runs in milliseconds. Anything over ~1s is a design
   smell: inject the slow part.
8. **Assert behavior, not implementation.** Assert outputs, returned errors,
   persisted bytes and rendered text a user sees. Don't assert private call
   sequences. No snapshot tests (`insta`, `toMatchSnapshot`): they assert
   everything and therefore nothing. Inline expected values instead.
9. **Errors are asserted precisely.** Match the error variant
   (`assert!(matches!(err, Error::Io { .. }))`), and for user-facing
   messages assert the actionable part of the text.
10. **Lint rules still apply.** Test code is under the same clippy and ESLint
    config. Rust test modules may
    `#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` with a
    comment saying why; nothing else is relaxed.

## 4. Naming and layout

- **Rust test names are sentences** in snake_case describing the behavior:
  `rejects_slug_with_path_separator`, `round_trips_manifest_with_all_fields`.
  Not `test_parse_1`.
- **TS:** `describe("<unit>")` + `it("<does something> when <condition>")`.
  File next to the code: `src/lib/format.ts` -> `src/lib/format.test.ts`.
- **e2e:** one spec file per screen or flow (`e2e/inbox.spec.ts`), test titles
  say what the user does: `test("opens a PR from the inbox and leaves a comment")`.
- **Rust layout:** inline `#[cfg(test)] mod tests` while the file fits the
  400-line rule; otherwise a sibling file wired with
  `#[cfg(test)] #[path = "tests/<module>.rs"] mod tests;`. Cross-module tests
  go in `<crate>/tests/`.
- **Fixtures:** small inline literals first. Larger inputs live in
  `<crate>/tests/fixtures/` (Rust, loaded with
  `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/…"))`)
  or `e2e/fixtures/` / `src/**/__fixtures__/` (TS). Fixtures are realistic
  (shapes copied from real output) but **contain no secrets, tokens, real
  emails or private repo names**. Builders (`fn sample_run() -> Run`) beat
  copy-pasted structs.

## 5. Per app type

### Rust library crate (`utils`, `tandem-core`, `agent-core`, `beagle-agent`, …)
- Unit-test every public function and every error variant.
- `utils/` public APIs carry a runnable doc-test example (it is the usage
  documentation). Doc-tests must not touch the network or the real FS either.
- Parsers and formatters get property tests (section 6).

### Rust CLI (`beagle`, `agentctl`)
- Keep `main.rs` thin: parse args (clap) and call a `run(args, io) -> Result`
  function. Test `run` with in-memory writers and a tempdir as the home/state
  dir. Test clap parsing with `Cli::try_parse_from([...])`.
- Assert exit-relevant results (the returned `Result`), stdout text and files
  written, not log lines.

### Rust daemon (`agentd`, `beagle-agentd`)
- Test one tick/step of the loop as a function over injected state, clock and
  spawner. Never start the forever-loop in a test.
- Control-socket / IPC handlers: test the request -> response function
  directly; an end-to-end socket test may bind inside a tempdir.
- Crash recovery: a test that seeds the state store with a "running" record
  whose process is gone and asserts it is finalized.

### Rust TUI (`beagle` TUI, `agenttop`)
- Separate state updates (`fn update(state, event) -> state`) from drawing;
  unit-test `update` exhaustively.
- Render tests draw into `TestBackend` at a fixed size and assert on
  specific cells/lines (a title, a selected row marker), not whole-screen
  snapshots.

### Tauri commands (`tandem/src-tauri`, `rca/desktop/src-tauri`, `workflows/desktop/src-tauri`)
- The `#[tauri::command]` fn only extracts state and delegates. The logic it
  delegates to is a plain function tested like library code.
- Pin the wire format: a `serde_json::to_value` test for each result type and
  error type the frontend reads. This is what keeps the hand-written TS types
  honest.
- Path arguments from the frontend: test that `..`, absolute paths outside
  the allowed root and symlink escapes are refused.

### React components
- `@testing-library/react` + vitest (jsdom). Query by role/label/text the
  user sees (`getByRole("button", { name: "Retry" })`), never by class name
  or test-id unless nothing else is accessible.
- Test behavior: what renders for loading/empty/error/data states, what a
  click or keypress does, that a failed command shows the error banner and
  leaves the rest usable, that an unknown enum value renders neutrally.
- Mock the IPC module (the one file that wraps `invoke`), never `invoke`
  itself from a component test.

### Pure TS library code (`src/lib/`)
- Plain vitest unit tests; no DOM. Type guards get accept **and** reject
  cases for every field.

### Playwright e2e with mocked IPC (every Tauri frontend)
- Contract: `playwright.config.ts` at the package root, specs in
  `e2e/*.spec.ts`, script `"e2e": "playwright test"`, Chromium only.
  `webServer` starts `vite --mode e2e` on a fixed port (tandem uses 1430, so
  it doesn't collide with `make dev`).
- In mode `e2e` the app installs `mockIPC` from `@tauri-apps/api/mocks` with
  realistic fixture data before rendering. This is loaded via a dynamic
  `import()` guarded by `import.meta.env.MODE === "e2e"`, so it is never in
  the production bundle path.
- The mock answers every command the screens under test call; an unmocked
  command throws with its name, so a missing fixture fails loudly.
- Cover: app start reaches the main screen, each main screen renders fixture
  data, and one or two real flows end to end (including one error path, by
  making the mock reject).
- Use web-first assertions (`await expect(locator).toBeVisible()`), role/text
  locators, no `waitForTimeout`, no fixed sleeps.

### Astro static site (`rca/web`)
- vitest for any TS helpers (content parsing, URL building).
- Playwright against `astro dev` / `astro preview` on a fixed port: every
  page returns 200, has its title and primary heading, and internal links
  resolve. No external network (block third-party requests in the config if
  a page references any).

## 6. Property tests (`proptest`)

Use them where an invariant holds for all inputs and hand-picked examples
would miss cases: parsers, formatters, escaping, slug/ID validation,
serialization, diff/merge logic, truncation.

- Prefer **round-trip** (`parse(format(x)) == x`), **never-panics**
  (`parse(arbitrary_string)` returns `Ok` or `Err`, never panics), and
  **invariant** (`truncate(s, n).len() <= n`) properties.
- Generate valid values with strategies (`"[a-z0-9-]{1,40}"`, `prop_compose!`)
  rather than generating and filtering (`prop_assume!` rejects waste runs).
- Keep the default case count; don't raise it to "be safe" (it slows every
  run). Commit `proptest-regressions/` files: they are regression tests.
- Add `proptest` as a `[dev-dependencies]` entry of the crate that uses it.

## 7. Which tests run (selection)

Two levels, both automatic:

1. **Apps:** CI's `changes` job skips every gate of an app the diff doesn't
   touch (`.github/workflows/ci.yml`). Shared paths (`utils/`, `scripts/`,
   `standards/`, toolchain, CI files) run every app.
2. **Cargo packages:** inside a Rust gate, a pull request runs
   `cargo test -p <affected>…` instead of the whole workspace.
   `scripts/ci/affected.mjs` computes it:
   - changed files since the merge-base with the base branch (plus
     uncommitted and untracked files locally);
   - each file maps to the package whose directory contains it, plus any
     package that compiles it in via `include_str!`/`include_bytes!` (e.g.
     `skills/beagle/SKILL.md` -> `beagle`);
   - add every package that depends on an affected one, transitively, across
     path dependencies into other workspaces (a change in `rca/cli` re-tests
     `beagle-desktop` in `rca/desktop/src-tauri`);
   - **whole workspace** when its `Cargo.toml`/`Cargo.lock`/`.cargo/`
     changed, or `utils/`, `rust-toolchain.toml`, `scripts/`, `standards/`,
     `ci.yml`/`_rust.yml` changed, on every push to `main`, and when the
     base can't be resolved.

   fmt, clippy and doc always cover the whole workspace. The job log (and
   the run summary) lists each tested package and why.

Frontends always run their full vitest and e2e suites when their app gate
runs.

Consequence for you: **a test only protects code that is inside, or
upstream of, the package it lives in.** Put tests in the crate that owns the
behavior, and don't rely on a downstream crate's test to catch an upstream
bug — it may not be selected.

## 8. Running tests locally

From the repo root (the Makefile puts cargo and node on PATH):

```sh
make test-affected                    # cargo tests for packages changed vs origin/main (+ dependents)
make test-affected BASE=HEAD          # only what your uncommitted changes affect
make check-rca                        # the full rca gate, exactly as CI
make e2e-tandem                       # Playwright for one frontend (also e2e-rca-desktop, -rca-web, -workflows-desktop)
make test-scripts                     # the selection logic's own tests
```

Narrower, inside a workspace dir (`cd rca`, `cd tandem`, …):

```sh
cargo test -p beagle                          # one crate
cargo test -p beagle skill::tests             # one module
cargo test -p beagle rejects_unknown_field    # tests whose name contains this
cargo test -p beagle --test cli               # one integration test file (tests/cli.rs)
cargo test -p beagle-utils --doc              # doc-tests only
```

In a frontend dir (`cd tandem`, `cd rca/desktop`, …), via `corepack pnpm`:

```sh
corepack pnpm test                            # all vitest
corepack pnpm exec vitest run src/lib/format.test.ts
corepack pnpm exec vitest -t "renders the error banner"
corepack pnpm e2e                             # all Playwright specs
corepack pnpm exec playwright test e2e/inbox.spec.ts
corepack pnpm exec playwright test -g "leaves a comment" --headed
corepack pnpm exec playwright show-trace test-results/**/trace.zip
```

See what selection would pick, without running anything:

```sh
node scripts/ci/affected.mjs rca --base origin/main
node scripts/ci/affected.mjs rca/desktop/src-tauri --changed rca/cli/src/lib.rs
```

## 9. Checklist before you say "done"

- [ ] Every new branch of logic has a test; every bug fix has a regression
      test that failed before the fix (you ran it).
- [ ] IPC change: Rust serde test, TS type + guard test, e2e fixture and spec
      updated together.
- [ ] Data format change: round-trip, previous-format load, invalid-input
      rejection.
- [ ] No network, no sleeps, no real home/config/state dirs, no TTY, no real
      agents or `gh`; tempdirs for all FS access.
- [ ] Names describe behavior; no snapshots; no `.only`/`.skip`/`#[ignore]`.
- [ ] Tests live in the crate/package that owns the behavior.
- [ ] `make test-affected` (Rust) and `pnpm test` / `pnpm e2e` (frontend you
      touched) pass locally; then `make check-<app>` and `make standards`.
