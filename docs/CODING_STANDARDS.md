# Coding standards

These are the coding standards for **every app in this monorepo**:
`tandem/`, `rca/`, `workflows/`, and the shared crate `utils/`. Every app
follows the same rules. Each app's `AGENTS.md` adds only app-specific facts:
architecture, commands and domain invariants. It never loosens a rule here.

The baseline is Tandem's standards. Rules that came from rca and were adopted
everywhere are marked _(from rca)_.

**The contract:** if `make check` (or the CI `gate` job) is green, the change
can be merged. There are no optional checks and no warnings-only checks. For
that to be safe, the gate has to be strict. So nothing below is weakened to
make a change pass: fix the code, or split the file.

Most rules are enforced by tooling. The configuration is shared, and
`scripts/check-standards.mjs` fails CI if any app drifts from it:

| What | Where it lives (shared) | Enforced by |
|---|---|---|
| Rust lint levels | `standards/cargo-lints.toml`, copied verbatim into every workspace `Cargo.toml` | clippy `-D warnings` and the drift check |
| Rust formatting | `rustfmt.toml` (root only) | `cargo fmt --check` |
| Rust toolchain | `rust-toolchain.toml` (root only) | CI installs exactly it |
| TS compiler flags | `standards/tsconfig.base.json` (apps `extends` it and may not redefine its keys) | `tsc` and the drift check |
| TS lint rules | `standards/eslint.base.js` (apps build on it and may only add rules) | `eslint --max-warnings 0` and the drift check |
| Formatting (TS/CSS/JSON/MD) | `.prettierrc.json` (root) | `prettier --check` |
| Node / pnpm | `.nvmrc`, plus `packageManager` in each `package.json` | the drift check |
| 400-line rule | `scripts/check-file-length.sh` | CI `standards` job |

---

## 1. General

1. **400-line limit on every source file.** This applies to `.rs .ts .tsx .js
   .mjs .css .astro .sh`. Blank lines and comments count. Check it with
   `./scripts/check-file-length.sh`. A file that wants to grow past 400 lines
   is telling you to split out a responsibility. Split it; never raise the
   limit. For Rust, turn the module into a directory with one file per
   responsibility, and re-export from `mod.rs` so public paths stay stable.
   _(The split technique is from rca.)_
2. **Files have one job.** Name the file after that job.
3. **Comments explain constraints the code can't express.** Write _why_,
   never _what_. Doc comments state the contract and the edge cases, not the
   implementation. _(from rca)_
4. **Zero-warning builds.** Every compiler, clippy, rustdoc, tsc and ESLint
   warning is an error in CI.
5. **Format before finishing:** run `cargo fmt --all` and `pnpm format` in
   the app you touched.
6. **Secrets never go in code, caches, logs, fixtures, or the repo.** Settings
   files that can hold tokens are written `0600`, atomically.
7. **Errors reach the user.** Every fallible path returns a typed error whose
   message someone could act on. For example: "`gh auth token` failed — is the
   gh CLI logged in?" In the desktop apps the error surfaces in the UI error
   bar, not just in a log.
8. **Degrade, don't die.** Malformed files, missing sections, invalid UTF-8
   and oversized input skip or degrade the affected item with a visible
   warning. They never crash the app. _(from rca)_
9. **Never block the UI thread or busy-loop.** Wait on channels, events,
   `select!` or timeouts. Use events over polling. Idle CPU is ~0%.
10. **Resources are bounded.** Event payloads are capped and log tails are
    truncated. File reads check the size limit from metadata first _(from
    rca)_. Every child process has a timeout and a kill path. A stuck agent or
    subprocess must never wedge the app.
11. **Write atomically.** Persisted files are written to a temp file in the
    same directory and then renamed (`beagle_utils::fs::write_atomic`).
    _(from rca)_
12. **Paths from a UI or an IPC caller are untrusted.** Canonicalize them, and
    refuse anything outside the directory the command is allowed to touch.
    _(from workflows)_
13. **On-disk and IPC formats are public API.** Changing one needs a migration
    note in the PR and an update to that app's `AGENTS.md`. Schema migrations
    are append-only: never edit a migration that has shipped. _(the general
    form is from rca)_
14. **Shared code goes in `utils/`** (`beagle-utils`), and only when two or
    more apps need it. Domain types and app error enums stay in their app. See
    `utils/README.md`.

## 2. Rust

### Lints (`standards/cargo-lints.toml`)
- `unsafe_code = "forbid"`. There is no `unsafe` anywhere, and it can't be
  re-allowed locally.
- `clippy::all` is deny and `clippy::pedantic` is warn (deny in CI). The only
  pedantic exceptions are the four `allow`s in the shared file.
- `unwrap_used`, `expect_used`, `panic`, `todo` and `dbg_macro` are **denied**.
  Errors flow through `Result`, so the user sees a message, never a crash.
- `missing_docs` _(from rca)_: every public item has a doc comment that says
  something the signature doesn't. `cargo doc` runs with `-D warnings` in CI.
- `missing_debug_implementations`: public types derive or implement `Debug`.
- **Every `#[allow(...)]` needs an adjacent comment explaining why.** Scope it
  to the smallest item possible, never module-wide or crate-wide. Test modules
  may `#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]`, with
  that comment. _(from rca)_
- Every package opts in with `[lints] workspace = true`. Per-package lint
  tables are rejected by the drift check.

### Errors
- **Library crates:** one error type per crate, built with `thiserror` and
  carrying context. I/O errors carry the path they happened on
  (`Error::Io { path, source }`). _(from rca)_
- **Tauri commands** return the app's serializable error type
  (`TandemError`, `AppError`, …), never a bare `String`.
- **Binaries** (CLIs, daemons) may use `anyhow` in `main.rs` and in
  top-level command glue only. Library code never returns `anyhow::Error`.
- `Result` goes all the way up with `?`. **No silent `.ok()`, `let _ =` or
  `unwrap_or_default()` on an error** without a comment saying why ignoring
  it is correct. _(from rca)_

### Types _(from rca)_
- **Make illegal states unrepresentable.** Prefer a type the compiler checks
  over a comment the reviewer might miss.
- **Newtypes over primitives** for IDs, slugs and paths with invariants.
  Construct them through a validating constructor, so that holding a value
  proves the invariant. Don't add an unchecked escape hatch.
- **Enums over stringly-typed data,** with explicit serde renames.
- **Parse, don't validate.** Deserialize external data into fully-typed
  structs at the boundary, in one place. Use `#[serde(deny_unknown_fields)]`
  on user-authored config and manifests, so typos fail loudly.
- **Exhaustive matches:** no `_ =>` arm on enums we own.
- **Narrow visibility:** fields are private unless a consumer needs them.
  Prefer `pub(crate)` to `pub`.

### Memory and performance _(from rca)_
- Borrow first and `clone` last: take `&str` and `&[T]`. Every `.clone()`
  should answer "who needs to own this?".
- Use `Vec::with_capacity` when the size is known. Prefer iterator chains to
  intermediate `Vec`s, and don't `collect` only to iterate again.
- Allocate outside hot loops (render, per-event and per-line code).
- **No global mutable state** (`static` with `Mutex`, `lazy_static`). Pass
  state explicitly. `OnceLock` for immutable, computed-once values is fine.

### Concurrency
Each app declares **one** concurrency model in its `AGENTS.md`, and code in
that app follows it:

| App | Model |
|---|---|
| tandem | async (tokio) |
| rca | threads plus channels (no async) |
| workflows | synchronous, with the daemon as the single writer |

Whatever the model:
- Hold locks only for short sections that don't `.await`.
- Never call `block_on` inside async code.
- `kill_on_drop` or explicit group-kill, so no child process is orphaned.

### Tests
Full strategy, per-crate-type guidance and the checklist:
[`TESTING.md`](TESTING.md). The rules:
- New logic ships with unit tests. **Every bug fix ships with a regression
  test** that fails without the fix.
- No network, no sleeping, no TTY, no real filesystem outside `tempfile`
  dirs. _(from rca)_
- Data models get serde round-trip tests plus invalid-input rejection tests;
  IPC types get a test pinning their JSON. _(from rca)_
- Parsers and formatters get `proptest` properties where an invariant holds
  for all inputs.
- Unit tests beside the code (`#[cfg(test)] mod tests`, or
  `#[path = "tests/<module>.rs"]` once the file nears 400 lines);
  cross-module tests in the crate's `tests/`; doc-tests on public `utils/`
  APIs.
- Put a test in the crate that owns the behavior: PRs only run tests for the
  affected packages and their dependents.

### Dependencies _(from rca)_
A new runtime dependency needs a reason in the PR description: what it does,
why ~50 lines of our own code can't do it, its transitive weight, and its
effect on build time. Dev-tooling dependencies are freer. Prefer what is
already in the app's lockfile.

## 3. TypeScript / React

### Compiler and lint
- Every app extends `standards/tsconfig.base.json`:
  - Tandem's flags: `strict`, `noUncheckedIndexedAccess`,
    `noImplicitOverride`, unused locals and parameters, and switch
    fallthrough.
  - Flags from rca: `exactOptionalPropertyTypes`, `noImplicitReturns` and
    `verbatimModuleSyntax`.
- **Never weaken a compiler flag to make an error go away.** Flags may be
  added; none may be removed. _(from rca)_
- Every app's ESLint builds on `standards/eslint.base.js`, and runs with
  `--max-warnings 0`:
  - `strictTypeChecked` plus `stylisticTypeChecked` and react-hooks.
  - `max-lines` 400 (counting everything), `eqeqeq`, `no-console` (warn and
    error are allowed), and an exhaustive switch.
  - From rca: `max-lines-per-function` 120, `consistent-type-imports`, and
    `explicit-module-boundary-types`.
- **No `any`, no non-null `!` and no unsafe casts.** Narrow with a type guard
  instead. _(from rca)_
- Every exported function declares its return type.

### Structure
- **Exactly one module wraps `invoke()`** (the IPC module). Its path is named
  in the app's `AGENTS.md`. Components never call `invoke` directly.
- **Types mirror the Rust serde output by hand** (snake_case fields,
  `kind`-tagged enums). There is no codegen, so a field added in Rust and not
  in TS is invisible rather than a build error. Update both sides in the same
  PR.
- **Components are presentational.** Shared state lives where the app's
  `AGENTS.md` says (for example the zustand store). Pure logic lives in
  `src/lib/`. One component per file. _(from rca)_
- **Rust owns the domain.** Sorting, filtering, parsing and lifecycle rules
  live in Rust and cross the boundary as data. The frontend renders; it does
  not reimplement them. _(from rca)_
- **Parse, don't validate:** data from IPC is narrowed with a type guard at
  the boundary. _(from rca)_

### Behavior
- Render from cache or current state first. Every network wait shows a
  visible loading or sync state.
- A failed command shows an error banner and leaves the rest of the UI
  usable.
- Unknown enum values from a newer backend render neutrally and never throw.
  _(from rca)_
- No floating promises: every promise is awaited or explicitly handled.

### Tests _(from rca)_
Full strategy: [`TESTING.md`](TESTING.md). The rules:
- `test` runs vitest; `e2e` runs Playwright (Chromium) from
  `playwright.config.ts` at the package root. Both run in CI.
- Pure logic in `src/lib/` gets unit tests; type guards get accept and reject
  cases. Components with behavior get `@testing-library/react` tests that
  query by role/label/text.
- Tauri frontends run e2e against `vite --mode e2e`, which installs `mockIPC`
  fixtures (never in the production bundle). An IPC change updates the TS
  type, its guard test and the e2e fixture in the same PR.
- No snapshot tests, no `.only`/`.skip`, no fixed sleeps.

## 4. Workflow

- Branch from `main`, named `feat/<slug>`, `fix/<slug>`, `docs/<slug>`,
  `perf/<slug>` or `chore/<slug>`. _(from rca)_
- Commits follow Conventional Commits, scoped by app: `feat(tandem): …`,
  `fix(rca): …`, `chore(utils): …`. _(from rca)_
- Keep PRs small and single-purpose. A PR that changes a data format, the
  store and the renderer is three PRs.
- Every PR description answers **what** changed, **why**, and **how it was
  tested**. _(from rca)_
- **Before you say you're done:** run `make check-<app>` (or `make check`) at
  the repo root, plus `make standards`. If you changed runtime behavior, run
  the app and exercise the flow you touched.
