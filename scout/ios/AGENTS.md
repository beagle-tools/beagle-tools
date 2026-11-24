# AGENTS.md — scout iPhone app (`scout/ios/`)

Read the root [`AGENTS.md`](../../AGENTS.md) and
[`docs/CODING_STANDARDS.md`](../../docs/CODING_STANDARDS.md) first. The
API this app reads is [`docs/serve-api.md`](../../docs/serve-api.md); it is
the contract, and the app never reads the scout store directly.

## What it is

A SwiftUI app (iOS 17+, Swift 6, main-actor by default) that shows the scout
store read-only, the way the `beagle scout` TUI does. No third-party
dependencies.

## Layout

```text
Scout.xcodeproj/    plain Xcode project; synchronized folders, so files need no project edits
  xcshareddata/xcschemes/Scout.xcscheme   the shared scheme CI tests (unit + UI)
Scout/              the app target
  ScoutApp.swift    @main entry
  RootView.swift    the tabs: Scout, Settings
ScoutTests/         unit tests (Swift Testing), hosted by the app
ScoutUITests/       XCUITest smoke tests
```

## Rules

- **Gate:** `make check-ios` (= `scripts/ci/ios.sh scout/ios`, the CI job
  `scout-ios`): `swift-format lint --strict` with the root `.swift-format`,
  then `xcodebuild test` on an iPhone simulator, then an unsigned Release
  build. Warnings are errors.
- **400 lines per file** applies to `.swift` too (`scripts/check-file-length.sh`).
- **Signing stays automatic with no team ID.** Never commit
  `DEVELOPMENT_TEAM`, a provisioning profile or manual signing;
  `make standards` fails it (`scripts/ci/ios-standards.mjs`).
- **No personal data:** no real hostnames, tailnet names or tokens in code,
  tests or fixtures. Use `mac.example.ts.net` and obviously fake tokens.
- Test fixtures copy the JSON shapes in `docs/serve-api.md` with neutral
  example data.
