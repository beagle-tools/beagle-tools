# beagle-utils

Small, dependency-free helpers shared by the apps in this monorepo
(`tandem/`, `rca/`, `workflows/`). Its own Cargo workspace, MSRV 1.79.

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

## Rules

- Only code used by **two or more** apps goes here.
- Std-only by default; anything heavier sits behind an opt-in feature.
- App-specific error types stay in the apps — functions here return
  `std::io::Result`, `Option`, or plain values.
