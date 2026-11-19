# beagle-utils

Small, dependency-free helpers shared by the apps in this monorepo
(`tandem/`, `rca/`, `workflows/`, `cli/`). Its own Cargo workspace, MSRV
1.79, with two members: `beagle-utils` (this directory, std-only) and
`beagle-tui` (`tui/`, terminal-UI pieces).

## Modules

| Module     | What it holds                                                              |
|------------|----------------------------------------------------------------------------|
| `paths`    | `$HOME` / XDG base-dir resolution, `~` expansion (pure `_from` variants)    |
| `process`  | Augmented `PATH` for GUI-launched apps, preset `Command`, `which`           |
| `shell`    | POSIX single-quote escaping                                                |
| `text`     | Char-safe truncation (`…`), byte truncation, one-line collapsing           |
| `duration` | Compact human durations (`1h2m`) and single-unit ages (`3m`)               |
| `fs`       | Atomic writes (temp sibling + fsync + rename), optional reads              |
| `notify`   | AppleScript string escaping                                                |
| `log`      | `tracing` subscriber init — only with the `tracing` feature                |
| `update`   | Self-update from GitHub releases: a `Channel` (repo, tag prefix, tarball), version parsing, release listing via `curl`, download + sha256 verify + atomic swap — only with the `update` feature |

## `beagle-tui` (`tui/`)

Terminal-UI pieces shared by the beagle-tools TUIs (`beagle recap`,
`beagle standup`, `beagle scout`, rca, Tandem's review TUI, agenttop). Depends on `ratatui`, `crossterm`, `notify`, `serde` and
`toml`, which is why it is a separate crate: `beagle-utils` stays std-only.

| Module      | What it holds                                                                  |
|-------------|--------------------------------------------------------------------------------|
| `markdown`  | Minimal, panic-free markdown → styled lines (prose vs preformatted)            |
| `ansi`      | ANSI SGR → styled `Text`, for colourised ASCII diagrams                        |
| `wrap`      | `DocLine` and word-wrapping layout with a source-line map                      |
| `docview`   | `DocView`: scrolling document pane with line cursor, visual select, pan, keys  |
| `doc`       | Document building blocks: dim text, rules, link rows, diagram blocks           |
| `widgets`   | Pane borders, centred overlays, the key-help sheet, cursor painting            |
| `watch`     | Recursive filesystem watcher + per-frame drain, for live reload                |
| `clipboard` | Copy via `pbcopy` / `wl-copy` / `xclip`                                         |
| `links`     | Open a URL with `open` / `xdg-open`, detached                                  |
| `config`    | `AppDirs`: store root from `--root` → `$<APP>_ROOT` → config → ancestor → cwd  |
| `version_picker` | The interactive `version list` picker over an `update::Channel`'s releases |
| `pane_resize` | `SideResize`: the shared `w` resize mode for a side pane's width (limits, reset, keys) |

### The side-pane resize standard

Every beagle-tools TUI with a side pane (a list or sidebar beside a main
pane) binds **`w`** to resize mode through `beagle_tui::pane_resize`, and
binds `w` to nothing else. Keep a `SideResize` in the app, draw the side
pane at `pane.width(total)`, call `pane.start()` on `w` (or say how to show
the pane when it is hidden), and while `pane.is_active()` hand every key to
`pane.handle_key(key, pane.last_total())`:

- `←`/`h` narrow and `→`/`l` widen the pane by 2 columns; `0` restores the
  app's default; `Esc`, `Enter`, `w` and `q` leave (q never quits here);
  every other key is ignored.
- The side pane stays within 20–100 columns and the main pane keeps 40; on
  a terminal too narrow for both, the side pane keeps its minimum.
- The width is not saved: a restart returns to the default. Show
  `pane_resize::HINT` in the footer while resizing and list
  `pane_resize::HELP` in the `?` overlay.

## Rules

- Only code used by **two or more** apps goes here.
- Std-only by default; anything heavier sits behind an opt-in feature.
- App-specific error types stay in the apps — functions here return
  `std::io::Result`, `Option`, or plain values.
